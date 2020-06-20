use crate::renderer::mem::ManagedBuffer;
use futures::executor::block_on;
use glam::*;
use rayon::prelude::*;
use rtbvh::builders::{binned_sah::BinnedSahBuilder, Builder};
use rtbvh::{BVHNode, Bounds, MBVHNode, AABB, BVH, MBVH};
use scene::renderers::{RenderMode, Renderer};
use scene::{
    raw_window_handle::HasRawWindowHandle, AreaLight, BitVec, CameraView, DeviceMaterial,
    DirectionalLight, Instance, Material, Mesh, PointLight, RTTriangle, SpotLight, Texture,
};
use shared::*;
use std::error::Error;
use std::fmt::{Display, Formatter};

mod bind_group;
mod mem;

#[repr(u32)]
enum IntersectionBindings {
    Output = 0,
    Camera = 1,
    PathStates = 2,
    PathOrigins = 3,
    PathDirections = 4,
    AccumulationBuffer = 5,
}

#[repr(u32)]
enum TopBindings {
    InstanceDescriptors = 0,
    TopInstanceIndices = 1,
    TopBVHNodes = 2,
    TopMBVHNodes = 3,
    Materials = 4,
}

#[repr(u32)]
enum MeshBindings {
    PrimIndices = 0,
    BVHNodes = 1,
    MBVHNodes = 2,
    Triangles = 3,
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct CameraData {
    pub pos: Vec4,
    pub right: Vec4,
    pub up: Vec4,
    pub p1: Vec4,

    pub lens_size: f32,
    pub spread_angle: f32,
    pub epsilon: f32,
    pub inv_width: f32,

    pub inv_height: f32,
    pub path_count: i32,
    pub extension_id: i32,
    pub shadow_id: i32,

    pub width: i32,
    pub height: i32,
    pub sample_count: i32,
    pub clamp_value: f32,

    pub point_light_count: i32,
    pub area_light_count: i32,
    pub spot_light_count: i32,
    pub directional_light_count: i32,
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
            pos: view.pos.extend(1.0),
            right: view.right.extend(1.0),
            up: view.up.extend(1.0),
            p1: view.p1.extend(1.0),
            lens_size: view.lens_size,
            spread_angle: view.spread_angle,
            epsilon: view.epsilon,
            inv_width: view.inv_width,
            inv_height: view.inv_height,
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

#[derive(Debug, Copy, Clone)]
pub struct GPUMeshData {
    pub bvh_offset: u32,
    pub bvh_nodes: u32,
    pub triangle_offset: u32,
    pub triangles: u32,
    pub prim_index_offset: u32,
    pub mbvh_offset: u32,
}

impl Default for GPUMeshData {
    fn default() -> Self {
        Self {
            bvh_offset: 0,
            bvh_nodes: 0,
            triangle_offset: 0,
            triangles: 0,
            prim_index_offset: 0,
            mbvh_offset: 0,
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct GPUInstanceData {
    pub bvh_offset: u32,
    pub mbvh_offset: u32,
    pub triangle_offset: u32,
    pub prim_index_offset: u32,
    _dummy0: Vec4,
    _dummy1: Vec4,
    _dummy2: Vec4,

    pub matrix: Mat4,
    pub inverse: Mat4,
    pub normal: Mat4,
}

impl Default for GPUInstanceData {
    fn default() -> Self {
        Self {
            matrix: Mat4::identity(),
            inverse: Mat4::identity(),
            normal: Mat4::identity(),
            bvh_offset: 0,
            mbvh_offset: 0,
            triangle_offset: 0,
            prim_index_offset: 0,
            _dummy0: Vec4::zero(),
            _dummy1: Vec4::zero(),
            _dummy2: Vec4::zero(),
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
    shade_pipeline: wgpu::ComputePipeline,
    blit_pipeline: wgpu::ComputePipeline,

    mesh_bind_group: wgpu::BindGroup,
    mesh_bind_group_layout: wgpu::BindGroupLayout,

    top_bind_group: wgpu::BindGroup,
    top_bind_group_layout: wgpu::BindGroupLayout,
    top_bvh_buffer: ManagedBuffer<BVHNode>,
    top_mbvh_buffer: ManagedBuffer<MBVHNode>,
    top_indices: ManagedBuffer<u32>,

    compiler: Compiler<'a>,
    output_bind_group: bind_group::BindGroup,
    output_texture: wgpu::Texture,
    output_pipeline_layout: wgpu::PipelineLayout,
    output_pipeline: wgpu::RenderPipeline,
    accumulation_texture: wgpu::Texture,

    meshes: Vec<Mesh>,
    meshes_changed: BitVec,
    meshes_gpu_data: Vec<GPUMeshData>,
    meshes_bvh_buffer: ManagedBuffer<BVHNode>,
    meshes_mbvh_buffer: ManagedBuffer<MBVHNode>,
    meshes_prim_indices: ManagedBuffer<u32>,
    mesh_prim_index_counter: usize,
    mesh_bvh_index_counter: usize,
    mesh_mbvh_index_counter: usize,

    instances: Vec<Instance>,
    instances_buffer: ManagedBuffer<GPUInstanceData>,
    triangles_buffer: ManagedBuffer<RTTriangle>,
    triangles_index_counter: usize,

    materials: Vec<Material>,
    textures: Vec<Texture>,

    materials_buffer: ManagedBuffer<DeviceMaterial>,

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
    const ACC_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
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
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
            },
            wgpu::BackendBit::PRIMARY,
        ))
        .unwrap();

        println!("Picked render device: {}", adapter.get_info().name);

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
        let accumulation_texture = Self::create_output_texture(&device, width, height);

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

        let mesh_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mesh-bind-group-layout"),
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: MeshBindings::PrimIndices as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: MeshBindings::BVHNodes as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: MeshBindings::MBVHNodes as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: MeshBindings::Triangles as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                ],
            });

        let mut compiler = CompilerBuilder::new()
            // .with_opt_level(OptimizationLevel::Performance)
            .build()
            .unwrap();

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
                    wgpu::TextureComponentType::Float,
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
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::AccumulationBuffer as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageTexture(
                    accumulation_texture.create_default_view(),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Float,
                    wgpu::TextureViewDimension::D2,
                ),
            })
            .unwrap()
            .build(&device);

        let top_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("top-bind-group-layout"),
                bindings: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: TopBindings::InstanceDescriptors as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: TopBindings::TopInstanceIndices as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: TopBindings::TopBVHNodes as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: TopBindings::TopMBVHNodes as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: TopBindings::Materials as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            readonly: true,
                        },
                    },
                ],
            });

        let intersection_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                bind_group_layouts: &[
                    &intersection_bind_group.layout,
                    &mesh_bind_group_layout,
                    &top_bind_group_layout,
                ],
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

        let compute_module = compiler
            .compile_from_file("renderers/gpu-rt/shaders/shade.comp", ShaderKind::Compute)
            .unwrap();
        let compute_module = device.create_shader_module(compute_module.as_slice());
        let shade_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            layout: &intersection_pipeline_layout,
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &compute_module,
            },
        });

        let compute_module = compiler
            .compile_from_file("renderers/gpu-rt/shaders/blit.comp", ShaderKind::Compute)
            .unwrap();
        let compute_module = device.create_shader_module(compute_module.as_slice());
        let blit_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            layout: &intersection_pipeline_layout,
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &compute_module,
            },
        });

        let meshes_bvh_buffer = ManagedBuffer::new(&device, 65536, wgpu::BufferUsage::STORAGE_READ);

        let meshes_mbvh_buffer =
            ManagedBuffer::new(&device, 65536, wgpu::BufferUsage::STORAGE_READ);

        let meshes_prim_indices =
            ManagedBuffer::new(&device, 65536, wgpu::BufferUsage::STORAGE_READ);

        let triangles_buffer = ManagedBuffer::new(&device, 65536, wgpu::BufferUsage::STORAGE_READ);

        let instances_buffer = ManagedBuffer::new(&device, 2048, wgpu::BufferUsage::STORAGE_READ);

        let mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            bindings: &[
                meshes_prim_indices.as_binding(MeshBindings::PrimIndices as u32),
                meshes_bvh_buffer.as_binding(MeshBindings::BVHNodes as u32),
                meshes_mbvh_buffer.as_binding(MeshBindings::MBVHNodes as u32),
                triangles_buffer.as_binding(MeshBindings::Triangles as u32),
            ],
            label: Some("mesh-bind-group"),
            layout: &mesh_bind_group_layout,
        });

        let top_bvh_buffer = ManagedBuffer::new(&device, 1024, wgpu::BufferUsage::STORAGE_READ);
        let top_mbvh_buffer = ManagedBuffer::new(&device, 512, wgpu::BufferUsage::STORAGE_READ);
        let top_indices = ManagedBuffer::new(&device, 1024, wgpu::BufferUsage::STORAGE_READ);
        let materials_buffer = ManagedBuffer::new(&device, 32, wgpu::BufferUsage::STORAGE_READ);

        let top_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("top-bind-group"),
            layout: &top_bind_group_layout,
            bindings: &[
                instances_buffer.as_binding(TopBindings::InstanceDescriptors as u32),
                top_bvh_buffer.as_binding(TopBindings::TopBVHNodes as u32),
                top_mbvh_buffer.as_binding(TopBindings::TopMBVHNodes as u32),
                top_indices.as_binding(TopBindings::TopInstanceIndices as u32),
                materials_buffer.as_binding(TopBindings::Materials as u32),
            ],
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
            shade_pipeline,
            blit_pipeline,
            mesh_bind_group,
            mesh_bind_group_layout,
            top_bind_group,
            top_bind_group_layout,
            top_bvh_buffer,
            top_mbvh_buffer,
            top_indices,
            width,
            height,
            sample_count: 0,
            buffer_capacity: width * height,
            compiler,
            output_bind_group,
            output_texture,
            output_pipeline_layout,
            output_pipeline,
            accumulation_texture,
            meshes: Vec::new(),
            meshes_changed: Default::default(),
            meshes_gpu_data: vec![],
            meshes_bvh_buffer,
            meshes_mbvh_buffer,
            meshes_prim_indices,
            mesh_prim_index_counter: 0,
            mesh_bvh_index_counter: 0,
            mesh_mbvh_index_counter: 0,
            instances: Vec::new(),
            instances_buffer,
            triangles_buffer,
            triangles_index_counter: 0,
            materials: Vec::new(),
            textures: Vec::new(),
            materials_buffer,
            bvh: BVH::empty(),
            mbvh: MBVH::empty(),
            point_lights: Vec::new(),
            spot_lights: Vec::new(),
            area_lights: Vec::new(),
            directional_lights: Vec::new(),
        }))
    }

    fn set_mesh(&mut self, id: usize, mesh: &Mesh) {
        if id >= self.meshes.len() {
            self.meshes.push(Mesh::empty());
            self.meshes_changed.push(true);
        }

        self.meshes[id] = mesh.clone();
        self.meshes_changed.set(id, true);
    }

    fn set_instance(&mut self, id: usize, instance: &Instance) {
        if id >= self.instances.len() {
            self.instances.push(Instance::default());
        }

        self.instances[id] = instance.clone();
    }

    fn set_materials(
        &mut self,
        _materials: &[scene::Material],
        device_materials: &[scene::DeviceMaterial],
    ) {
        self.materials_buffer
            .resize(&self.device, device_materials.len());
        self.materials_buffer.copy_from_slice(device_materials);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("material-copy"),
            });
        self.materials_buffer.update(&self.device, &mut encoder);
        self.queue.submit(&[encoder.finish()]);
        self.top_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("top-bind-group"),
            layout: &self.top_bind_group_layout,
            bindings: &[
                self.instances_buffer
                    .as_binding(TopBindings::InstanceDescriptors as u32),
                self.top_bvh_buffer
                    .as_binding(TopBindings::TopBVHNodes as u32),
                self.top_mbvh_buffer
                    .as_binding(TopBindings::TopMBVHNodes as u32),
                self.top_indices
                    .as_binding(TopBindings::TopInstanceIndices as u32),
                self.materials_buffer
                    .as_binding(TopBindings::Materials as u32),
            ],
        });
    }

    fn set_textures(&mut self, textures: &[scene::Texture]) {
        self.textures = textures.to_vec();
    }

    fn synchronize(&mut self) {
        if self.meshes.is_empty() {
            return;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("synchronize-command"),
            });

        let meshes_changed = &self.meshes_changed;
        let constructed: usize = self
            .meshes
            .iter_mut()
            .enumerate()
            .par_bridge()
            .map(|(i, mesh)| {
                if mesh.bvh.is_none() || *meshes_changed.get(i).unwrap() {
                    mesh.construct_bvh();
                    1
                } else {
                    0
                }
            })
            .sum();

        self.meshes_changed.set_all(false);

        if constructed != 0 {
            self.triangles_index_counter = 0;
            self.mesh_bvh_index_counter = 0;
            self.mesh_mbvh_index_counter = 0;
            self.mesh_prim_index_counter = 0;

            self.meshes_gpu_data
                .resize(self.meshes.len(), GPUMeshData::default());
            for i in 0..self.meshes.len() {
                let mesh = &self.meshes[i];
                let start_triangle = self.triangles_index_counter;
                let start_bvh_node = self.mesh_bvh_index_counter;
                let start_mbvh_node = self.mesh_mbvh_index_counter;
                let start_prim_index = self.mesh_prim_index_counter;

                self.meshes_gpu_data[i].bvh_nodes = mesh.bvh.as_ref().unwrap().nodes.len() as u32;
                self.meshes_gpu_data[i].bvh_offset = start_bvh_node as u32;
                self.meshes_gpu_data[i].mbvh_offset = start_mbvh_node as u32;
                self.meshes_gpu_data[i].triangles = mesh.triangles.len() as u32;
                self.meshes_gpu_data[i].triangle_offset = start_triangle as u32;
                self.meshes_gpu_data[i].prim_index_offset = start_prim_index as u32;

                self.triangles_index_counter += mesh.triangles.len();
                self.mesh_bvh_index_counter += mesh.bvh.as_ref().unwrap().nodes.len();
                self.mesh_mbvh_index_counter += mesh.mbvh.as_ref().unwrap().m_nodes.len();
                self.mesh_prim_index_counter += mesh.bvh.as_ref().unwrap().prim_indices.len();
            }

            self.meshes_prim_indices
                .resize(&self.device, self.mesh_prim_index_counter);
            self.meshes_bvh_buffer
                .resize(&self.device, self.mesh_bvh_index_counter);
            self.meshes_mbvh_buffer
                .resize(&self.device, self.mesh_bvh_index_counter);
            self.triangles_buffer
                .resize(&self.device, self.triangles_index_counter);

            for i in 0..self.meshes.len() {
                let mesh = &self.meshes[i];
                let offset_data = &self.meshes_gpu_data[i];

                self.meshes_prim_indices.copy_from_slice_offset(
                    mesh.bvh.as_ref().unwrap().prim_indices.as_slice(),
                    offset_data.prim_index_offset as usize,
                );

                self.meshes_bvh_buffer.copy_from_slice_offset(
                    mesh.bvh.as_ref().unwrap().nodes.as_slice(),
                    offset_data.bvh_offset as usize,
                );

                self.meshes_mbvh_buffer.copy_from_slice_offset(
                    mesh.mbvh.as_ref().unwrap().m_nodes.as_slice(),
                    offset_data.mbvh_offset as usize,
                );

                self.triangles_buffer.copy_from_slice_offset(
                    mesh.triangles.as_slice(),
                    offset_data.triangle_offset as usize,
                );
            }

            self.meshes_prim_indices.update(&self.device, &mut encoder);
            self.meshes_bvh_buffer.update(&self.device, &mut encoder);
            self.meshes_mbvh_buffer.update(&self.device, &mut encoder);
            self.triangles_buffer.update(&self.device, &mut encoder);

            self.mesh_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                bindings: &[
                    self.meshes_prim_indices
                        .as_binding(MeshBindings::PrimIndices as u32),
                    self.meshes_bvh_buffer
                        .as_binding(MeshBindings::BVHNodes as u32),
                    self.meshes_mbvh_buffer
                        .as_binding(MeshBindings::MBVHNodes as u32),
                    self.triangles_buffer
                        .as_binding(MeshBindings::Triangles as u32),
                ],
                label: Some("mesh-bind-group"),
                layout: &self.mesh_bind_group_layout,
            });
        }

        self.instances_buffer
            .resize(&self.device, self.instances.len());
        let mesh_data = self.meshes_gpu_data.as_slice();
        let instances = self.instances.as_slice();
        let aabbs: Vec<AABB> = self.instances.iter().map(|i| i.bounds()).collect();

        let centers: Vec<Vec3> = aabbs.iter().map(|bb| bb.center()).collect();
        let builder = BinnedSahBuilder::new(aabbs.as_slice(), centers.as_slice());
        self.bvh = builder.build();
        self.mbvh = MBVH::construct(&self.bvh);

        self.top_bvh_buffer
            .resize(&self.device, self.bvh.nodes.len());
        self.top_mbvh_buffer
            .resize(&self.device, self.mbvh.nodes.len());
        self.top_indices
            .resize(&self.device, self.bvh.prim_indices.len());
        self.instances_buffer.as_mut_slice()[0..self.instances.len()]
            .iter_mut()
            .enumerate()
            .for_each(|(i, inst)| {
                let mesh_data = &mesh_data[instances[i].get_hit_id()];
                inst.prim_index_offset = mesh_data.prim_index_offset;
                inst.triangle_offset = mesh_data.triangle_offset;
                inst.bvh_offset = mesh_data.bvh_offset;
                inst.mbvh_offset = mesh_data.mbvh_offset;
                inst.matrix = instances[i].get_transform();
                inst.inverse = instances[i].get_inverse_transform();
                inst.normal = instances[i].get_normal_transform();
            });

        self.top_bvh_buffer
            .copy_from_slice(self.bvh.nodes.as_slice());
        self.top_mbvh_buffer
            .copy_from_slice(self.mbvh.m_nodes.as_slice());
        self.top_indices
            .copy_from_slice(self.bvh.prim_indices.as_slice());

        self.top_bvh_buffer.update(&self.device, &mut encoder);
        self.top_mbvh_buffer.update(&self.device, &mut encoder);
        self.top_indices.update(&self.device, &mut encoder);
        self.instances_buffer.update(&self.device, &mut encoder);

        self.queue.submit(&[encoder.finish()]);
        self.top_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("top-bind-group"),
            layout: &self.top_bind_group_layout,
            bindings: &[
                self.instances_buffer
                    .as_binding(TopBindings::InstanceDescriptors as u32),
                self.top_bvh_buffer
                    .as_binding(TopBindings::TopBVHNodes as u32),
                self.top_mbvh_buffer
                    .as_binding(TopBindings::TopMBVHNodes as u32),
                self.top_indices
                    .as_binding(TopBindings::TopInstanceIndices as u32),
                self.materials_buffer
                    .as_binding(TopBindings::Materials as u32),
            ],
        });
    }

    fn render(&mut self, camera: &scene::Camera, mode: RenderMode) {
        if self.meshes.is_empty() {
            return;
        }

        if mode == RenderMode::Reset {
            self.sample_count = 0;
        }

        let view = camera.get_view();
        let camera_data = CameraData::new(
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
            let bind_group = self.intersection_bind_group.as_bind_group(&self.device);
            // Generate & Intersect
            let mut compute_pass = encoder.begin_compute_pass();
            compute_pass.set_pipeline(&self.intersection_pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);
            compute_pass.set_bind_group(1, &self.mesh_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.top_bind_group, &[]);
            compute_pass.dispatch(
                (self.width as f32 / 16.0).ceil() as u32,
                (self.height as f32 / 16.0).ceil() as u32,
                1,
            );

            // Shade
            compute_pass.set_pipeline(&self.shade_pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);
            compute_pass.set_bind_group(1, &self.mesh_bind_group, &[]);
            compute_pass.set_bind_group(2, &self.top_bind_group, &[]);
            compute_pass.dispatch(
                ((self.width * self.height) as f32 / 16.0).ceil() as u32,
                1,
                1,
            );

            // Blit
            compute_pass.set_pipeline(&self.blit_pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);
            compute_pass.dispatch(
                (self.width as f32 / 16.0).ceil() as u32,
                (self.height as f32 / 4.0).ceil() as u32,
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
        self.accumulation_texture = Self::create_accumulation_texture(&self.device, width, height);

        self.output_bind_group
            .bind(
                IntersectionBindings::Output as u32,
                bind_group::Binding::SampledTexture(
                    self.output_texture.create_default_view(),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Float,
                    wgpu::TextureViewDimension::D2,
                ),
            )
            .unwrap();
        self.intersection_bind_group
            .bind(
                IntersectionBindings::AccumulationBuffer as u32,
                bind_group::Binding::WriteStorageTexture(
                    self.accumulation_texture.create_default_view(),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Float,
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

    fn create_accumulation_texture(
        device: &wgpu::Device,
        width: usize,
        height: usize,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("output-texture"),
            size: wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth: 1,
            },
            array_layer_count: 1,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::STORAGE,
            format: Self::ACC_FORMAT,
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
