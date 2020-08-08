use crate::surface::Surface;

use futures::executor::block_on;
use glam::*;
use rand::{thread_rng, Rng};
use rayon::prelude::*;
use rtbvh::builders::{locb::LocallyOrderedClusteringBuilder, Builder};
use rtbvh::{Bounds, Ray, AABB, BVH, MBVH};

use rfw_system::scene::{
    constants,
    graph::Skin,
    raw_window_handle::HasRawWindowHandle,
    renderers::{RenderMode, Renderer},
    AnimatedMesh, AreaLight, BitVec, DeviceMaterial, DirectionalLight, Instance, Light, Material,
    Mesh, PointLight, SpotLight, TIntersector, Texture,
};
use shared::*;
use std::error::Error;
use std::fmt::{Display, Formatter};
use wgpu::TextureCopyView;

pub struct RayTracer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: wgpu::Adapter,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,
    pixels: Vec<Vec4>,
    render_surface: Surface<Vec4>,
    width: usize,
    height: usize,
    sample_count: usize,

    output_sampler: wgpu::Sampler,
    output_texture: wgpu::Texture,
    output_texture_view: wgpu::TextureView,
    output_bind_group_layout: wgpu::BindGroupLayout,
    output_bind_group: wgpu::BindGroup,
    output_pipeline_layout: wgpu::PipelineLayout,
    output_pipeline: wgpu::RenderPipeline,

    meshes: Vec<Mesh>,
    anim_meshes: Vec<AnimatedMesh>,
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
    skybox: Option<Texture>,
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

impl RayTracer {
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    const PACKET_WIDTH: usize = 4;
    const PACKET_HEIGHT: usize = 1;
}

impl Renderer for RayTracer {
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

        let vert_shader = include_bytes!("../shaders/quad.vert.spv");
        let frag_shader = include_bytes!("../shaders/quad.frag.spv");

