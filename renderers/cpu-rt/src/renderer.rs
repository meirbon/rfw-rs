use crate::surface::Surface;

use futures::executor::block_on;
use glam::*;
use rayon::prelude::*;
use rtbvh::builders::{locb::LocallyOrderedClusteringBuilder, Builder};
use rtbvh::{Bounds, Ray, AABB, BVH, MBVH};
use scene::renderers::Renderer;
use scene::{
    constants, AreaLight, BitVec, DeviceMaterial, DirectionalLight, HasRawWindowHandle, Instance,
    Light, Material, Mesh, PointLight, SpotLight, TIntersector, Texture,
};
use shared::*;
use std::error::Error;
use std::fmt::{Display, Formatter};
use wgpu::TextureCopyView;

pub struct RayTracer<'a> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: wgpu::Adapter,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,
    pixels: Vec<Vec4>,
    render_surface: Surface<Vec4>,
    pixel_buffer: wgpu::Buffer,
    width: usize,
    height: usize,

    compiler: Compiler<'a>,
    output_sampler: wgpu::Sampler,
    output_texture: wgpu::Texture,
    output_texture_view: wgpu::TextureView,
    output_bind_group_layout: wgpu::BindGroupLayout,
    output_bind_group: wgpu::BindGroup,
    output_pipeline_layout: wgpu::PipelineLayout,
    output_pipeline: wgpu::RenderPipeline,

    meshes: Vec<Mesh>,
    instances: Vec<Instance>,
    materials: Vec<Material>,
    device_materials: Vec<DeviceMaterial>,
    textures: Vec<Texture>,
    bvh: BVH,
    mbvh: MBVH,

    point_lights: Vec<PointLight>,
    spot_lights: Vec<SpotLight>,
    area_lights: Vec<AreaLight>,
    directional_lights: Vec<DirectionalLight>,
}

#[derive(Debug, Copy, Clone)]
enum RayTracerError {
    RequestDeviceError,
}

impl std::error::Error for RayTracerError {}

impl Display for RayTracerError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Could not retrieve valid device.")
    }
}

impl RayTracer<'_> {
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    const PACKET_WIDTH: usize = 4;
    const PACKET_HEIGHT: usize = 1;
}

