use crate::mem::ManagedBuffer;
use futures::executor::block_on;
use glam::*;
use rayon::prelude::*;
use rtbvh::builders::{binned_sah::BinnedSahBuilder, Builder};
use rtbvh::{BVHNode, Bounds, MBVHNode, AABB, BVH, MBVH};

use rfw_scene::{
    graph::Skin,
    raw_window_handle::HasRawWindowHandle,
    renderers::{RenderMode, Renderer},
    AnimatedMesh, AreaLight, BitVec, CameraView, ChangedIterator, DeviceMaterial, DirectionalLight,
    Instance, Mesh, ObjectRef, PointLight, RTTriangle, SpotLight, Texture, TrackedStorage,
};
use shared::*;
use std::borrow::Cow;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::num::NonZeroU32;

mod bind_group;
mod blue_noise;
mod mem;

#[repr(u32)]
enum IntersectionBindings {
    Output = 0,
    Camera = 1,
    PathStates = 2,
    AccumulationBuffer = 3,
    PotentialContributions = 4,
    Skybox = 5,
}

#[repr(u32)]
enum TopBindings {
    InstanceDescriptors = 0,
    TopInstanceIndices = 1,
    TopBVHNodes = 2,
    TopMBVHNodes = 3,
}

#[repr(u32)]
enum MaterialBindings {
    Materials = 0,
    Textures = 1,
    TextureSampler = 2,
    InstanceDescriptors = 3,
}

#[repr(u32)]
enum MeshBindings {
    PrimIndices = 0,
    BVHNodes = 1,
    MBVHNodes = 2,
    Triangles = 3,
}

#[repr(u32)]
enum LightBindings {
    PointLights = 0,
    SpotLights = 1,
    AreaLights = 2,
    DirectionalLights = 3,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum PassType {
    Primary,
    Secondary,
    Shadow,
}

impl std::fmt::Display for PassType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PassType({})",
            match self {
                PassType::Primary => "Primary",
                PassType::Secondary => "Secondary",
                PassType::Shadow => "Shadow",
            }
        )
    }
}

#[derive(Debug, Clone)]
enum AnimMesh {
    None,
    Skinned {
        original: AnimatedMesh,
        skinned: Mesh,
    },
    Regular(AnimatedMesh),
}

impl Default for AnimMesh {
    fn default() -> Self {
        Self::None
    }
}

#[allow(dead_code)]
impl AnimMesh {
    pub fn set_skinned_mesh(&mut self, mesh: Mesh) {
        *self = match self {
            AnimMesh::None => panic!("This should not happen"),
            AnimMesh::Skinned { original, .. } => AnimMesh::Skinned {
                original: original.clone(),
                skinned: mesh,
            },
            AnimMesh::Regular(original) => AnimMesh::Skinned {
                original: original.clone(),
                skinned: mesh,
            },
        }
    }

    fn consume(self) -> AnimatedMesh {
        match self {
            AnimMesh::None => AnimatedMesh::default(),
            AnimMesh::Skinned { original, .. } => original,
            AnimMesh::Regular(original) => original,
        }
    }

    pub fn as_ref(&self) -> &AnimatedMesh {
        match self {
            AnimMesh::None => panic!("This should not happen"),
            AnimMesh::Skinned { original, .. } => original,
            AnimMesh::Regular(original) => original,
        }
    }