        let vert_module = device.create_shader_module(vert_shader.as_quad_bytes());
        let frag_module = device.create_shader_module(frag_shader.as_quad_bytes());

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
            width,
            height,
            sample_count: 0,
            output_sampler,
            output_texture,
            output_texture_view,
            output_bind_group_layout,
            output_bind_group,
            output_pipeline_layout,
            output_pipeline,
            meshes: Vec::new(),
            anim_meshes: Vec::new(),
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
            skybox: None,
        }))
    }

    fn set_mesh(&mut self, id: usize, mesh: &Mesh) {
        while id >= self.meshes.len() {
            self.meshes.push(Mesh::empty());
        }

        self.meshes[id] = mesh.clone();
    }

    fn set_animated_mesh(&mut self, id: usize, mesh: &AnimatedMesh) {
        while id >= self.anim_meshes.len() {
            self.anim_meshes.push(AnimatedMesh::empty());
        }

        self.anim_meshes[id] = mesh.clone();
    }

    fn set_instance(&mut self, id: usize, instance: &Instance) {
        while id >= self.instances.len() {
            self.instances.push(Instance::default());
        }

        self.instances[id] = instance.clone();
    }

    fn set_materials(
        &mut self,
        materials: &[rfw_system::scene::Material],
        device_materials: &[rfw_system::scene::DeviceMaterial],
    ) {
        self.materials = materials.to_vec();
        self.device_materials = device_materials.to_vec();
    }

    fn set_textures(&mut self, textures: &[rfw_system::scene::Texture]) {
        self.textures = textures.to_vec();
    }

    fn synchronize(&mut self) {
        self.meshes.par_iter_mut().for_each(|mesh| {
            if let None = mesh.bvh {
                mesh.construct_bvh();
            }
        });

        let aabbs: Vec<AABB> = self.instances.iter().map(|i| i.bounds()).collect();
        let centers: Vec<Vec3A> = aabbs.iter().map(|bb| bb.center()).collect();
        let builder = LocallyOrderedClusteringBuilder::new(aabbs.as_slice(), centers.as_slice());
        self.bvh = builder.build();
        self.mbvh = MBVH::construct(&self.bvh);
    }

    fn render(&mut self, camera: &rfw_system::scene::Camera, mode: RenderMode) {
        if mode == RenderMode::Reset {
            self.sample_count = 0;
            self.render_surface.clear();
        }

        let view = camera.get_view();
        let surface = &self.render_surface;

        let intersector = TIntersector::new(
            self.meshes.as_slice(),
            self.anim_meshes.as_slice(),
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

        // Initialize weights for pixels
        let new_sample_count = self.sample_count + 1;
        let new_weight = 1.0 / new_sample_count as f32;
        let pixel_weight = if self.sample_count == 0 {
            0.0
        } else {
            self.sample_count as f32 / new_sample_count as f32
        };

        surface.as_tiles().into_par_iter().for_each(|t| {
            let mut rng = thread_rng();

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

                    let r0: [f32; 4] = [rng.gen(), rng.gen(), rng.gen(), rng.gen()];
                    let r1: [f32; 4] = [rng.gen(), rng.gen(), rng.gen(), rng.gen()];
                    let r2: [f32; 4] = [rng.gen(), rng.gen(), rng.gen(), rng.gen()];
                    let r3: [f32; 4] = [rng.gen(), rng.gen(), rng.gen(), rng.gen()];

                    let mut packet = view.generate_lens_ray4(xs, ys, r0, r1, r2, r3, width as u32);
                    let t_min = [constants::DEFAULT_T_MIN; 4];
                    let (instance_ids, prim_ids) = intersector.intersect4(&mut packet, t_min);

                    for i in 0..4 {
                        let prim_id = prim_ids[i];
                        let instance_id = instance_ids[i];
                        let pixel_x = x + i as u32;
                        if let Some(cur_color) = surface.get(pixel_x, y) {
                            let t = packet.t[i];
                            let mut path_length = 0;

                            let color = if t < constants::DEFAULT_T_MAX
                                && (prim_id >= 0 || instance_id >= 0)
                            {
                                path_length += 1;
                                let mut throughput = Vec3A::one();
                                let mut color = Vec3A::zero();
                                let mut ray = packet.ray(i);

                                let mut hit =
                                    intersector.get_hit_record(ray, t, instance_id, prim_id);

                                while path_length < 8 {
                                    let material = &materials[hit.mat_id as usize];
                                    let mat_color = Vec4::from(material.color).truncate();

                                    if material.is_emissive() {
                                        // Only camera rays 'see' lights (TODO: implement multiple importance sampling)
                                        if path_length <= 1 {
                                            color += throughput * mat_color;
                                        }
                                        break;
                                    } else {
                                        let normal: Vec3A = hit.normal.into();
                                        let (origin, direction) = ray.get_vectors::<Vec3A>();
                                        let p: Vec3A = origin + direction * hit.t;
                                        let backward_facing = -direction.dot(normal).signum();
                                        let normal = normal * backward_facing;

                                        let brdf = mat_color * std::f32::consts::FRAC_1_PI;

                                        // Next event estimation
                                        let sampled_light = (rng.gen::<f32>()
                                            * ((area_lights.len().max(1) - 1) as f32))
                                            .round()
                                            as usize;
                                        if let Some(al) = area_lights.get(sampled_light) {
                                            let nee_pdf = 1.0 / area_lights.len() as f32;
                                            let pos = Vec3A::from(al.position);
                                            let l: Vec3A = pos - p;
                                            let dist2 = l.dot(l);
                                            let dist = dist2.sqrt();
                                            let l: Vec3A = l / dist;

                                            let n_dot_l = normal.dot(l);
                                            let ln_dot_l = -Vec3A::from(al.normal).dot(l);
                                            if n_dot_l > 0.0 && ln_dot_l > 0.0 {
                                                if !intersector.occludes(
                                                    Ray::new(p.into(), l.into()),
                                                    constants::EPSILON,
                                                    dist - 2.0 * constants::EPSILON,
                                                ) {
                                                    let solid_angle = ln_dot_l * al.area / dist2;
                                                    color += throughput
                                                        * brdf
                                                        * n_dot_l
                                                        * solid_angle
                                                        * al.get_radiance()
                                                        / nee_pdf;
                                                }
                                            }
                                        }

                                        // Create a cosine-weighted reflection ray
                                        let direction = Self::world_sample_cos(
                                            normal,
                                            rng.gen::<f32>(),
                                            rng.gen::<f32>(),
                                        );
                                        let origin: Vec3A = p;
                                        ray = Ray::new(origin.into(), direction.into());

                                        // Intersect new ray
                                        if let Some(h) = intersector.intersect(
                                            ray,
                                            constants::DEFAULT_T_MIN,
                                            constants::DEFAULT_T_MAX,
                                        ) {
                                            hit = h;
                                            let n_dot_d = normal.dot(direction);
                                            let pdf = n_dot_d * std::f32::consts::FRAC_1_PI;
                                            throughput *= brdf * n_dot_d / pdf;
                                        } else {
                                            break;
                                        }

                                        // Russian roulette
                                        let probability =
                                            throughput.max_element().max(constants::EPSILON);
                                        if rng.gen::<f32>() < probability {
                                            throughput /= probability;
                                        } else {
                                            break;
                                        }
                                    }
                                }

                                color.extend(1.0)
                            } else {
                                Vec4::zero()
                            };

                            surface.draw(
                                pixel_x,
                                y,
                                (*cur_color) * pixel_weight + color * new_weight,
                            );
                        }
                    }
                }
            }
        });

        let pixels = &self.pixels;
        if let Ok(output) = self.swap_chain.get_next_texture() {
            let pixel_buffer = self.device.create_buffer_mapped(&wgpu::BufferDescriptor {
                label: Some("pixel-buffer"),
                usage: wgpu::BufferUsage::COPY_SRC | wgpu::BufferUsage::MAP_WRITE,
                size: (self.width * self.height * 4) as wgpu::BufferAddress,
            });

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pixel-buffer-copy"),
                });

            let color_weight = Vec4::splat(255.0);
            let width = self.width;
            let fb_iterayor = pixel_buffer.data.par_chunks_mut(width * 4).enumerate();

            fb_iterayor.for_each(|(y, fb_pixels)| {
                let line_iterayor = fb_pixels.chunks_exact_mut(4).enumerate();
                let y_offset = y * width;

                for (x, pixel) in line_iterayor {
                    let color: Vec4 = unsafe { *pixels.get_unchecked(x + y_offset) };
                    let color = color.min(Vec4::one()).max(Vec4::zero());

                    let color: Vec4 = color * color_weight;
                    let red = color.x() as u8;
                    let green = color.y() as u8;
                    let blue = color.z() as u8;
                    pixel.copy_from_slice(&[blue, green, red, 0xff]);
                }
            });

            let pixel_buffer = pixel_buffer.finish();

            encoder.copy_buffer_to_texture(
                wgpu::BufferCopyView {
                    buffer: &pixel_buffer,
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

        self.sample_count += 1;
    }

    fn resize<T: HasRawWindowHandle>(&mut self, _window: &T, width: usize, height: usize) {
        let pixel_ref = &mut self.pixels[0..(width * height)];
        self.render_surface = Surface::new(pixel_ref, width, height, 16, 16);
        self.render_surface.clear();
        self.sample_count = 0;

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

    fn set_point_lights(&mut self, _changed: &BitVec, lights: &[rfw_system::scene::PointLight]) {
        self.point_lights = Vec::from(lights);
    }

    fn set_spot_lights(&mut self, _changed: &BitVec, lights: &[rfw_system::scene::SpotLight]) {
        self.spot_lights = Vec::from(lights);
    }

    fn set_area_lights(&mut self, _changed: &BitVec, lights: &[rfw_system::scene::AreaLight]) {
        self.area_lights = Vec::from(lights);
    }

    fn set_directional_lights(
        &mut self,
        _changed: &BitVec,
        lights: &[rfw_system::scene::DirectionalLight],
    ) {
        self.directional_lights = Vec::from(lights);
    }

    fn set_skybox(&mut self, skybox: Texture) {
        self.skybox = Some(skybox);
    }

    fn set_skin(&mut self, id: usize, _skin: &Skin) {}

    fn get_settings(&self) -> Vec<rfw_system::scene::renderers::Setting> {
        Vec::new()
    }
    fn set_setting(&mut self, _setting: rfw_system::scene::renderers::Setting) {
        todo!()
    }
}

impl RayTracer {
    fn create_tangent_space(normal: Vec3A) -> (Vec3A, Vec3A) {
        // *w = v2;
        // *u = normalize(cross(fabs(v2.x) > fabs(v2.y) ? (float3)(0, 1, 0) : (float3)(1, 0, 0), *w));
        // *v = cross(*w, *u);
        // const float3 wi = -rDirection;
        // return normalize((float3)(dot(*u, wi), dot(*v, wi), dot(*w, wi)));

        let t = if normal.x().abs() > normal.y().abs() {
            Vec3A::new(0.0, 1.0, 0.0)
        } else {
            Vec3A::new(1.0, 0.0, 0.0)
        };

        let t = t.cross(normal).normalize();
        let b = normal.cross(t);
        (t, b)
    }

    fn tangent_to_world(sample: Vec3A, normal: Vec3A, tb: (Vec3A, Vec3A)) -> Vec3A {
        let (t, b) = tb;
        sample.x() * b + sample.y() * t + sample.z() * normal
    }

    fn world_to_tangent(sample: Vec3A, normal: Vec3A, tb: (Vec3A, Vec3A)) -> Vec3A {
        let (t, b) = tb;
        Vec3A::new(t.dot(sample), b.dot(sample), normal.dot(sample)).normalize()
    }

    fn sample_hemisphere(r1: f32, r2: f32) -> Vec3A {
        let r = (1.0 - r1 * r1).sqrt();
        let phi = 2.0 * std::f32::consts::PI * r2;
        let (x, y) = phi.sin_cos();
        let (x, y) = (x * r, y * r);
        Vec3A::new(x, y, r1)
    }

    fn world_sample(normal: Vec3A, r1: f32, r2: f32) -> Vec3A {
        let tb = Self::create_tangent_space(normal);
        let sample = Self::sample_hemisphere(r1, r2);
        Self::tangent_to_world(sample, normal, tb)
    }

    fn sample_hemisphere_cos(r1: f32, r2: f32) -> Vec3A {
        let r = r1.sqrt();
        let theta = 2.0 * std::f32::consts::PI * r2;
        let (x, y) = theta.sin_cos();
        let (x, y) = (x * r, y * r);
        Vec3A::new(x, y, (1.0 - r1).sqrt())
    }

    fn world_sample_cos(normal: Vec3A, r1: f32, r2: f32) -> Vec3A {
        let tb = Self::create_tangent_space(normal);
        let sample = Self::sample_hemisphere_cos(r1, r2);
        Self::tangent_to_world(sample, normal, tb)
    }
}