impl Renderer for RayTracer<'_> {
    fn init<T: HasRawWindowHandle>(
        window: &T,
        width: usize,
        height: usize,
    ) -> Result<Box<Self>, Box<dyn Error>> {
        let surface = wgpu::Surface::create(window);
        let adapter = block_on(wgpu::Adapter::request(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::Default,
                compatible_surface: Some(&surface),
            },
            wgpu::BackendBit::PRIMARY,
        ))
        .unwrap();

        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            extensions: wgpu::Extensions {
                anisotropic_filtering: true,
            },
            limits: wgpu::Limits::default(),
        }));

        let mut pixels = vec![Vec4::zero(); width * height];
        let render_surface = Surface::new(pixels.as_mut_slice(), width, height, 16, 16);

        let descriptor = wgpu::SwapChainDescriptor {
            width: width as u32,
            height: height as u32,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: Self::OUTPUT_FORMAT,
            present_mode: wgpu::PresentMode::Mailbox,
        };

        let swap_chain = device.create_swap_chain(&surface, &descriptor);
        let pixel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            usage: wgpu::BufferUsage::COPY_SRC | wgpu::BufferUsage::MAP_WRITE,
            label: Some("pixel-buffer"),
            size: (width * height * 4) as wgpu::BufferAddress,
        });

        let output_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            compare: wgpu::CompareFunction::Never,
        });

        let output_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("output-texture"),
            size: wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth: 1,
            },
            array_layer_count: 1,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
            format: Self::OUTPUT_FORMAT,
            dimension: wgpu::TextureDimension::D2,
            mip_level_count: 1,
            sample_count: 1,
        });

        let output_texture_view = output_texture.create_default_view();

        let mut compiler = Compiler::new().unwrap();

        let output_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("output-bind-group-layout"),
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        ty: wgpu::BindingType::SampledTexture {
                            dimension: wgpu::TextureViewDimension::D2,
                            component_type: wgpu::TextureComponentType::Uint,
                            multisampled: false,
                        },
                        visibility: wgpu::ShaderStage::FRAGMENT,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                        visibility: wgpu::ShaderStage::FRAGMENT,
                    },
                ],
            });

        let output_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&output_bind_group_layout],
            });

        let output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("output-bind-group"),
            layout: &output_bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&output_texture_view),
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&output_sampler),
                },
            ],
        });

        let vert_shader_source = include_str!("../../../shaders/quad.vert");
        let frag_shader_source = include_str!("../../../shaders/quad.frag");

        let vert_shader = compiler
            .compile_from_string(vert_shader_source, ShaderKind::Vertex)
            .unwrap();
        let frag_shader = compiler
            .compile_from_string(frag_shader_source, ShaderKind::Fragment)
            .unwrap();

        let vert_module = device.create_shader_module(vert_shader.as_slice());
        let frag_module = device.create_shader_module(frag_shader.as_slice());

        let output_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: &output_pipeline_layout,
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &vert_module,
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &frag_module,
                entry_point: "main",
            }),
            rasterization_state: Some(wgpu::RasterizationStateDescriptor {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                depth_bias: 0,
                depth_bias_slope_scale: 0.0,
                depth_bias_clamp: 0.0,
            }),
            primitive_topology: wgpu::PrimitiveTopology::TriangleList,
            color_states: &[wgpu::ColorStateDescriptor {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                color_blend: wgpu::BlendDescriptor::REPLACE,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: None,
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        Ok(Box::new(Self {
            device,
            queue,
            adapter,
            surface,
            swap_chain,
            render_surface,
            pixels,
            pixel_buffer,
            width,
            height,
            compiler,
            output_sampler,
            output_texture,
            output_texture_view,
            output_bind_group_layout,
            output_bind_group,
            output_pipeline_layout,
            output_pipeline,
            meshes: Vec::new(),
            instances: Vec::new(),
            materials: Vec::new(),
            device_materials: Vec::new(),
            textures: Vec::new(),
            bvh: BVH::empty(),
            mbvh: MBVH::empty(),
            point_lights: Vec::new(),
            spot_lights: Vec::new(),
            area_lights: Vec::new(),
            directional_lights: Vec::new(),
        }))
    }

    fn set_mesh(&mut self, id: usize, mesh: &scene::Mesh) {
        if id >= self.meshes.len() {
            self.meshes.push(mesh.clone())
        } else {
            self.meshes[id] = mesh.clone();
        }
    }

    fn set_instance(&mut self, id: usize, instance: &Instance) {
        if id >= self.instances.len() {
            self.instances.push(instance.clone());
        } else {
            self.instances[id] = instance.clone();
        }
    }

    fn set_materials(
        &mut self,
        materials: &[scene::Material],
        device_materials: &[scene::DeviceMaterial],
    ) {
        self.materials = materials.to_vec();
        self.device_materials = device_materials.to_vec();
    }

    fn set_textures(&mut self, textures: &[scene::Texture]) {
        self.textures = textures.to_vec();
    }

    fn synchronize(&mut self) {
        self.meshes.par_iter_mut().for_each(|mesh| {
            if let None = mesh.bvh {
                mesh.construct_bvh();
            }
        });

        let aabbs: Vec<AABB> = self.instances.iter().map(|i| i.bounds()).collect();
        let centers: Vec<Vec3> = aabbs.iter().map(|bb| bb.center()).collect();
        let builder = LocallyOrderedClusteringBuilder::new(aabbs.as_slice(), centers.as_slice());
        self.bvh = builder.build();
        self.mbvh = MBVH::construct(&self.bvh);
    }

    fn render(&mut self, camera: &scene::Camera) {
        self.render_surface.clear();
        let view = camera.get_view();
        let surface = &self.render_surface;

        let intersector = TIntersector::new(
            self.meshes.as_slice(),
            self.instances.as_slice(),
            &self.bvh,
            &self.mbvh,
        );
        let materials = self.materials.as_slice();

        let width = self.width;

        let x_range = match Self::PACKET_WIDTH {
            2 => [0, 0, 1, 1],
            4 => [0, 1, 2, 3],
            _ => [0, 0, 0, 0],
        };

        let y_range = match Self::PACKET_HEIGHT {
            2 => [0, 0, 1, 1],
            4 => [0, 1, 2, 3],
            _ => [0, 0, 0, 0],
        };

        let area_lights = &self.area_lights;

        surface.as_tiles().into_par_iter().for_each(|t| {
            for y in t.y_start..t.y_end {
                let y = y as u32;
                for x in (t.x_start..t.x_end).step_by(4) {
                    let x = x as u32;

                    let xs = [
                        x_range[0] + x,
                        x_range[1] + x,
                        x_range[2] + x,
                        x_range[3] + x,
                    ];

                    let ys = [
                        y_range[0] + y,
                        y_range[1] + y,
                        y_range[2] + y,
                        y_range[3] + y,
                    ];

                    // const USE_PACKETS: bool = false;

                    // if USE_PACKETS {
                    let mut packet = view.generate_ray4(&xs, &ys, width as u32);

                    // let packet: &mut RayPacket4 = &mut packet[p as usize];
                    let (instance_ids, prim_ids) =
                        intersector.intersect4(&mut packet, [constants::DEFAULT_T_MIN; 4]);

                    // let hit = intersector.get_hit_record4(&packet, instance_ids, prim_ids);

                    for i in 0..4 {
                        let prim_id = prim_ids[i];
                        let instance_id = instance_ids[i];

                        surface.draw(
                            x + i as u32,
                            y,
                            if prim_id >= 0 || instance_id >= 0 {
                                // let origin = packet.
                                let ray = packet.ray(i);
                                let hit = intersector.get_hit_record(
                                    ray,
                                    packet.t[i],
                                    instance_id,
                                    prim_id,
                                );
                                let material = &materials[hit.mat_id as usize];
                                let mat_color = Vec4::from(material.color).truncate();

                                if material.is_emissive() {
                                    mat_color.extend(1.0)
                                } else {
                                    let normal: Vec3 = hit.normal.into();
                                    let (origin, direction) = ray.into();
                                    let p = origin + direction * hit.t;
                                    let backward_facing = direction.dot(normal) >= 0.0;

                                    let normal = normal * if backward_facing { -1.0 } else { 1.0 };

                                    let mut light = Vec3::zero();
                                    area_lights.iter().for_each(|al| {
                                        let pos = Vec3::from(al.position);
                                        let l: Vec3 = pos - p;
                                        let dist2 = l.dot(l);
                                        let dist = dist2.sqrt();
                                        let l: Vec3 = l / dist;

                                        let n_dot_l = normal.dot(l);
                                        let ln_dot_l = -Vec3::from(al.normal).dot(l);
                                        if n_dot_l <= 0.0 || ln_dot_l <= 0.0 {
                                            return;
                                        }

                                        if !intersector.occludes(
                                            Ray::new(p.into(), l.into()),
                                            constants::EPSILON,
                                            dist - 2.0 * constants::EPSILON,
                                        ) {
                                            light += mat_color * n_dot_l * ln_dot_l / dist2
                                                * al.area
                                                * al.get_radiance();
                                            // TODO area lights need area available
                                        }
                                    });

                                    light.extend(1.0)
                                }
                            } else {
                                Vec4::zero()
                            },
                        );
                    }
                    // } else {
                    //     for i in 0..4 {
                    //         surface.draw(x + i as u32, y, {
                    //             let ray = view.generate_ray(xs[i], ys[i]);
                    //             let (_, depth) = intersector.depth_test(
                    //                 ray,
                    //                 constants::DEFAULT_T_MIN,
                    //                 constants::DEFAULT_T_MAX,
                    //             );
                    //             if depth == 0 {
                    //                 Vec4::from([0.0; 4])
                    //             } else {
                    //                 let r = (depth as f32).log(2.0) * (1.0 / 16.0);
                    //                 let g = (16 - depth.min(16)) as f32 * (1.0 / 32.0);
                    //                 let b = depth as f32 * (1.0 / 128.0);
                    //                 (r, g, b, 1.0).into()
                    //             }
                    //         });
                    //     }
                    // }
                }
            }
        });

        let pixels = &self.pixels;
        if let Ok(output) = self.swap_chain.get_next_texture() {
            let mapping = self
                .pixel_buffer
                .map_write(0, (self.width * self.height) as wgpu::BufferAddress * 4);
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pixel-buffer-copy"),
                });

            self.device.poll(wgpu::Maintain::Wait);
            if let Ok(mut mapping) = block_on(mapping) {
                let width = self.width;
                let fb_iterator = mapping.as_slice().par_chunks_mut(width * 4).enumerate();

                fb_iterator.for_each(|(y, fb_pixels)| {
                    let line_iterator = fb_pixels.chunks_exact_mut(4).enumerate();
                    let y_offset = y * width;

                    for (x, pixel) in line_iterator {
                        let color: Vec4 = unsafe { *pixels.get_unchecked(x + y_offset) };
                        let color = color.min(Vec4::one()).max(Vec4::zero());

                        let color: Vec4 = color * Vec4::splat(255.0);
                        let red = color.x() as u8;
                        let green = color.y() as u8;
                        let blue = color.z() as u8;
                        pixel.copy_from_slice(&[blue, green, red, 0xff]);
                    }
                });
            } else {
                println!("Could not map pixel buffer.");
            }

            encoder.copy_buffer_to_texture(
                wgpu::BufferCopyView {
                    buffer: &self.pixel_buffer,
                    offset: 0,
                    bytes_per_row: self.width as u32 * 4,
                    rows_per_image: self.height as u32,
                },
                TextureCopyView {
                    texture: &self.output_texture,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                    array_layer: 0,
                    mip_level: 0,
                },
                wgpu::Extent3d {
                    width: self.width as u32,
                    height: self.height as u32,
                    depth: 1,
                },
            );

            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    depth_stencil_attachment: None,
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: &output.view,
                        resolve_target: None,
                        store_op: wgpu::StoreOp::Store,
                        load_op: wgpu::LoadOp::Clear,
                        clear_color: wgpu::Color::BLACK,
                    }],
                });

                render_pass.set_pipeline(&self.output_pipeline);
                render_pass.set_bind_group(0, &self.output_bind_group, &[]);
                render_pass.draw(0..6, 0..1);
            }

            self.queue.submit(&[encoder.finish()]);
        } else {
            println!("Could not get next swap-chain texture.");
        }
    }

    fn resize<T: HasRawWindowHandle>(&mut self, _window: &T, width: usize, height: usize) {
        if self.pixels.len() <= (width * height) {
            let new_size = width * height * 2;
            self.pixels.resize(new_size, Vec4::zero());

            self.pixel_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                usage: wgpu::BufferUsage::COPY_SRC | wgpu::BufferUsage::MAP_WRITE,
                label: Some("pixel-buffer"),
                size: (new_size * 4) as wgpu::BufferAddress,
            });
        }
        self.render_surface =
            Surface::new(&mut self.pixels[0..(width * height)], width, height, 16, 16);

        self.swap_chain = self.device.create_swap_chain(
            &self.surface,
            &wgpu::SwapChainDescriptor {
                width: width as u32,
                height: height as u32,
                present_mode: wgpu::PresentMode::Mailbox,
                format: Self::OUTPUT_FORMAT,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            },
        );

        self.width = width;
        self.height = height;

        self.output_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("output-texture"),
            size: wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth: 1,
            },
            array_layer_count: 1,
            usage: wgpu::TextureUsage::SAMPLED
                | wgpu::TextureUsage::COPY_DST
                | wgpu::TextureUsage::COPY_SRC,
            format: Self::OUTPUT_FORMAT,
            dimension: wgpu::TextureDimension::D2,
            mip_level_count: 1,
            sample_count: 1,
        });
        self.output_texture_view = self.output_texture.create_default_view();

        self.output_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("output-bind-group"),
            layout: &self.output_bind_group_layout,
            bindings: &[
                wgpu::Binding {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.output_texture_view),
                },
                wgpu::Binding {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.output_sampler),
                },
            ],
        });
    }

    fn set_point_lights(&mut self, _changed: &BitVec, lights: &[scene::PointLight]) {
        self.point_lights = Vec::from(lights);
    }

    fn set_spot_lights(&mut self, _changed: &BitVec, lights: &[scene::SpotLight]) {
        self.spot_lights = Vec::from(lights);
    }

    fn set_area_lights(&mut self, _changed: &BitVec, lights: &[scene::AreaLight]) {
        self.area_lights = Vec::from(lights);
    }

    fn set_directional_lights(&mut self, _changed: &BitVec, lights: &[scene::DirectionalLight]) {
        self.directional_lights = Vec::from(lights);
    }

    fn get_settings(&self) -> Vec<scene::renderers::Setting> {
        Vec::new()
    }
    fn set_setting(&mut self, _setting: scene::renderers::Setting) {
        todo!()
    }
}