    pub fn as_mut(&mut self) -> &mut AnimatedMesh {
        match self {
            AnimMesh::None => panic!("This should not happen"),
            AnimMesh::Skinned { original, .. } => original,
            AnimMesh::Regular(original) => original,
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct CameraData {
    pub pos: [f32; 3],
    pub path_length: i32,
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
            pos: view.pos.into(),
            path_length: 0,
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

pub struct RayTracer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface,
    swap_chain: wgpu::SwapChain,
    width: usize,
    height: usize,
    sample_count: usize,
    buffer_capacity: usize,

    camera_read_buffer: wgpu::Buffer,

    intersection_bind_group: bind_group::BindGroup,
    intersection_pipeline: wgpu::ComputePipeline,

    extend_pipeline: wgpu::ComputePipeline,
    shadow_pipeline: wgpu::ComputePipeline,

    shade_pipeline: wgpu::ComputePipeline,
    shade_pipeline_layout: wgpu::PipelineLayout,
    blit_pipeline: wgpu::ComputePipeline,
    blit_pipeline_layout: wgpu::PipelineLayout,

    mesh_bind_group: wgpu::BindGroup,
    mesh_bind_group_layout: wgpu::BindGroupLayout,

    top_bind_group: wgpu::BindGroup,
    top_bind_group_layout: wgpu::BindGroupLayout,
    material_bind_group: wgpu::BindGroup,
    material_bind_group_layout: wgpu::BindGroupLayout,

    top_bvh_buffer: ManagedBuffer<BVHNode>,
    top_mbvh_buffer: ManagedBuffer<MBVHNode>,
    top_indices: ManagedBuffer<u32>,

    output_bind_group: bind_group::BindGroup,
    output_texture: wgpu::Texture,
    output_pipeline: wgpu::RenderPipeline,
    accumulation_texture: wgpu::Texture,

    skins: Vec<Skin>,
    meshes: TrackedStorage<Mesh>,
    anim_meshes: TrackedStorage<AnimMesh>,
    meshes_changed: BitVec,
    anim_meshes_changed: BitVec,

    meshes_gpu_data: Vec<GPUMeshData>,
    meshes_bvh_buffer: ManagedBuffer<BVHNode>,
    meshes_mbvh_buffer: ManagedBuffer<MBVHNode>,
    meshes_prim_indices: ManagedBuffer<u32>,
    mesh_prim_index_counter: usize,
    mesh_bvh_index_counter: usize,
    mesh_mbvh_index_counter: usize,

    instances: TrackedStorage<Instance>,
    instances_buffer: ManagedBuffer<GPUInstanceData>,
    triangles_buffer: ManagedBuffer<RTTriangle>,
    triangles_index_counter: usize,

    textures: Vec<Texture>,

    materials_buffer: ManagedBuffer<DeviceMaterial>,
    texture_array: wgpu::Texture,
    texture_array_view: wgpu::TextureView,
    texture_sampler: wgpu::Sampler,

    bvh: BVH,
    mbvh: MBVH,

    point_lights: ManagedBuffer<PointLight>,
    spot_lights: ManagedBuffer<SpotLight>,
    area_lights: ManagedBuffer<AreaLight>,
    directional_lights: ManagedBuffer<DirectionalLight>,

    lights_bind_group_layout: wgpu::BindGroupLayout,
    lights_bind_group: wgpu::BindGroup,
    light_counts: [usize; 4],
    skybox_texture: wgpu::Texture,
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
    const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
    const ACC_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba32Float;
    const SWAPCHAIN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    const TEXTURE_WIDTH: usize = 1024;
    const TEXTURE_HEIGHT: usize = 1024;
    const TEXTURE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;
}

impl Renderer for RayTracer {
    fn init<T: HasRawWindowHandle>(
        window: &T,
        width: usize,
        height: usize,
    ) -> Result<Box<Self>, Box<dyn Error>> {
        let instance = wgpu::Instance::new(wgpu::BackendBit::PRIMARY);
        let surface = unsafe { instance.create_surface(window) };
        let adapter = match block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
        })) {
            Some(adapter) => adapter,
            None => return Err(Box::new(RayTracerError::RequestDeviceError)),
        };

        println!("Picked render device: {}", adapter.get_info().name);

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::PUSH_CONSTANTS
                    | wgpu::Features::SAMPLED_TEXTURE_BINDING_ARRAY
                    | wgpu::Features::MAPPABLE_PRIMARY_BUFFERS,
                limits: wgpu::Limits::default(),
                shader_validation: true,
            },
            None,
        ))
        .unwrap();

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

        let skybox_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("skybox"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth: 1,
            },
            mip_level_count: 5,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::TEXTURE_FORMAT,
            usage: wgpu::TextureUsage::SAMPLED,
        });
        let skybox_texture_view = skybox_texture.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::TEXTURE_FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: NonZeroU32::new(5),
            base_array_layer: 0,
            array_layer_count: None,
        });

        let output_bind_group = bind_group::BindGroupBuilder::default()
            .with_binding(bind_group::BindGroupBinding {
                index: 0,
                binding: bind_group::Binding::SampledTexture(
                    output_texture.create_view(&wgpu::TextureViewDescriptor {
                        label: None,
                        format: Some(Self::OUTPUT_FORMAT),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        level_count: None,
                        base_array_layer: 0,
                        array_layer_count: None,
                    }),
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
                        label: None,
                        address_mode_u: wgpu::AddressMode::ClampToEdge,
                        address_mode_v: wgpu::AddressMode::ClampToEdge,
                        address_mode_w: wgpu::AddressMode::ClampToEdge,
                        mag_filter: wgpu::FilterMode::Linear,
                        min_filter: wgpu::FilterMode::Linear,
                        mipmap_filter: wgpu::FilterMode::Linear,
                        lod_min_clamp: 0.0,
                        lod_max_clamp: 0.0,
                        compare: None,
                        anisotropy_clamp: None,
                    },
                )),
                visibility: wgpu::ShaderStage::FRAGMENT,
            })
            .unwrap()
            .build(&device);

        let mesh_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mesh-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: MeshBindings::PrimIndices as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: MeshBindings::BVHNodes as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: MeshBindings::MBVHNodes as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: MeshBindings::Triangles as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                ],
            });

        let output_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&output_bind_group.layout],
                push_constant_ranges: &[],
            });

        let vert_shader: &[u8] = include_bytes!("../shaders/quad.vert.spv");
        let frag_shader: &[u8] = include_bytes!("../shaders/quad.frag.spv");

        let vert_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(Cow::from(
            vert_shader.as_quad_bytes(),
        )));
        let frag_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(Cow::from(
            frag_shader.as_quad_bytes(),
        )));

        let output_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            layout: Some(&output_pipeline_layout),
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
                clamp_depth: false,
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

        let blue_noise = blue_noise::create_blue_noise_buffer();

        let camera_read_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera-view-mem"),
            size: 128,
            usage: wgpu::BufferUsage::MAP_READ | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera-view-mem"),
            size: (128 + (blue_noise.len() * std::mem::size_of::<u32>())) as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::STORAGE
                | wgpu::BufferUsage::MAP_WRITE
                | wgpu::BufferUsage::COPY_SRC
                | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: true,
        });

        {
            let mapping = camera_buffer.slice(
                128..(128 + (blue_noise.len() * std::mem::size_of::<u32>())) as wgpu::BufferAddress,
            );
            mapping
                .get_mapped_range_mut()
                .copy_from_slice(blue_noise.as_bytes());
        }

        camera_buffer.unmap();

        let intersection_bind_group = bind_group::BindGroupBuilder::default()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::Output as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageTexture(
                    output_texture.create_view(&wgpu::TextureViewDescriptor {
                        label: None,
                        format: Some(Self::OUTPUT_FORMAT),
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        level_count: NonZeroU32::new(1),
                        base_array_layer: 0,
                        array_layer_count: NonZeroU32::new(1),
                    }),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Float,
                    wgpu::TextureViewDimension::D2,
                ),
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::Camera as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageBuffer(
                    camera_buffer,
                    0..((128 + (blue_noise.len() * std::mem::size_of::<u32>()))
                        as wgpu::BufferAddress),
                ),
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::PathStates as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageBuffer(
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("states-mem"),
                        usage: wgpu::BufferUsage::STORAGE,
                        size: (width * height * 2 * std::mem::size_of::<[f32; 4]>() * 4)
                            as wgpu::BufferAddress,
                        mapped_at_creation: false,
                    }),
                    0..(width * height * 2 * std::mem::size_of::<[f32; 4]>() * 4)
                        as wgpu::BufferAddress,
                ),
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::AccumulationBuffer as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageBuffer(
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("accumulation_buffer"),
                        size: (width * height * 4 * std::mem::size_of::<f32>())
                            as wgpu::BufferAddress,
                        usage: wgpu::BufferUsage::STORAGE,
                        mapped_at_creation: false,
                    }),
                    0..((width * height * 4 * std::mem::size_of::<f32>()) as wgpu::BufferAddress),
                ),
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::PotentialContributions as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::WriteStorageBuffer(
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("accumulation_buffer"),
                        size: (width * height * 12 * std::mem::size_of::<f32>())
                            as wgpu::BufferAddress,
                        usage: wgpu::BufferUsage::STORAGE,
                        mapped_at_creation: false,
                    }),
                    0..((width * height * 12 * std::mem::size_of::<f32>()) as wgpu::BufferAddress),
                ),
            })
            .unwrap()
            .with_binding(bind_group::BindGroupBinding {
                index: IntersectionBindings::Skybox as u32,
                visibility: wgpu::ShaderStage::COMPUTE,
                binding: bind_group::Binding::SampledTexture(
                    skybox_texture_view,
                    Self::TEXTURE_FORMAT,
                    wgpu::TextureComponentType::Uint,
                    wgpu::TextureViewDimension::D2,
                ),
            })
            .unwrap()
            .build(&device);

        let top_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("top-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: TopBindings::InstanceDescriptors as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: TopBindings::TopInstanceIndices as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: TopBindings::TopBVHNodes as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: TopBindings::TopMBVHNodes as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                ],
            });

        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: MaterialBindings::Materials as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: MaterialBindings::Textures as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::SampledTexture {
                            component_type: wgpu::TextureComponentType::Uint,
                            dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: MaterialBindings::TextureSampler as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::Sampler { comparison: false },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: MaterialBindings::InstanceDescriptors as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                ],
            });

        let point_lights = ManagedBuffer::new(
            &device,
            32,
            wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::STORAGE,
        );
        let spot_lights = ManagedBuffer::new(
            &device,
            32,
            wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::STORAGE,
        );
        let area_lights = ManagedBuffer::new(
            &device,
            32,
            wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::STORAGE,
        );
        let directional_lights = ManagedBuffer::new(
            &device,
            32,
            wgpu::BufferUsage::COPY_DST | wgpu::BufferUsage::STORAGE,
        );

        let lights_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("lights_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: LightBindings::PointLights as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: LightBindings::SpotLights as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: LightBindings::AreaLights as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: LightBindings::DirectionalLights as u32,
                        visibility: wgpu::ShaderStage::COMPUTE,
                        ty: wgpu::BindingType::StorageBuffer {
                            dynamic: false,
                            min_binding_size: None,
                            readonly: true,
                        },
                        count: None,
                    },
                ],
            });

        let lights_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lights_bind_group"),
            layout: &lights_bind_group_layout,
            entries: &[
                point_lights.as_binding(LightBindings::PointLights as u32),
                spot_lights.as_binding(LightBindings::SpotLights as u32),
                area_lights.as_binding(LightBindings::AreaLights as u32),
                directional_lights.as_binding(LightBindings::DirectionalLights as u32),
            ],
        });

        let intersection_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    &intersection_bind_group.layout,
                    &mesh_bind_group_layout,
                    &top_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let shade_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[
                    &intersection_bind_group.layout,
                    &mesh_bind_group_layout,
                    &material_bind_group_layout,
                    &lights_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let compute_module: &[u8] = include_bytes!("../shaders/ray_gen.comp.spv");
        let compute_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(
            Cow::from(compute_module.as_quad_bytes()),
        ));
        let intersection_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                layout: Some(&intersection_pipeline_layout),
                compute_stage: wgpu::ProgrammableStageDescriptor {
                    entry_point: "main",
                    module: &compute_module,
                },
            });

        let compute_module: &[u8] = include_bytes!("../shaders/ray_extend.comp.spv",);
        let compute_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(
            Cow::from(compute_module.as_quad_bytes()),
        ));
        let extend_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            layout: Some(&intersection_pipeline_layout),
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &compute_module,
            },
        });

        let compute_module: &[u8] = include_bytes!("../shaders/ray_shadow.comp.spv");
        let compute_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(
            Cow::from(compute_module.as_quad_bytes()),
        ));
        let shadow_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            layout: Some(&intersection_pipeline_layout),
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &compute_module,
            },
        });

        let compute_module: &[u8] = include_bytes!("../shaders/shade.comp.spv");
        let compute_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(
            Cow::from(compute_module.as_quad_bytes()),
        ));
        let shade_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            layout: Some(&shade_pipeline_layout),
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &compute_module,
            },
        });

        let compute_module: &[u8] = include_bytes!("../shaders/blit.comp.spv");
        let compute_module = device.create_shader_module(wgpu::ShaderModuleSource::SpirV(
            Cow::from(compute_module.as_quad_bytes()),
        ));

        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&intersection_bind_group.layout],
            push_constant_ranges: &[],
        });
        let blit_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            layout: Some(&blit_pipeline_layout),
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &compute_module,
            },
        });

        let meshes_bvh_buffer = ManagedBuffer::new(&device, 65536, wgpu::BufferUsage::STORAGE);

        let meshes_mbvh_buffer = ManagedBuffer::new(&device, 65536, wgpu::BufferUsage::STORAGE);

        let meshes_prim_indices = ManagedBuffer::new(&device, 65536, wgpu::BufferUsage::STORAGE);

        let triangles_buffer = ManagedBuffer::new(&device, 65536, wgpu::BufferUsage::STORAGE);

        let instances_buffer = ManagedBuffer::new(&device, 2048, wgpu::BufferUsage::STORAGE);

        let mesh_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            entries: &[
                meshes_prim_indices.as_binding(MeshBindings::PrimIndices as u32),
                meshes_bvh_buffer.as_binding(MeshBindings::BVHNodes as u32),
                meshes_mbvh_buffer.as_binding(MeshBindings::MBVHNodes as u32),
                triangles_buffer.as_binding(MeshBindings::Triangles as u32),
            ],
            label: Some("mesh-bind-group"),
            layout: &mesh_bind_group_layout,
        });

        let top_bvh_buffer = ManagedBuffer::new(&device, 1024, wgpu::BufferUsage::STORAGE);
        let top_mbvh_buffer = ManagedBuffer::new(&device, 512, wgpu::BufferUsage::STORAGE);
        let top_indices = ManagedBuffer::new(&device, 1024, wgpu::BufferUsage::STORAGE);
        let materials_buffer = ManagedBuffer::new(&device, 32, wgpu::BufferUsage::STORAGE);

        let texture_array = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture_array"),
            size: wgpu::Extent3d {
                width: Self::TEXTURE_WIDTH as u32,
                height: Self::TEXTURE_HEIGHT as u32,
                depth: 32,
            },
            mip_level_count: Texture::MIP_LEVELS as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::TEXTURE_FORMAT,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        let texture_array_view = texture_array.create_view(&wgpu::TextureViewDescriptor {
            label: None,
            format: Some(Self::TEXTURE_FORMAT),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            level_count: NonZeroU32::new(Texture::MIP_LEVELS as u32),
            base_array_layer: 0,
            array_layer_count: NonZeroU32::new(32),
        });

        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: (Texture::MIP_LEVELS - 1) as f32,
            compare: None,
            anisotropy_clamp: None,
        });

        let top_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("top-bind-group"),
            layout: &top_bind_group_layout,
            entries: &[
                instances_buffer.as_binding(TopBindings::InstanceDescriptors as u32),
                top_bvh_buffer.as_binding(TopBindings::TopBVHNodes as u32),
                top_mbvh_buffer.as_binding(TopBindings::TopMBVHNodes as u32),
                top_indices.as_binding(TopBindings::TopInstanceIndices as u32),
            ],
        });

        let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material-bind-group"),
            layout: &material_bind_group_layout,
            entries: &[
                materials_buffer.as_binding(MaterialBindings::Materials as u32),
                wgpu::BindGroupEntry {
                    binding: MaterialBindings::Textures as u32,
                    resource: wgpu::BindingResource::TextureView(&texture_array_view),
                },
                wgpu::BindGroupEntry {
                    binding: MaterialBindings::TextureSampler as u32,
                    resource: wgpu::BindingResource::Sampler(&texture_sampler),
                },
                instances_buffer.as_binding(MaterialBindings::InstanceDescriptors as u32),
            ],
        });

        Ok(Box::new(Self {
            device,
            queue,
            surface,
            swap_chain,
            intersection_bind_group,
            intersection_pipeline,
            extend_pipeline,
            shadow_pipeline,
            shade_pipeline,
            shade_pipeline_layout,
            blit_pipeline,
            blit_pipeline_layout,
            mesh_bind_group,
            mesh_bind_group_layout,
            top_bind_group,
            top_bind_group_layout,
            material_bind_group,
            material_bind_group_layout,
            top_bvh_buffer,
            top_mbvh_buffer,
            top_indices,
            width,
            height,
            sample_count: 0,
            buffer_capacity: width * height,
            camera_read_buffer,
            output_bind_group,
            output_texture,
            output_pipeline,
            accumulation_texture,
            skins: Vec::new(),
            meshes: TrackedStorage::new(),
            anim_meshes: TrackedStorage::new(),
            meshes_changed: BitVec::new(),
            anim_meshes_changed: BitVec::new(),
            meshes_gpu_data: vec![],
            meshes_bvh_buffer,
            meshes_mbvh_buffer,
            meshes_prim_indices,
            mesh_prim_index_counter: 0,
            mesh_bvh_index_counter: 0,
            mesh_mbvh_index_counter: 0,
            instances: TrackedStorage::new(),
            instances_buffer,
            triangles_buffer,
            triangles_index_counter: 0,
            textures: Vec::new(),
            materials_buffer,
            texture_array,
            texture_array_view,
            texture_sampler,
            bvh: BVH::empty(),
            mbvh: MBVH::empty(),
            point_lights,
            spot_lights,
            area_lights,
            directional_lights,
            lights_bind_group_layout,
            lights_bind_group,
            light_counts: [0; 4],
            skybox_texture,
        }))
    }

    fn set_meshes(&mut self, meshes: ChangedIterator<'_, Mesh>) {
        for (id, mesh) in meshes {
            if id >= self.meshes.len() {
                self.meshes.push(Mesh::empty());
                self.meshes_changed.push(true);
            }

            self.meshes[id] = mesh.clone();
            self.meshes_changed.set(id, true);
        }
    }

    fn set_animated_meshes(&mut self, meshes: ChangedIterator<'_, AnimatedMesh>) {
        for (id, mesh) in meshes {
            if id >= self.anim_meshes.len() {
                self.anim_meshes
                    .push(AnimMesh::Regular(AnimatedMesh::empty()));
                self.anim_meshes_changed.push(true);
            }

            self.anim_meshes[id] = AnimMesh::Regular(mesh.clone());
            self.anim_meshes_changed.set(id, true);
        }
    }

    fn set_instances(&mut self, instances: ChangedIterator<'_, Instance>) {
        for (id, instance) in instances {
            if id >= self.instances.len() {
                self.instances.push(Instance::default());
            }

            self.instances[id] = instance.clone();
        }
    }

    fn set_materials(&mut self, materials: ChangedIterator<'_, rfw_scene::DeviceMaterial>) {
        let materials = materials.as_slice();

        self.materials_buffer.resize(&self.device, materials.len());
        self.materials_buffer.copy_from_slice(materials);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("material-copy"),
            });
        self.materials_buffer.update(&self.device, &mut encoder);
        self.queue.submit(std::iter::once(encoder.finish()));
        self.top_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("top-bind-group"),
            layout: &self.top_bind_group_layout,
            entries: &[
                self.instances_buffer
                    .as_binding(TopBindings::InstanceDescriptors as u32),
                self.top_bvh_buffer
                    .as_binding(TopBindings::TopBVHNodes as u32),
                self.top_mbvh_buffer
                    .as_binding(TopBindings::TopMBVHNodes as u32),
                self.top_indices
                    .as_binding(TopBindings::TopInstanceIndices as u32),
            ],
        });
    }

    fn set_textures(&mut self, textures: ChangedIterator<'_, rfw_scene::Texture>) {
        let textures = textures.as_slice();
        self.textures = textures
            .par_iter()
            .map(|t| {
                if t.width as usize == Self::TEXTURE_WIDTH
                    && t.height as usize == Self::TEXTURE_HEIGHT
                {
                    t.clone()
                } else {
                    let mut texture =
                        t.resized(Self::TEXTURE_WIDTH as u32, Self::TEXTURE_HEIGHT as u32);
                    texture.generate_mipmaps(Texture::MIP_LEVELS);
                    texture
                }
            })
            .collect();

        self.texture_array = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("texture_array"),
            size: wgpu::Extent3d {
                width: Self::TEXTURE_WIDTH as u32,
                height: Self::TEXTURE_HEIGHT as u32,
                depth: textures.len() as u32,
            },
            mip_level_count: Texture::MIP_LEVELS as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::TEXTURE_FORMAT,
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        for (layer, texture) in self.textures.iter().enumerate() {
            for i in 0..Texture::MIP_LEVELS {
                let data_offset = texture.offset_for_level(i) * 4;
                let (width, height) = texture.mip_level_width_height(i);
                assert!(width > 0, "width was 0");
                assert!(height > 0, "height was 0");

                self.queue.write_texture(
                    wgpu::TextureCopyView {
                        texture: &self.texture_array,
                        mip_level: i as _,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: layer as _,
                        },
                    },
                    &texture.data.as_bytes()
                        [data_offset..(data_offset + width * height * std::mem::size_of::<u32>())],
                    wgpu::TextureDataLayout {
                        bytes_per_row: (width * std::mem::size_of::<u32>()) as u32,
                        offset: 0,
                        rows_per_image: height as u32,
                    },
                    wgpu::Extent3d {
                        width: width as u32,
                        height: height as u32,
                        depth: 1,
                    },
                );
            }
        }

        self.texture_array_view = self
            .texture_array
            .create_view(&wgpu::TextureViewDescriptor {
                label: None,
                format: Some(Self::TEXTURE_FORMAT),
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                level_count: NonZeroU32::new(Texture::MIP_LEVELS as u32),
                base_array_layer: 0,
                array_layer_count: NonZeroU32::new(textures.len() as u32),
            });
    }

    fn synchronize(&mut self) {
        if self.meshes.is_empty() {
            return;
        }

        let skins = &self.skins;
        let anim_meshes = &mut self.anim_meshes;
        let meshes = &mut self.meshes;

        self.instances
            .iter_changed()
            .filter(|(_, inst)| inst.skin_id.is_some())
            .for_each(|(_, inst)| {
                let skin = &skins[inst.skin_id.unwrap() as usize];
                match inst.object_id {
                    ObjectRef::None => {}
                    ObjectRef::Static(_) => {}
                    ObjectRef::Animated(mesh) => {
                        let m = match &anim_meshes[mesh as usize] {
                            AnimMesh::Skinned { original, .. } => original.to_static_mesh(skin),
                            AnimMesh::Regular(org) => org.to_static_mesh(skin),
                            _ => panic!("This should not happen."),
                        };

                        anim_meshes[mesh as usize].set_skinned_mesh(m);
                    }
                }
            });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("synchronize-command"),
            });

        let meshes_changed = &self.meshes_changed;
        let constructed: usize = meshes
            .iter_mut()
            .enumerate()
            .par_bridge()
            .map(|(i, (_, mesh))| {
                if mesh.bvh.is_none() || *meshes_changed.get(i).unwrap() {
                    mesh.refit_bvh();
                    1
                } else {
                    0
                }
            })
            .sum();

        let constructed: usize = constructed
            + anim_meshes
                .iter_mut()
                .enumerate()
                .par_bridge()
                .map(|(i, (_, mesh))| match mesh {
                    AnimMesh::None => 0,
                    AnimMesh::Skinned { skinned, .. } => {
                        if skinned.bvh.is_none() || *meshes_changed.get(i).unwrap() {
                            skinned.refit_bvh();
                            1
                        } else {
                            0
                        }
                    }
                    AnimMesh::Regular(mesh) => {
                        if mesh.bvh.is_none() || *meshes_changed.get(i).unwrap() {
                            mesh.refit_bvh();
                            1
                        } else {
                            0
                        }
                    }
                })
                .sum::<usize>();

        self.meshes_changed.set_all(false);

        if constructed != 0 {
            self.triangles_index_counter = 0;
            self.mesh_bvh_index_counter = 0;
            self.mesh_mbvh_index_counter = 0;
            self.mesh_prim_index_counter = 0;

            self.meshes_gpu_data
                .resize(meshes.len() + anim_meshes.len(), GPUMeshData::default());
            for i in 0..meshes.len() {
                let mesh = &meshes[i];
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

            for i in 0..anim_meshes.len() {
                let j = i + meshes.len();
                match &anim_meshes[i] {
                    AnimMesh::None => {}
                    AnimMesh::Skinned { skinned, .. } => {
                        let mesh = skinned;
                        let start_triangle = self.triangles_index_counter;
                        let start_bvh_node = self.mesh_bvh_index_counter;
                        let start_mbvh_node = self.mesh_mbvh_index_counter;
                        let start_prim_index = self.mesh_prim_index_counter;

                        self.meshes_gpu_data[j].bvh_nodes =
                            mesh.bvh.as_ref().unwrap().nodes.len() as u32;
                        self.meshes_gpu_data[j].bvh_offset = start_bvh_node as u32;
                        self.meshes_gpu_data[j].mbvh_offset = start_mbvh_node as u32;
                        self.meshes_gpu_data[j].triangles = mesh.triangles.len() as u32;
                        self.meshes_gpu_data[j].triangle_offset = start_triangle as u32;
                        self.meshes_gpu_data[j].prim_index_offset = start_prim_index as u32;

                        self.triangles_index_counter += mesh.triangles.len();
                        self.mesh_bvh_index_counter += mesh.bvh.as_ref().unwrap().nodes.len();
                        self.mesh_mbvh_index_counter += mesh.mbvh.as_ref().unwrap().m_nodes.len();
                        self.mesh_prim_index_counter +=
                            mesh.bvh.as_ref().unwrap().prim_indices.len();
                    }
                    AnimMesh::Regular(mesh) => {
                        let start_triangle = self.triangles_index_counter;
                        let start_bvh_node = self.mesh_bvh_index_counter;
                        let start_mbvh_node = self.mesh_mbvh_index_counter;
                        let start_prim_index = self.mesh_prim_index_counter;

                        self.meshes_gpu_data[j].bvh_nodes =
                            mesh.bvh.as_ref().unwrap().nodes.len() as u32;
                        self.meshes_gpu_data[j].bvh_offset = start_bvh_node as u32;
                        self.meshes_gpu_data[j].mbvh_offset = start_mbvh_node as u32;
                        self.meshes_gpu_data[j].triangles = mesh.triangles.len() as u32;
                        self.meshes_gpu_data[j].triangle_offset = start_triangle as u32;
                        self.meshes_gpu_data[j].prim_index_offset = start_prim_index as u32;

                        self.triangles_index_counter += mesh.triangles.len();
                        self.mesh_bvh_index_counter += mesh.bvh.as_ref().unwrap().nodes.len();
                        self.mesh_mbvh_index_counter += mesh.mbvh.as_ref().unwrap().m_nodes.len();
                        self.mesh_prim_index_counter +=
                            mesh.bvh.as_ref().unwrap().prim_indices.len();
                    }
                }
            }

            self.meshes_prim_indices
                .resize(&self.device, self.mesh_prim_index_counter);
            self.meshes_bvh_buffer
                .resize(&self.device, self.mesh_bvh_index_counter);
            self.meshes_mbvh_buffer
                .resize(&self.device, self.mesh_bvh_index_counter);
            self.triangles_buffer
                .resize(&self.device, self.triangles_index_counter);

            for i in 0..meshes.len() {
                let mesh = &meshes[i];
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

            for i in 0..anim_meshes.len() {
                match &anim_meshes[i] {
                    AnimMesh::None => {}
                    AnimMesh::Skinned { skinned, .. } => {
                        let mesh = skinned;
                        let offset_data = &self.meshes_gpu_data[i + meshes.len()];

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
                    AnimMesh::Regular(mesh) => {
                        let offset_data = &self.meshes_gpu_data[i + meshes.len()];

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
                }
            }

            self.meshes_prim_indices.update(&self.device, &mut encoder);
            self.meshes_bvh_buffer.update(&self.device, &mut encoder);
            self.meshes_mbvh_buffer.update(&self.device, &mut encoder);
            self.triangles_buffer.update(&self.device, &mut encoder);

            self.mesh_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                entries: &[
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
        let instances = &self.instances;
        let instances_buffer = &mut self.instances_buffer;
        let aabbs: Vec<AABB> = self.instances.iter().map(|(_, i)| i.bounds()).collect();

        let centers: Vec<Vec3A> = aabbs.iter().map(|bb| bb.center()).collect();
        let builder = BinnedSahBuilder::new(aabbs.as_slice(), centers.as_slice());
        self.bvh = builder.build();
        self.mbvh = MBVH::construct(&self.bvh);

        self.top_bvh_buffer
            .resize(&self.device, self.bvh.nodes.len());
        self.top_mbvh_buffer
            .resize(&self.device, self.mbvh.nodes.len());
        self.top_indices
            .resize(&self.device, self.bvh.prim_indices.len());
        instances_buffer.as_mut_slice()[0..instances.len()]
            .iter_mut()
            .enumerate()
            .for_each(|(i, inst)| match instances[i].object_id {
                ObjectRef::None => {}
                ObjectRef::Static(mesh_id) => {
                    let mesh_data = &mesh_data[mesh_id as usize];
                    inst.prim_index_offset = mesh_data.prim_index_offset;
                    inst.triangle_offset = mesh_data.triangle_offset;
                    inst.bvh_offset = mesh_data.bvh_offset;
                    inst.mbvh_offset = mesh_data.mbvh_offset;
                    inst.matrix = instances[i].get_transform();
                    inst.inverse = instances[i].get_inverse_transform();
                    inst.normal = instances[i].get_normal_transform();
                }
                ObjectRef::Animated(mesh_id) => {
                    let mesh_id = mesh_id + meshes.len() as u32;
                    let mesh_data = &mesh_data[mesh_id as usize];
                    inst.prim_index_offset = mesh_data.prim_index_offset;
                    inst.triangle_offset = mesh_data.triangle_offset;
                    inst.bvh_offset = mesh_data.bvh_offset;
                    inst.mbvh_offset = mesh_data.mbvh_offset;
                    inst.matrix = instances[i].get_transform();
                    inst.inverse = instances[i].get_inverse_transform();
                    inst.normal = instances[i].get_normal_transform();
                }
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

        self.point_lights.update(&self.device, &mut encoder);
        self.spot_lights.update(&self.device, &mut encoder);
        self.area_lights.update(&self.device, &mut encoder);
        self.directional_lights.update(&self.device, &mut encoder);

        self.lights_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("lights_bind_group"),
            layout: &self.lights_bind_group_layout,
            entries: &[
                self.point_lights
                    .as_binding(LightBindings::PointLights as u32),
                self.spot_lights
                    .as_binding(LightBindings::SpotLights as u32),
                self.area_lights
                    .as_binding(LightBindings::AreaLights as u32),
                self.directional_lights
                    .as_binding(LightBindings::DirectionalLights as u32),
            ],
        });

        self.queue.submit(std::iter::once(encoder.finish()));
        self.top_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("top-bind-group"),
            layout: &self.top_bind_group_layout,
            entries: &[
                self.instances_buffer
                    .as_binding(TopBindings::InstanceDescriptors as u32),
                self.top_bvh_buffer
                    .as_binding(TopBindings::TopBVHNodes as u32),
                self.top_mbvh_buffer
                    .as_binding(TopBindings::TopMBVHNodes as u32),
                self.top_indices
                    .as_binding(TopBindings::TopInstanceIndices as u32),
            ],
        });

        self.material_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material-bind-group"),
            layout: &self.material_bind_group_layout,
            entries: &[
                self.materials_buffer
                    .as_binding(MaterialBindings::Materials as u32),
                wgpu::BindGroupEntry {
                    binding: MaterialBindings::Textures as u32,
                    resource: wgpu::BindingResource::TextureView(&self.texture_array_view),
                },
                wgpu::BindGroupEntry {
                    binding: MaterialBindings::TextureSampler as u32,
                    resource: wgpu::BindingResource::Sampler(&self.texture_sampler),
                },
                self.instances_buffer
                    .as_binding(MaterialBindings::InstanceDescriptors as u32),
            ],
        });
    }

    fn render(&mut self, camera: &rfw_scene::Camera, mode: RenderMode) {
        if self.meshes.is_empty() {
            return;
        }

        if mode == RenderMode::Reset {
            self.sample_count = 0;
        }

        let view = camera.get_view();
        let mut camera_data = CameraData::new(
            view,
            self.width,
            self.height,
            self.sample_count,
            self.light_counts[LightBindings::PointLights as usize],
            self.light_counts[LightBindings::AreaLights as usize],
            self.light_counts[LightBindings::SpotLights as usize],
            self.light_counts[LightBindings::DirectionalLights as usize],
        );

        let mut path_count = self.width * self.height;
        let mut i = 0;
        while path_count > 0 && i < 3 {
            self.write_camera_data(&camera_data);

            if i == 0 {
                self.perform_pass(self.width, self.height, PassType::Primary);
            } else {
                self.perform_pass(path_count, 0, PassType::Secondary);
            }

            self.read_camera_data(&mut camera_data);

            path_count = camera_data.extension_id as usize;
            if camera_data.shadow_id > 0 {
                self.perform_pass(camera_data.shadow_id as usize, 0, PassType::Shadow);
            }

            camera_data.shadow_id = 0;
            camera_data.path_length += 1;
            camera_data.extension_id = 0;
            camera_data.path_count = path_count as i32;
            i += 1;
        }

        self.sample_count += 1;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render-command-mem"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass();
            let bind_group = self.intersection_bind_group.as_bind_group(&self.device);

            // Blit
            compute_pass.set_pipeline(&self.blit_pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);
            compute_pass.dispatch(
                (self.width as f32 / 16.0).ceil() as u32,
                (self.height as f32 / 4.0).ceil() as u32,
                1,
            );
        }

        if let Ok(output) = self.swap_chain.get_current_frame() {
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    depth_stencil_attachment: None,
                    color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                        attachment: &output.output.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: true,
                        },
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

            self.queue.submit(std::iter::once(encoder.finish()));
        } else {
            println!("Could not get next swap-chain texture.");
        }
    }

    fn resize<T: HasRawWindowHandle>(&mut self, _window: &T, width: usize, height: usize) {
        if (width * height) > self.buffer_capacity {
            self.buffer_capacity = ((width * height) as f64 * 1.5).ceil() as usize;
            self.intersection_bind_group
                .bind(
                    IntersectionBindings::PathStates as u32,
                    bind_group::Binding::WriteStorageBuffer(
                        self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("states-mem"),
                            usage: wgpu::BufferUsage::STORAGE,
                            size: (self.buffer_capacity * 2 * std::mem::size_of::<[f32; 4]>() * 4)
                                as wgpu::BufferAddress,
                            mapped_at_creation: false,
                        }),
                        0..(self.buffer_capacity * 2 * std::mem::size_of::<[f32; 4]>() * 4)
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
                    self.output_texture
                        .create_view(&wgpu::TextureViewDescriptor {
                            label: None,
                            format: Some(Self::OUTPUT_FORMAT),
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            aspect: wgpu::TextureAspect::All,
                            base_mip_level: 0,
                            level_count: None,
                            base_array_layer: 0,
                            array_layer_count: None,
                        }),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Float,
                    wgpu::TextureViewDimension::D2,
                ),
            )
            .unwrap();
        self.intersection_bind_group
            .bind(
                IntersectionBindings::AccumulationBuffer as u32,
                bind_group::Binding::WriteStorageBuffer(
                    self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("accumulation_buffer"),
                        size: (self.width * self.height * 4 * std::mem::size_of::<f32>())
                            as wgpu::BufferAddress,
                        usage: wgpu::BufferUsage::STORAGE,
                        mapped_at_creation: false,
                    }),
                    0..((self.width * self.height * 4 * std::mem::size_of::<f32>())
                        as wgpu::BufferAddress),
                ),
            )
            .unwrap();
        self.intersection_bind_group
            .bind(
                IntersectionBindings::PotentialContributions as u32,
                bind_group::Binding::WriteStorageBuffer(
                    self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("accumulation_buffer"),
                        size: (self.width * self.height * 12 * std::mem::size_of::<f32>())
                            as wgpu::BufferAddress,
                        usage: wgpu::BufferUsage::STORAGE,
                        mapped_at_creation: false,
                    }),
                    0..((self.width * self.height * 12 * std::mem::size_of::<f32>())
                        as wgpu::BufferAddress),
                ),
            )
            .unwrap();

        self.intersection_bind_group
            .bind(
                IntersectionBindings::Output as u32,
                bind_group::Binding::WriteStorageTexture(
                    self.output_texture
                        .create_view(&wgpu::TextureViewDescriptor {
                            label: None,
                            format: Some(Self::OUTPUT_FORMAT),
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            aspect: wgpu::TextureAspect::All,
                            base_mip_level: 0,
                            level_count: None,
                            base_array_layer: 0,
                            array_layer_count: None,
                        }),
                    Self::OUTPUT_FORMAT,
                    wgpu::TextureComponentType::Uint,
                    wgpu::TextureViewDimension::D2,
                ),
            )
            .unwrap();
    }

    fn set_point_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::PointLight>) {
        self.light_counts[LightBindings::PointLights as usize] = lights.len();
        self.point_lights.resize(&self.device, lights.len());
        self.point_lights.as_mut_slice()[0..lights.len()].clone_from_slice(lights.as_slice());
    }

    fn set_spot_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::SpotLight>) {
        self.light_counts[LightBindings::SpotLights as usize] = lights.len();
        self.spot_lights.resize(&self.device, lights.len());
        self.spot_lights.as_mut_slice()[0..lights.len()].clone_from_slice(lights.as_slice());
    }

    fn set_area_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::AreaLight>) {
        self.light_counts[LightBindings::AreaLights as usize] = lights.len();
        self.area_lights.resize(&self.device, lights.len());
        self.area_lights.as_mut_slice()[0..lights.len()].clone_from_slice(lights.as_slice());
    }

    fn set_directional_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::DirectionalLight>) {
        self.light_counts[LightBindings::DirectionalLights as usize] = lights.len();
        self.directional_lights.resize(&self.device, lights.len());
        self.directional_lights.as_mut_slice()[0..lights.len()].clone_from_slice(lights.as_slice());
    }

    fn set_skybox(&mut self, mut skybox: Texture) {
        skybox.generate_mipmaps(5);

        self.skybox_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("skybox"),
            size: wgpu::Extent3d {
                width: skybox.width,
                height: skybox.height,
                depth: 1,
            },
            mip_level_count: skybox.mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::TEXTURE_FORMAT,
            usage: wgpu::TextureUsage::COPY_DST | wgpu::TextureUsage::SAMPLED,
        });

        for i in 0..skybox.mip_levels {
            let data_offset = skybox.offset_for_level(i as _);
            let (width, height) = skybox.mip_level_width_height(i as _);

            let start = data_offset * 4;
            let end = start + (width * height * 4);

            self.queue.write_texture(
                wgpu::TextureCopyView {
                    texture: &self.skybox_texture,
                    mip_level: i,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                },
                &skybox.data.as_bytes()[start..end],
                wgpu::TextureDataLayout {
                    offset: 0,
                    bytes_per_row: (width * std::mem::size_of::<u32>()) as u32,
                    rows_per_image: height as _,
                },
                wgpu::Extent3d {
                    width: width as _,
                    height: height as _,
                    depth: 1,
                },
            );
        }

        self.intersection_bind_group
            .bind(
                IntersectionBindings::Skybox as u32,
                bind_group::Binding::SampledTexture(
                    self.skybox_texture
                        .create_view(&wgpu::TextureViewDescriptor {
                            label: None,
                            format: Some(Self::OUTPUT_FORMAT),
                            dimension: Some(wgpu::TextureViewDimension::D2),
                            aspect: wgpu::TextureAspect::All,
                            base_mip_level: 0,
                            level_count: NonZeroU32::new(skybox.mip_levels),
                            base_array_layer: 0,
                            array_layer_count: None,
                        }),
                    Self::TEXTURE_FORMAT,
                    wgpu::TextureComponentType::Uint,
                    wgpu::TextureViewDimension::D2,
                ),
            )
            .unwrap();
    }

    fn set_skins(&mut self, skins: ChangedIterator<'_, Skin>) {
        for (id, skin) in skins {
            while id >= self.skins.len() {
                self.skins.push(Skin::default());
            }

            self.skins[id] = skin.clone();
        }
    }

    fn get_settings(&self) -> Vec<rfw_scene::renderers::Setting> {
        Vec::new()
    }

    fn set_setting(&mut self, _setting: rfw_scene::renderers::Setting) {
        todo!()
    }
}

