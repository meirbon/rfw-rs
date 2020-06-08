use crate::surface::Surface;

use futures::executor::block_on;
use glam::*;
use rand::{thread_rng, Rng};
use rayon::prelude::*;
use rtbvh::builders::{locb::LocallyOrderedClusteringBuilder, Builder};
use rtbvh::{Bounds, Ray, AABB, BVH, MBVH};
use scene::renderers::{RenderMode, Renderer};
use scene::{
    constants, AreaLight, BitVec, CameraView, DeviceMaterial, DirectionalLight, HasRawWindowHandle,
    Instance, Light, Material, Mesh, PointLight, SpotLight, TIntersector, Texture,
};
use shared::*;
use std::error::Error;
use std::fmt::{Display, Formatter};

mod bind_group;

#[repr(u32)]
enum IntersectionBindings {
    Output = 0,
    Camera = 1,
    PathStates = 2,
    PathOrigins = 3,
    PathDirections = 4,
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct CameraData {
    view: CameraView,
    path_count: i32,
    extension_id: i32,
    shadow_id: i32,
    width: i32,
    height: i32,
    sample_count: i32,
    clamp_value: f32,
    point_light_count: i32,
    area_light_count: i32,
    spot_light_count: i32,
    directional_light_count: i32,
}

impl CameraData {
    pub fn new(
        view: CameraView,
        width: usize,
        height: usize,
        sample_count: usize,
        pl_count: usize,
        al_count: usize,
        sl_count: usize,
        dl_count: usize,
    ) -> Self {
        Self {
            view,
            path_count: (width * height) as i32,
            extension_id: 0,
            shadow_id: 0,
            width: width as i32,
            height: height as i32,
            sample_count: sample_count as i32,
            clamp_value: 10.0,
            point_light_count: pl_count as i32,
            area_light_count: al_count as i32,
            spot_light_count: sl_count as i32,
            directional_light_count: dl_count as i32,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                (self as *const CameraData) as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }
}

pub struct RayTracer<'a> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter: wgpu::Adapter,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,
    width: usize,
    height: usize,
    sample_count: usize,
    buffer_capacity: usize,

    intersection_bind_group: bind_group::BindGroup,
    intersection_pipeline_layout: wgpu::PipelineLayout,
    intersection_pipeline: wgpu::ComputePipeline,

    compiler: Compiler<'a>,
    output_bind_group: bind_group::BindGroup,
    output_texture: wgpu::Texture,
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
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
    const SWAPCHAIN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
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

        let descriptor = wgpu::SwapChainDescriptor {
            width: width as u32,
            height: height as u32,
            usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            format: Self::SWAPCHAIN_FORMAT,
            present_mode: wgpu::PresentMode::Mailbox,
        };

        let swap_chain = device.create_swap_chain(&surface, &descriptor);

        let output_texture = Self::create_output_texture(&device, width, height);

        let output_bind_group = bind_group::BindGroupBuilder::default()
            .with_binding(bind_group::BindGroupBinding {
                index: 0,
                binding: bind_group::Binding::SampledTexture(
                    output_texture.create_default_view(),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Uint,
                    wgpu::TextureViewDimension::D2,
                ),
                visibility: wgpu::ShaderStage::FRAGMENT,
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: 1,
                binding: bind_group::Binding::Sampler(device.create_sampler(
                    &wgpu::SamplerDescriptor {
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        mipmap_filter: wgpu::FilterMode::Linear,
                        lod_min_clamp: 0.0,
                        lod_max_clamp: 0.0,
                        compare: wgpu::CompareFunction::Never,
                    },
                )),
                visibility: wgpu::ShaderStage::FRAGMENT,
            })
            .unwrap()
            .build(&device);

        let mut compiler = CompilerBuilder::new().build().unwrap();