impl RayTracer {
    fn create_output_texture(device: &wgpu::Device, width: usize, height: usize) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("output-texture"),
            size: wgpu::Extent3d {
                width: width as u32,
                height: height as u32,
                depth: 1,
            },
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
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::STORAGE,
            format: Self::ACC_FORMAT,
            dimension: wgpu::TextureDimension::D2,
            mip_level_count: 1,
            sample_count: 1,
        })
    }

    fn read_camera_data(&mut self, camera_data: &mut CameraData) {
        {
            let slice = self.camera_read_buffer.slice(..);
            let mapping = slice.map_async(wgpu::MapMode::Read);
            self.device.poll(wgpu::Maintain::Wait);
            block_on(mapping).unwrap();

            let data = unsafe {
                std::slice::from_raw_parts_mut(
                    camera_data as *mut CameraData as *mut u8,
                    std::mem::size_of::<CameraData>(),
                )
            };

            data.copy_from_slice(slice.get_mapped_range().as_ref());
        }
        self.camera_read_buffer.unmap();
    }

    fn write_camera_data(&mut self, camera_data: &CameraData) {
        if let Some(binding) = self
            .intersection_bind_group
            .get_mut(IntersectionBindings::Camera as u32)
        {
            match &mut binding.binding {
                bind_group::Binding::WriteStorageBuffer(buffer, _) => {
                    self.queue
                        .write_buffer(buffer, 0, &camera_data.as_bytes()[0..128]);
                }
                _ => {}
            }
        }
    }

    fn perform_pass(&mut self, width: usize, height: usize, pass_type: PassType) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pass"),
            });
        {
            let bind_group = self.intersection_bind_group.as_bind_group(&self.device);
            let mut compute_pass = encoder.begin_compute_pass();

            match pass_type {
                PassType::Primary => {
                    compute_pass.set_pipeline(&self.intersection_pipeline);
                    compute_pass.set_bind_group(0, bind_group, &[]);
                    compute_pass.set_bind_group(1, &self.mesh_bind_group, &[]);
                    compute_pass.set_bind_group(2, &self.top_bind_group, &[]);
                    compute_pass.dispatch(
                        (width as f32 / 16.0).ceil() as u32,
                        (height as f32 / 16.0).ceil() as u32,
                        1,
                    );

                    // Shade
                    compute_pass.set_pipeline(&self.shade_pipeline);
                    compute_pass.set_bind_group(0, bind_group, &[]);
                    compute_pass.set_bind_group(1, &self.mesh_bind_group, &[]);
                    compute_pass.set_bind_group(2, &self.material_bind_group, &[]);
                    compute_pass.set_bind_group(3, &self.lights_bind_group, &[]);
                    compute_pass.dispatch(((width * height) as f32 / 64.0).ceil() as u32, 1, 1);
                }
                PassType::Secondary => {
                    compute_pass.set_pipeline(&self.extend_pipeline);
                    compute_pass.set_bind_group(0, bind_group, &[]);
                    compute_pass.set_bind_group(1, &self.mesh_bind_group, &[]);
                    compute_pass.set_bind_group(2, &self.top_bind_group, &[]);
                    compute_pass.dispatch((width as f32 / 64.0).ceil() as u32, 1, 1);

                    // // Shade
                    compute_pass.set_pipeline(&self.shade_pipeline);
                    compute_pass.set_bind_group(0, bind_group, &[]);
                    compute_pass.set_bind_group(1, &self.mesh_bind_group, &[]);
                    compute_pass.set_bind_group(2, &self.material_bind_group, &[]);
                    compute_pass.set_bind_group(3, &self.lights_bind_group, &[]);
                    compute_pass.dispatch((width as f32 / 64.0).ceil() as u32, 1, 1);
                }
                PassType::Shadow => {
                    compute_pass.set_pipeline(&self.shadow_pipeline);
                    compute_pass.set_bind_group(0, bind_group, &[]);
                    compute_pass.set_bind_group(1, &self.mesh_bind_group, &[]);
                    compute_pass.set_bind_group(2, &self.top_bind_group, &[]);
                    compute_pass.dispatch((width as f32 / 64.0).ceil() as u32, 1, 1);
                }
            }
        }

        if let Some(binding) = self
            .intersection_bind_group
            .get_mut(IntersectionBindings::Camera as u32)
        {
            match &mut binding.binding {
                bind_group::Binding::WriteStorageBuffer(buffer, _) => {
                    encoder.copy_buffer_to_buffer(buffer, 0, &self.camera_read_buffer, 0, 128);
                }
                _ => {}
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