        let output_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&output_bind_group.layout],
            });

        let vert_shader_source = include_str!("../../../../shaders/quad.vert");
        let frag_shader_source = include_str!("../../../../shaders/quad.frag");

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

        let intersection_bind_group = bind_group::BindGroupBuilder::default()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::Output as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageTexture(
                    output_texture.create_default_view(),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Uint,
                    wgpu::TextureViewDimension::D2,
                ),
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::Camera as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::UniformBuffer(
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("camera-view-buffer"),
                        size: std::mem::size_of::<CameraData>().next_power_of_two()
                            as wgpu::BufferAddress,
                        usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::MAP_WRITE,
                    }),
                    0..std::mem::size_of::<CameraData>() as wgpu::BufferAddress,
                ),
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::PathStates as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageBuffer(
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("states-buffer"),
                        usage: wgpu::BufferUsage::STORAGE,
                        size: (width * height * std::mem::size_of::<[f32; 4]>())
                            as wgpu::BufferAddress,
                    }),
                    0..(width * height * std::mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                ),
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::PathOrigins as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageBuffer(
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("states-buffer"),
                        usage: wgpu::BufferUsage::STORAGE,
                        size: (width * height * std::mem::size_of::<[f32; 4]>())
                            as wgpu::BufferAddress,
                    }),
                    0..(width * height * std::mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                ),
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::PathDirections as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageBuffer(
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("states-buffer"),
                        usage: wgpu::BufferUsage::STORAGE,
                        size: (width * height * std::mem::size_of::<[f32; 4]>())
                            as wgpu::BufferAddress,
                    }),
                    0..(width * height * std::mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                ),
            })
            .unwrap()
            .build(&device);

        let intersection_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[&intersection_bind_group.layout],
            });

        let compute_module = compiler
            .compile_from_file("renderers/gpu-rt/shaders/ray_gen.comp", ShaderKind::Compute)
            .unwrap();
        let compute_module = device.create_shader_module(compute_module.as_slice());
        let intersection_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                layout: &intersection_pipeline_layout,
                compute_stage: wgpu::ProgrammableStageDescriptor {
                    entry_point: "main",
                    module: &compute_module,
                },
            });

        Ok(Box::new(Self {
            device,
            queue,
            adapter,
            surface,
            swap_chain,
            intersection_bind_group,
            intersection_pipeline_layout,
            intersection_pipeline,
            width,
            height,
            sample_count: 0,
            buffer_capacity: width * height,
            compiler,
            output_bind_group,
            output_texture,
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

    fn set_mesh(&mut self, id: usize, mesh: &Mesh) {
        while id >= self.meshes.len() {
            self.meshes.push(Mesh::empty());
        }

        self.meshes[id] = mesh.clone();
    }

    fn set_instance(&mut self, id: usize, instance: &Instance) {
        while id >= self.instances.len() {
            self.instances.push(Instance::default());
        }

        self.instances[id] = instance.clone();
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

    fn render(&mut self, camera: &scene::Camera, mode: RenderMode) {
        if mode == RenderMode::Reset {
            self.sample_count = 0;
        }

        let view = camera.get_view();
        let mut camera_data = CameraData::new(
            view,
            self.width,
            self.height,
            self.sample_count,
            self.point_lights.len(),
            self.area_lights.len(),
            self.spot_lights.len(),
            self.directional_lights.len(),
        );

        self.write_camera_data(&camera_data);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-command-buffer"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass();
            compute_pass.set_pipeline(&self.intersection_pipeline);
            compute_pass.set_bind_group(
                0,
                self.intersection_bind_group.as_bind_group(&self.device),
                &[],
            );
            compute_pass.dispatch(
                (self.width as f32 / 16.0).ceil() as u32,
                (self.height as f32 / 16.0).ceil() as u32,
                1,
            );
        }

        if let Ok(output) = self.swap_chain.get_next_texture() {
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
                render_pass.set_bind_group(
                    0,
                    self.output_bind_group.as_bind_group(&self.device),
                    &[],
                );
                render_pass.draw(0..6, 0..1);
            }

            self.queue.submit(&[encoder.finish()]);
        } else {
            println!("Could not get next swap-chain texture.");
        }

        self.sample_count += 1;
    }

    fn resize<T: HasRawWindowHandle>(&mut self, _window: &T, width: usize, height: usize) {
        if (width * height) > self.buffer_capacity {
            self.buffer_capacity = ((width * height) as f64 * 1.5).ceil() as usize;
            self.intersection_bind_group
                .bind(
                    IntersectionBindings::PathStates as u32,
                    bind_group::Binding::WriteStorageBuffer(
                        self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("states-buffer"),
                            usage: wgpu::BufferUsage::STORAGE,
                            size: (self.buffer_capacity * std::mem::size_of::<[f32; 4]>())
                                as wgpu::BufferAddress,
                        }),
                        0..(self.buffer_capacity * std::mem::size_of::<[f32; 4]>())
                            as wgpu::BufferAddress,
                    ),
                )
                .unwrap();
            self.intersection_bind_group
                .bind(
                    IntersectionBindings::PathOrigins as u32,
                    bind_group::Binding::WriteStorageBuffer(
                        self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("states-buffer"),
                            usage: wgpu::BufferUsage::STORAGE,
                            size: (self.buffer_capacity * std::mem::size_of::<[f32; 4]>())
                                as wgpu::BufferAddress,
                        }),
                        0..(self.buffer_capacity * std::mem::size_of::<[f32; 4]>())
                            as wgpu::BufferAddress,
                    ),
                )
                .unwrap();
            self.intersection_bind_group
                .bind(
                    IntersectionBindings::PathDirections as u32,
                    bind_group::Binding::WriteStorageBuffer(
                        self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("states-buffer"),
                            usage: wgpu::BufferUsage::STORAGE,
                            size: (self.buffer_capacity * std::mem::size_of::<[f32; 4]>())
                                as wgpu::BufferAddress,
                        }),
                        0..(self.buffer_capacity * std::mem::size_of::<[f32; 4]>())
                            as wgpu::BufferAddress,
                    ),
                )
                .unwrap();
        }

        self.sample_count = 0;

        self.swap_chain = self.device.create_swap_chain(
            &self.surface,
            &wgpu::SwapChainDescriptor {
                width: width as u32,
                height: height as u32,
                present_mode: wgpu::PresentMode::Mailbox,
                format: Self::SWAPCHAIN_FORMAT,
                usage: wgpu::TextureUsage::OUTPUT_ATTACHMENT,
            },
        );

        self.width = width;
        self.height = height;

        self.output_texture = Self::create_output_texture(&self.device, width, height);

        self.output_bind_group
            .bind(
                0,
                bind_group::Binding::SampledTexture(
                    self.output_texture.create_default_view(),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Uint,
                    wgpu::TextureViewDimension::D2,
                ),
            )
            .unwrap();

        self.intersection_bind_group
            .bind(
                IntersectionBindings::Output as u32,
                bind_group::Binding::WriteStorageTexture(
                    self.output_texture.create_default_view(),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Uint,
                    wgpu::TextureViewDimension::D2,
                ),
            )
            .unwrap();
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

impl<'a> RayTracer<'a> {
    fn create_output_texture(device: &wgpu::Device, width: usize, height: usize) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("output-texture"),
            size: wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth: 1,
            },
            array_layer_count: 1,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::STORAGE,
            format: Self::OUTPUT_FORMAT,
            dimension: wgpu::TextureDimension::D2,
            mip_level_count: 1,
            sample_count: 1,
        })
    }

    fn write_camera_data(&mut self, camera_data: &CameraData) {
        if let Some(binding) = self
            .intersection_bind_group
            .get_mut(IntersectionBindings::Camera as u32)
        {
            match &mut binding.binding {
                bind_group::Binding::UniformBuffer(buffer, range) => {
                    let mapping = buffer.map_write(range.start, range.end);
                    self.device.poll(wgpu::Maintain::Wait);
                    let mapping = futures::executor::block_on(mapping);
                    if let Ok(mut mapping) = mapping {
                        mapping.as_slice().copy_from_slice(camera_data.as_bytes());
                    }
                }
                _ => {}
            }
        }
    }
}
