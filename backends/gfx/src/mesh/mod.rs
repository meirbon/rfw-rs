use crate::hal::command::BufferCopy;
use crate::hal::format::Aspects;
use crate::hal::image::{Access, Kind, Layout, Level, SubresourceRange, Tiling};
use crate::hal::memory::Segment;
use crate::hal::memory::{Barrier, Dependencies};
use crate::mem::{Allocator, Buffer, Memory};
use crate::{hal, instances::SceneList, CmdBufferPool, DeviceHandle, Queue};

use crate::hal::prelude::DescriptorPool;
use crate::instances::RenderBuffers;
use crate::mem::image::{Texture, TextureDescriptor, TextureView, TextureViewDescriptor};
use crate::skinning::SkinList;
use hal::*;
use hal::{
    command::{self, CommandBuffer},
    device::Device,
    window::Extent2D,
};
use pass::Subpass;
use rfw::prelude::mesh::VertexMesh;
use rfw::prelude::*;
use std::{borrow::Borrow, mem::ManuallyDrop, ptr, rc::Rc, sync::Arc};

pub mod anim;

#[derive(Debug, Clone)]
pub struct GfxMesh<B: hal::Backend> {
    pub id: usize,
    pub buffer: Option<Arc<Buffer<B>>>,
    pub sub_meshes: Vec<VertexMesh>,
    pub vertices: usize,
    pub bounds: AABB,
}

impl<B: hal::Backend> Default for GfxMesh<B> {
    fn default() -> Self {
        Self {
            id: 0,
            buffer: None,
            sub_meshes: Vec::new(),
            vertices: 0,
            bounds: AABB::empty(),
        }
    }
}

#[allow(dead_code)]
impl<B: hal::Backend> GfxMesh<B> {
    pub fn default_id(id: usize) -> Self {
        Self {
            id,
            ..Self::default()
        }
    }

    pub fn valid(&self) -> bool {
        self.buffer.is_some()
    }
}

#[derive(Debug)]
pub struct RenderPipeline<B: hal::Backend> {
    device: DeviceHandle<B>,
    allocator: Allocator<B>,
    desc_pool: ManuallyDrop<B::DescriptorPool>,
    desc_set: B::DescriptorSet,
    set_layout: ManuallyDrop<B::DescriptorSetLayout>,
    pipeline: ManuallyDrop<B::GraphicsPipeline>,
    pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    render_pass: ManuallyDrop<B::RenderPass>,
    uniform_buffer: Buffer<B>,
    depth_image: ManuallyDrop<B::Image>,
    depth_image_view: ManuallyDrop<B::ImageView>,
    depth_memory: Memory<B>,

    textures: FlaggedStorage<Rc<Option<Texture<B>>>>,
    texture_views: FlaggedStorage<Rc<Option<TextureView<B>>>>,
    cmd_pool: CmdBufferPool<B>,
    queue: Queue<B>,

    mat_desc_pool: ManuallyDrop<B::DescriptorPool>,
    mat_set_layout: ManuallyDrop<B::DescriptorSetLayout>,
    mat_sets: Vec<Rc<Option<B::DescriptorSet>>>,
    material_buffer: Buffer<B>,
    tex_sampler: ManuallyDrop<B::Sampler>,

    output_image: ManuallyDrop<B::Image>,
    output_image_view: ManuallyDrop<B::ImageView>,
    output_memory: Memory<B>,

    output_sampler: ManuallyDrop<B::Sampler>,
    output_pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    output_pipeline: ManuallyDrop<B::GraphicsPipeline>,
    output_pass: ManuallyDrop<B::RenderPass>,
    output_set_layout: ManuallyDrop<B::DescriptorSetLayout>,
    output_set: ManuallyDrop<B::DescriptorSet>,
    output_framebuffer: ManuallyDrop<B::Framebuffer>,

    output_format: format::Format,
    viewport: pso::Viewport,
}

impl<B: hal::Backend> RenderPipeline<B> {
    const DEPTH_FORMAT: hal::format::Format = hal::format::Format::D32Sfloat;
    const UNIFORM_CAMERA_SIZE: usize = std::mem::size_of::<Mat4>() * 2
        + std::mem::size_of::<[u32; 4]>()
        + std::mem::size_of::<Vec4>();

    pub fn new(
        device: DeviceHandle<B>,
        allocator: Allocator<B>,
        queue: Queue<B>,
        format: hal::format::Format,
        width: u32,
        height: u32,
        scene_list: &SceneList<B>,
        skins: &SkinList<B>,
    ) -> Self {
        let set_layout = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_set_layout(
                    &[pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Uniform,
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: false,
                            },
                        },
                        count: 1,
                        stage_flags: pso::ShaderStageFlags::VERTEX
                            | pso::ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    }],
                    &[],
                )
            }
            .expect("Can't create descriptor set layout"),
        );

        let mat_set_layout = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_set_layout(
                    &[
                        pso::DescriptorSetLayoutBinding {
                            binding: 0,
                            ty: pso::DescriptorType::Buffer {
                                ty: pso::BufferDescriptorType::Uniform,
                                format: pso::BufferDescriptorFormat::Structured {
                                    dynamic_offset: false,
                                },
                            },
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::FRAGMENT,
                            immutable_samplers: false,
                        },
                        pso::DescriptorSetLayoutBinding {
                            binding: 1,
                            ty: pso::DescriptorType::Sampler,
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::FRAGMENT,
                            immutable_samplers: false,
                        },
                        pso::DescriptorSetLayoutBinding {
                            binding: 2,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::FRAGMENT,
                            immutable_samplers: false,
                        },
                        pso::DescriptorSetLayoutBinding {
                            binding: 3,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::FRAGMENT,
                            immutable_samplers: false,
                        },
                        pso::DescriptorSetLayoutBinding {
                            binding: 4,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::FRAGMENT,
                            immutable_samplers: false,
                        },
                        pso::DescriptorSetLayoutBinding {
                            binding: 5,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::FRAGMENT,
                            immutable_samplers: false,
                        },
                        pso::DescriptorSetLayoutBinding {
                            binding: 6,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: 1,
                            stage_flags: pso::ShaderStageFlags::FRAGMENT,
                            immutable_samplers: false,
                        },
                    ],
                    &[],
                )
            }
            .expect("Can't create descriptor set layout"),
        );

        let mut desc_pool = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_pool(
                    2, // sets
                    &[
                        pso::DescriptorRangeDesc {
                            ty: pso::DescriptorType::Buffer {
                                ty: pso::BufferDescriptorType::Uniform,
                                format: pso::BufferDescriptorFormat::Structured {
                                    dynamic_offset: false,
                                },
                            },
                            count: 1,
                        },
                        pso::DescriptorRangeDesc {
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: 1,
                        },
                        pso::DescriptorRangeDesc {
                            ty: pso::DescriptorType::Sampler,
                            count: 1,
                        },
                    ],
                    pso::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET,
                )
            }
            .expect("Can't create descriptor pool"),
        );

        let mat_desc_pool = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_pool(
                    256, // sets
                    &[
                        pso::DescriptorRangeDesc {
                            ty: pso::DescriptorType::Buffer {
                                ty: pso::BufferDescriptorType::Uniform,
                                format: pso::BufferDescriptorFormat::Structured {
                                    dynamic_offset: false,
                                },
                            },
                            count: 256,
                        },
                        pso::DescriptorRangeDesc {
                            ty: pso::DescriptorType::Sampler,
                            count: 256,
                        },
                        pso::DescriptorRangeDesc {
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: 256 * 5,
                        },
                    ],
                    pso::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET,
                )
            }
            .expect("Can't create descriptor pool"),
        );
        let desc_set = unsafe { desc_pool.allocate_set(&set_layout) }.unwrap();

        let render_pass = {
            let color_attachment = pass::Attachment {
                format: Some(format),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Clear,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: image::Layout::Undefined..image::Layout::ShaderReadOnlyOptimal,
            };

            let depth_attachment = pass::Attachment {
                format: Some(Self::DEPTH_FORMAT),
                samples: 1,
                ops: pass::AttachmentOps::new(
                    pass::AttachmentLoadOp::Clear,
                    pass::AttachmentStoreOp::Store,
                ),
                stencil_ops: pass::AttachmentOps::DONT_CARE,
                layouts: image::Layout::Undefined..image::Layout::DepthStencilAttachmentOptimal,
            };

            let subpass = pass::SubpassDesc {
                colors: &[(0, image::Layout::ColorAttachmentOptimal)],
                depth_stencil: Some(&(1, image::Layout::DepthStencilAttachmentOptimal)),
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            ManuallyDrop::new(
                unsafe {
                    device.create_render_pass(
                        &[color_attachment, depth_attachment],
                        &[subpass],
                        &[],
                    )
                }
                .expect("Can't create render pass"),
            )
        };

        let pipeline_layout = ManuallyDrop::new(
            unsafe {
                device.create_pipeline_layout(
                    vec![
                        &*set_layout,
                        &*scene_list.set_layout,
                        &*mat_set_layout,
                        &*skins.desc_layout,
                    ],
                    &[],
                )
            }
            .expect("Can't create pipeline layout"),
        );

        let pipeline = {
            let vs_module = {
                let spirv: &[u8] = include_bytes!("../../shaders/mesh.vert.spv");
                unsafe { device.create_shader_module(spirv.as_quad_bytes()) }.unwrap()
            };

            let fs_module = {
                let spirv: &[u8] = include_bytes!("../../shaders/mesh.frag.spv");
                unsafe { device.create_shader_module(spirv.as_quad_bytes()) }.unwrap()
            };

            let pipeline = {
                let (vs_entry, fs_entry) = (
                    pso::EntryPoint {
                        entry: "main",
                        module: &vs_module,
                        specialization: pso::Specialization::default(),
                    },
                    pso::EntryPoint {
                        entry: "main",
                        module: &fs_module,
                        specialization: pso::Specialization::default(),
                    },
                );

                let subpass = Subpass {
                    index: 0,
                    main_pass: &*render_pass,
                };

                let pipeline_desc = pso::GraphicsPipelineDesc {
                    primitive_assembler: pso::PrimitiveAssemblerDesc::Vertex {
                        buffers: &[pso::VertexBufferDesc {
                            binding: 0 as pso::BufferIndex,
                            stride: std::mem::size_of::<Vertex3D>() as pso::ElemStride,
                            rate: pso::VertexInputRate::Vertex,
                        }], // Vec<VertexBufferDesc>,
                        // Vertex attributes (IA)
                        attributes: &[
                            pso::AttributeDesc {
                                /// Vertex array location
                                location: 0,
                                /// Binding number of the associated vertex mem.
                                binding: 0,
                                /// Attribute element description.
                                element: pso::Element {
                                    format: format::Format::Rgba32Sfloat,
                                    offset: 0,
                                },
                            },
                            pso::AttributeDesc {
                                /// Vertex array location
                                location: 1,
                                /// Binding number of the associated vertex mem.
                                binding: 0,
                                /// Attribute element description.
                                element: pso::Element {
                                    format: format::Format::Rgb32Sfloat,
                                    offset: 16,
                                },
                            },
                            pso::AttributeDesc {
                                /// Vertex array location
                                location: 2,
                                /// Binding number of the associated vertex mem.
                                binding: 0,
                                /// Attribute element description.
                                element: pso::Element {
                                    format: format::Format::R32Uint,
                                    offset: 28,
                                },
                            },
                            pso::AttributeDesc {
                                /// Vertex array location
                                location: 3,
                                /// Binding number of the associated vertex mem.
                                binding: 0,
                                /// Attribute element description.
                                element: pso::Element {
                                    format: format::Format::Rg32Sfloat,
                                    offset: 32,
                                },
                            },
                            pso::AttributeDesc {
                                /// Vertex array location
                                location: 4,
                                /// Binding number of the associated vertex mem.
                                binding: 0,
                                /// Attribute element description.
                                element: pso::Element {
                                    format: format::Format::Rgba32Sfloat,
                                    offset: 40,
                                },
                            },
                        ],
                        input_assembler: pso::InputAssemblerDesc {
                            primitive: pso::Primitive::TriangleList,
                            with_adjacency: false,
                            restart_index: None,
                        },
                        vertex: vs_entry,
                        tessellation: None,
                        geometry: None,
                    },
                    fragment: Some(fs_entry),
                    // Rasterizer setup
                    rasterizer: pso::Rasterizer {
                        /// How to rasterize this primitive.
                        polygon_mode: pso::PolygonMode::Fill,
                        /// Which face should be culled.
                        cull_face: pso::Face::BACK,
                        /// Which vertex winding is considered to be the front face for culling.
                        front_face: pso::FrontFace::CounterClockwise,
                        /// Whether or not to enable depth clamping; when enabled, instead of
                        /// fragments being omitted when they are outside the bounds of the z-plane,
                        /// they will be clamped to the min or max z value.
                        depth_clamping: false,
                        /// What depth bias, if any, to use for the drawn primitives.
                        depth_bias: None,
                        /// Controls how triangles will be rasterized depending on their overlap with pixels.
                        conservative: false,
                        /// Controls width of rasterized line segments.
                        line_width: pso::State::Dynamic,
                    },
                    // Description of how blend operations should be performed.
                    blender: pso::BlendDesc {
                        /// The logic operation to apply to the blending equation, if any.
                        logic_op: None,
                        /// Which color targets to apply the blending operation to.
                        targets: vec![pso::ColorBlendDesc {
                            mask: pso::ColorMask::ALL,
                            blend: None,
                        }],
                    },
                    // Depth stencil (DSV)
                    depth_stencil: pso::DepthStencilDesc {
                        depth: Some(pso::DepthTest {
                            fun: pso::Comparison::LessEqual,
                            write: true,
                        }),
                        depth_bounds: false,
                        stencil: None,
                    },
                    // Multisampling.
                    multisampling: Some(pso::Multisampling {
                        rasterization_samples: 1 as image::NumSamples,
                        sample_shading: None,
                        sample_mask: !0,
                        /// Toggles alpha-to-coverage multisampling, which can produce nicer edges
                        /// when many partially-transparent polygons are overlapping.
                        /// See [here]( https://msdn.microsoft.com/en-us/library/windows/desktop/bb205072(v=vs.85).aspx#Alpha_To_Coverage) for a full description.
                        alpha_coverage: false,
                        alpha_to_one: false,
                    }),
                    // Static pipeline states.
                    baked_states: pso::BakedStates::default(),
                    // Pipeline layout.
                    layout: &*pipeline_layout,
                    // Subpass in which the pipeline can be executed.
                    subpass,
                    // Options that may be set to alter pipeline properties.
                    flags: pso::PipelineCreationFlags::empty(),
                    /// The parent pipeline, which may be
                    /// `BasePipeline::None`.
                    parent: pso::BasePipeline::None,
                };

                unsafe { device.create_graphics_pipeline(&pipeline_desc, None) }
            };

            unsafe {
                device.destroy_shader_module(vs_module);
            }
            unsafe {
                device.destroy_shader_module(fs_module);
            }

            match pipeline {
                Ok(pipeline) => ManuallyDrop::new(pipeline),
                Err(e) => panic!("Could not compile pipeline {}", e),
            }
        };

        let uniform_buffer = allocator
            .allocate_buffer(
                Self::UNIFORM_CAMERA_SIZE,
                hal::buffer::Usage::UNIFORM,
                hal::memory::Properties::CPU_VISIBLE,
                Some(hal::memory::Properties::CPU_VISIBLE | hal::memory::Properties::DEVICE_LOCAL),
            )
            .unwrap();

        let write = vec![pso::DescriptorSetWrite {
            set: &desc_set,
            binding: 0,
            array_offset: 0,
            descriptors: Some(pso::Descriptor::Buffer(
                uniform_buffer.buffer(),
                hal::buffer::SubRange::WHOLE,
            )),
        }];

        unsafe {
            device.write_descriptor_sets(write);
        }

        let (mut depth_image, req) = unsafe {
            let image = device
                .create_image(
                    Kind::D2(width, height, 1, 1),
                    1,
                    Self::DEPTH_FORMAT,
                    Tiling::Optimal,
                    image::Usage::DEPTH_STENCIL_ATTACHMENT,
                    image::ViewCapabilities::empty(),
                )
                .expect("Could not create depth image.");

            let req = device.get_image_requirements(&image);
            (image, req)
        };
        let depth_memory = allocator
            .allocate_with_reqs(req, memory::Properties::DEVICE_LOCAL, None)
            .unwrap();
        let depth_image_view = unsafe {
            device
                .bind_image_memory(depth_memory.memory(), 0, &mut depth_image)
                .unwrap();

            device
                .create_image_view(
                    &depth_image,
                    image::ViewKind::D2,
                    Self::DEPTH_FORMAT,
                    hal::format::Swizzle::NO,
                    hal::image::SubresourceRange {
                        aspects: hal::format::Aspects::DEPTH,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                )
                .unwrap()
        };

        let output_sampler = unsafe {
            device
                .create_sampler(&image::SamplerDesc {
                    min_filter: hal::image::Filter::Nearest,
                    mag_filter: hal::image::Filter::Nearest,
                    mip_filter: hal::image::Filter::Nearest,
                    wrap_mode: (
                        hal::image::WrapMode::Border,
                        hal::image::WrapMode::Border,
                        hal::image::WrapMode::Border,
                    ),
                    lod_bias: hal::image::Lod(0.0),
                    lod_range: hal::image::Lod(0.0)..hal::image::Lod(1.0),
                    comparison: None,
                    border: hal::image::PackedColor::from([0.0; 4]),
                    normalized: true,
                    anisotropy_clamp: None,
                })
                .expect("Could not create output sampler")
        };

        let output_set_layout = unsafe {
            device
                .create_descriptor_set_layout(
                    &[
                        pso::DescriptorSetLayoutBinding {
                            binding: 0,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: 1,
                            /// Valid shader stages.
                            stage_flags: pso::ShaderStageFlags::FRAGMENT,
                            /// Use the associated list of immutable samplers.
                            immutable_samplers: false,
                        },
                        pso::DescriptorSetLayoutBinding {
                            binding: 1,
                            ty: pso::DescriptorType::Sampler,
                            count: 1,
                            /// Valid shader stages.
                            stage_flags: pso::ShaderStageFlags::FRAGMENT,
                            /// Use the associated list of immutable samplers.
                            immutable_samplers: false,
                        },
                    ],
                    &[],
                )
                .expect("Could not create output set layout")
        };

        let output_set = unsafe {
            desc_pool
                .allocate_set(&output_set_layout)
                .expect("Could not create output descriptor set")
        };

        let (output_image, output_image_view, output_memory) = unsafe {
            let mut image = device
                .create_image(
                    Kind::D2(width, height, 1, 1),
                    1,
                    format,
                    Tiling::Optimal,
                    image::Usage::COLOR_ATTACHMENT | image::Usage::SAMPLED,
                    image::ViewCapabilities::empty(),
                )
                .expect("Could not create depth image.");

            let req = device.get_image_requirements(&image);
            let output_memory = allocator
                .allocate_with_reqs(req, memory::Properties::DEVICE_LOCAL, None)
                .unwrap();

            device
                .bind_image_memory(output_memory.memory(), 0, &mut image)
                .unwrap();

            let image_view = device
                .create_image_view(
                    &image,
                    image::ViewKind::D2,
                    format,
                    hal::format::Swizzle::NO,
                    hal::image::SubresourceRange {
                        aspects: hal::format::Aspects::DEPTH,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                )
                .unwrap();

            (image, image_view, output_memory)
        };

        let output_image = ManuallyDrop::new(output_image);
        let output_image_view = ManuallyDrop::new(output_image_view);

        unsafe {
            device.write_descriptor_sets(vec![
                pso::DescriptorSetWrite {
                    set: &output_set,
                    binding: 0,
                    array_offset: 0,
                    descriptors: std::iter::once(&pso::Descriptor::Image(
                        &*output_image_view,
                        image::Layout::ShaderReadOnlyOptimal,
                    )),
                },
                pso::DescriptorSetWrite {
                    set: &output_set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: std::iter::once(&pso::Descriptor::Sampler(&output_sampler)),
                },
            ]);
        }

        let output_pipeline_layout = unsafe {
            device
                .create_pipeline_layout(std::iter::once(&output_set_layout), &[])
                .expect("Could not create output pipeline layout")
        };

        let output_pass = unsafe {
            device
                .create_render_pass(
                    std::iter::once(pass::Attachment {
                        format: Some(format),
                        samples: 1,
                        ops: pass::AttachmentOps {
                            load: pass::AttachmentLoadOp::Clear,
                            store: pass::AttachmentStoreOp::Store,
                        },
                        stencil_ops: pass::AttachmentOps {
                            load: pass::AttachmentLoadOp::DontCare,
                            store: pass::AttachmentStoreOp::DontCare,
                        },
                        layouts: pass::AttachmentLayout::Undefined..pass::AttachmentLayout::Present,
                    }),
                    &[pass::SubpassDesc {
                        colors: &[(0, image::Layout::ColorAttachmentOptimal)],
                        depth_stencil: None,
                        inputs: &[],
                        resolves: &[],
                        preserves: &[],
                    }],
                    &[],
                )
                .expect("Could not create output pass")
        };

        let output_pipeline = unsafe {
            let vs_module = {
                let spirv: &[u8] = include_bytes!("../../shaders/blit.vert.spv");
                device.create_shader_module(spirv.as_quad_bytes()).unwrap()
            };

            let fs_module = {
                let spirv: &[u8] = include_bytes!("../../shaders/blit.frag.spv");
                device.create_shader_module(spirv.as_quad_bytes()).unwrap()
            };

            let (vs_entry, fs_entry) = (
                pso::EntryPoint {
                    entry: "main",
                    module: &vs_module,
                    specialization: pso::Specialization::default(),
                },
                pso::EntryPoint {
                    entry: "main",
                    module: &fs_module,
                    specialization: pso::Specialization::default(),
                },
            );

            device
                .create_graphics_pipeline(
                    &pso::GraphicsPipelineDesc {
                        primitive_assembler: pso::PrimitiveAssemblerDesc::Vertex {
                            buffers: &[],
                            attributes: &[],
                            input_assembler: pso::InputAssemblerDesc {
                                primitive: pso::Primitive::TriangleList,
                                with_adjacency: false,
                                restart_index: None,
                            },
                            vertex: vs_entry,
                            tessellation: None,
                            geometry: None,
                        },
                        rasterizer: pso::Rasterizer {
                            polygon_mode: pso::PolygonMode::Fill,
                            cull_face: pso::Face::NONE,
                            front_face: pso::FrontFace::CounterClockwise,
                            depth_clamping: false,
                            depth_bias: None,
                            conservative: false,
                            line_width: pso::State::Dynamic,
                        },
                        fragment: Some(fs_entry),
                        blender: pso::BlendDesc {
                            logic_op: None,
                            targets: vec![pso::ColorBlendDesc {
                                mask: pso::ColorMask::ALL,
                                blend: None,
                            }],
                        },
                        depth_stencil: pso::DepthStencilDesc {
                            depth: Some(pso::DepthTest::PASS_TEST),
                            depth_bounds: false,
                            stencil: None,
                        },
                        multisampling: Some(pso::Multisampling {
                            rasterization_samples: 1 as image::NumSamples,
                            sample_shading: None,
                            sample_mask: !0,
                            alpha_coverage: false,
                            alpha_to_one: false,
                        }),
                        baked_states: pso::BakedStates::default(),
                        layout: &output_pipeline_layout,
                        subpass: Subpass {
                            index: 0,
                            main_pass: &output_pass,
                        },
                        flags: pso::PipelineCreationFlags::empty(),
                        parent: pso::BasePipeline::None,
                    },
                    None,
                )
                .expect("Could not create output pipeline")
        };

        let output_framebuffer = unsafe {
            device
                .create_framebuffer(
                    &render_pass,
                    vec![&*output_image_view, &depth_image_view],
                    image::Extent {
                        width,
                        height,
                        depth: 1,
                    },
                )
                .expect("Could not create output frame buffer")
        };

        let cmd_pool = CmdBufferPool::new(
            device.clone(),
            &queue,
            hal::pool::CommandPoolCreateFlags::RESET_INDIVIDUAL,
        )
        .unwrap();

        let material_buffer = allocator
            .allocate_buffer(
                std::mem::size_of::<DeviceMaterial>() * 32,
                hal::buffer::Usage::UNIFORM | hal::buffer::Usage::TRANSFER_DST,
                hal::memory::Properties::DEVICE_LOCAL,
                None,
            )
            .unwrap();

        let tex_sampler = ManuallyDrop::new(unsafe {
            device
                .create_sampler(&hal::image::SamplerDesc {
                    min_filter: hal::image::Filter::Linear,
                    mag_filter: hal::image::Filter::Nearest,
                    mip_filter: hal::image::Filter::Nearest,
                    wrap_mode: (
                        hal::image::WrapMode::Tile,
                        hal::image::WrapMode::Tile,
                        hal::image::WrapMode::Tile,
                    ),
                    lod_bias: hal::image::Lod(0.0),
                    lod_range: hal::image::Lod(0.0)
                        ..hal::image::Lod(rfw::prelude::Texture::MIP_LEVELS as f32),
                    comparison: None,
                    border: hal::image::PackedColor::from([0.0; 4]),
                    normalized: true,
                    anisotropy_clamp: Some(8),
                })
                .unwrap()
        });

        Self {
            device,
            allocator,
            desc_pool,
            desc_set,
            set_layout,
            pipeline,
            pipeline_layout,
            render_pass,
            uniform_buffer,
            depth_image: ManuallyDrop::new(depth_image),
            depth_image_view: ManuallyDrop::new(depth_image_view),
            depth_memory,

            queue,
            cmd_pool,
            textures: FlaggedStorage::new(),
            texture_views: FlaggedStorage::new(),

            mat_desc_pool,
            mat_set_layout,
            mat_sets: Vec::new(),
            material_buffer,
            tex_sampler,

            output_image,
            output_image_view,
            output_memory,

            output_sampler: ManuallyDrop::new(output_sampler),
            output_pipeline_layout: ManuallyDrop::new(output_pipeline_layout),
            output_pipeline: ManuallyDrop::new(output_pipeline),
            output_pass: ManuallyDrop::new(output_pass),
            output_set_layout: ManuallyDrop::new(output_set_layout),
            output_set: ManuallyDrop::new(output_set),
            output_framebuffer: ManuallyDrop::new(output_framebuffer),
            output_format: format,

            viewport: pso::Viewport {
                rect: pso::Rect {
                    x: 0,
                    y: 0,
                    w: width as _,
                    h: height as _,
                },
                depth: 0.0..1.0,
            },
        }
    }

    pub unsafe fn create_frame_buffer<T: Borrow<B::ImageView>>(
        &self,
        surface_image: &T,
        dimensions: Extent2D,
    ) -> B::Framebuffer {
        self.device
            .create_framebuffer(
                &self.output_pass,
                vec![surface_image.borrow()],
                hal::image::Extent {
                    width: dimensions.width,
                    height: dimensions.height,
                    depth: 1,
                },
            )
            .unwrap()
    }

    pub fn update_camera(&mut self, camera: &rfw::prelude::Camera) {
        let mapping = match self.uniform_buffer.map(hal::memory::Segment::ALL) {
            Ok(mapping) => mapping,
            Err(_) => return,
        };

        let view = camera.get_rh_view_matrix();
        let projection = camera.get_rh_projection();

        let light_counts = [0 as u32; 4];

        unsafe {
            let ptr = mapping.as_ptr();
            // View matrix
            ptr.copy_from(view.as_ref().as_ptr() as *const u8, 64);
            // Projection matrix
            ptr.add(64)
                .copy_from(projection.as_ref().as_ptr() as *const u8, 64);

            // Light counts
            ptr.add(128)
                .copy_from(light_counts.as_ptr() as *const u8, 16);

            // Camera position
            ptr.add(144).copy_from(
                Vec3A::from(camera.pos).extend(1.0).as_ref().as_ptr() as *const u8,
                16,
            );
        }
    }

    pub unsafe fn draw(
        &self,
        cmd_buffer: &mut B::CommandBuffer,
        target_framebuffer: &B::Framebuffer,
        target_viewport: &pso::Viewport,
        scene: &SceneList<B>,
        _skins: &SkinList<B>,
        frustrum: &FrustrumG,
    ) {
        cmd_buffer.set_viewports(0, &[self.viewport.clone()]);
        cmd_buffer.set_scissors(0, &[self.viewport.rect]);
        cmd_buffer.begin_render_pass(
            &self.render_pass,
            &self.output_framebuffer,
            self.viewport.rect,
            &[
                command::ClearValue {
                    color: command::ClearColor {
                        float32: [0.0, 0.0, 0.0, 1.0],
                    },
                },
                command::ClearValue {
                    depth_stencil: command::ClearDepthStencil {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ],
            command::SubpassContents::Inline,
        );

        scene.iter_instances(|buffer, instance| {
            if !frustrum.aabb_in_frustrum(&instance.bounds).should_render() {
                return;
            }

            let iter = instance
                .meshes
                .iter()
                .filter(|m| frustrum.aabb_in_frustrum(&m.bounds).should_render());

            let mut first = true;

            iter.for_each(|mesh| {
                if first {
                    cmd_buffer.bind_graphics_descriptor_sets(
                        &self.pipeline_layout,
                        0,
                        std::iter::once(&self.desc_set),
                        &[],
                    );

                    cmd_buffer.bind_graphics_descriptor_sets(
                        &self.pipeline_layout,
                        1,
                        std::iter::once(&scene.desc_set),
                        &[],
                    );

                    match buffer {
                        RenderBuffers::Static(buffer) => {
                            cmd_buffer.bind_graphics_pipeline(&self.pipeline);
                            cmd_buffer.bind_vertex_buffers(
                                0,
                                std::iter::once((buffer.buffer(), buffer::SubRange::WHOLE)),
                            );
                        }
                    }
                    first = false;
                }

                cmd_buffer.bind_graphics_descriptor_sets(
                    &self.pipeline_layout,
                    2,
                    std::iter::once(
                        self.mat_sets[mesh.mat_id as usize]
                            .as_ref()
                            .as_ref()
                            .unwrap(),
                    ),
                    &[],
                );
                cmd_buffer.draw(mesh.first..mesh.last, instance.id..(instance.id + 1));
            });
        });
        cmd_buffer.end_render_pass();

        cmd_buffer.pipeline_barrier(
            pso::PipelineStage::FRAGMENT_SHADER..pso::PipelineStage::BOTTOM_OF_PIPE,
            Dependencies::VIEW_LOCAL,
            std::iter::once(memory::Barrier::Image {
                states: (
                    image::Access::COLOR_ATTACHMENT_WRITE,
                    image::Layout::ColorAttachmentOptimal,
                )
                    ..(
                        image::Access::COLOR_ATTACHMENT_READ,
                        image::Layout::ShaderReadOnlyOptimal,
                    ),
                target: &*self.output_image,
                range: image::SubresourceRange {
                    aspects: format::Aspects::COLOR,
                    level_start: 0,
                    level_count: Some(1),
                    layer_start: 0,
                    layer_count: Some(1),
                },
                families: None,
            }),
        );

        cmd_buffer.set_viewports(0, &[target_viewport.clone()]);
        cmd_buffer.set_scissors(0, &[target_viewport.rect]);
        cmd_buffer.begin_render_pass(
            &self.output_pass,
            target_framebuffer,
            target_viewport.rect,
            &[command::ClearValue {
                color: command::ClearColor {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            }],
            command::SubpassContents::Inline,
        );
        cmd_buffer.bind_graphics_pipeline(&*self.output_pipeline);

        cmd_buffer.bind_graphics_descriptor_sets(
            &*self.output_pipeline_layout,
            0,
            std::iter::once(&*self.output_set),
            &[],
        );
        cmd_buffer.draw(0..6, 0..1);
        cmd_buffer.end_render_pass();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unsafe {
            self.device
                .destroy_framebuffer(ManuallyDrop::into_inner(ptr::read(
                    &self.output_framebuffer,
                )));

            self.device
                .destroy_image_view(ManuallyDrop::into_inner(ptr::read(&self.depth_image_view)));
            self.device
                .destroy_image(ManuallyDrop::into_inner(ptr::read(&self.depth_image)));

            self.device
                .destroy_image_view(ManuallyDrop::into_inner(ptr::read(&self.output_image_view)));
            self.device
                .destroy_image(ManuallyDrop::into_inner(ptr::read(&self.output_image)));
        }

        let (depth_image, depth_image_view) = unsafe {
            let mut image = self
                .device
                .create_image(
                    Kind::D2(width, height, 1, 1),
                    1,
                    Self::DEPTH_FORMAT,
                    Tiling::Optimal,
                    image::Usage::DEPTH_STENCIL_ATTACHMENT,
                    image::ViewCapabilities::empty(),
                )
                .expect("Could not create depth image.");

            let req = self.device.get_image_requirements(&image);
            if req.size > self.depth_memory.len() as _ {
                self.depth_memory = self
                    .allocator
                    .allocate_with_reqs(req, memory::Properties::DEVICE_LOCAL, None)
                    .unwrap();
            }

            self.device
                .bind_image_memory(self.depth_memory.memory(), 0, &mut image)
                .unwrap();

            let image_view = self
                .device
                .create_image_view(
                    &image,
                    image::ViewKind::D2,
                    Self::DEPTH_FORMAT,
                    hal::format::Swizzle::NO,
                    hal::image::SubresourceRange {
                        aspects: hal::format::Aspects::DEPTH,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                )
                .unwrap();

            (image, image_view)
        };

        self.depth_image = ManuallyDrop::new(depth_image);
        self.depth_image_view = ManuallyDrop::new(depth_image_view);

        let (output_image, output_image_view) = unsafe {
            let mut image = self
                .device
                .create_image(
                    Kind::D2(width, height, 1, 1),
                    1,
                    self.output_format,
                    Tiling::Optimal,
                    image::Usage::COLOR_ATTACHMENT | image::Usage::SAMPLED,
                    image::ViewCapabilities::empty(),
                )
                .expect("Could not create depth image.");

            let req = self.device.get_image_requirements(&image);
            if req.size > self.output_memory.len() as _ {
                self.output_memory = self
                    .allocator
                    .allocate_with_reqs(req, memory::Properties::DEVICE_LOCAL, None)
                    .unwrap();
            }

            self.device
                .bind_image_memory(self.output_memory.memory(), 0, &mut image)
                .unwrap();

            let image_view = self
                .device
                .create_image_view(
                    &image,
                    image::ViewKind::D2,
                    self.output_format,
                    hal::format::Swizzle::NO,
                    hal::image::SubresourceRange {
                        aspects: hal::format::Aspects::DEPTH,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                )
                .unwrap();

            (image, image_view)
        };

        self.output_image = ManuallyDrop::new(output_image);
        self.output_image_view = ManuallyDrop::new(output_image_view);

        self.output_framebuffer = ManuallyDrop::new(unsafe {
            self.device
                .create_framebuffer(
                    &*self.render_pass,
                    vec![&*self.output_image_view, &self.depth_image_view],
                    image::Extent {
                        width,
                        height,
                        depth: 1,
                    },
                )
                .expect("Could not create output frame buffer")
        });

        unsafe {
            self.device.write_descriptor_sets(vec![
                pso::DescriptorSetWrite {
                    set: &*self.output_set,
                    binding: 0,
                    array_offset: 0,
                    descriptors: std::iter::once(&pso::Descriptor::Image(
                        &*self.output_image_view,
                        image::Layout::ShaderReadOnlyOptimal,
                    )),
                },
                pso::DescriptorSetWrite {
                    set: &*self.output_set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: std::iter::once(&pso::Descriptor::Sampler(&*self.output_sampler)),
                },
            ]);
        }

        self.viewport.rect.w = width as _;
        self.viewport.rect.h = height as _;
    }

    pub fn set_textures(&mut self, textures: ChangedIterator<'_, rfw::prelude::Texture>) {
        let mut texels = 0;

        for (i, t) in textures.clone() {
            texels += t.data.len();
            let tex = Texture::new(
                self.device.clone(),
                &self.allocator,
                TextureDescriptor {
                    kind: image::Kind::D2(t.width, t.height, 1, 1),
                    mip_levels: t.mip_levels as _,
                    format: format::Format::Bgra8Unorm,
                    tiling: image::Tiling::Optimal,
                    usage: image::Usage::SAMPLED,
                    capabilities: image::ViewCapabilities::empty(),
                },
            )
            .unwrap();
            let view = tex
                .create_view(TextureViewDescriptor {
                    view_kind: image::ViewKind::D2,
                    swizzle: Default::default(),
                    range: image::SubresourceRange {
                        aspects: format::Aspects::COLOR,
                        level_start: 0,
                        level_count: Some(t.mip_levels as _),
                        layer_start: 0,
                        layer_count: Some(1),
                    },
                })
                .unwrap();

            self.textures.overwrite_val(i, Rc::new(Some(tex)));
            self.texture_views.overwrite_val(i, Rc::new(Some(view)));
        }

        let mut staging_buffer = self
            .allocator
            .allocate_buffer(
                texels * std::mem::size_of::<u32>(),
                hal::buffer::Usage::TRANSFER_SRC,
                hal::memory::Properties::CPU_VISIBLE,
                None,
            )
            .unwrap();

        let mut cmd_buffer = unsafe {
            let mut cmd_buffer = self.cmd_pool.allocate_one(hal::command::Level::Primary);

            cmd_buffer.begin_primary(hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);
            cmd_buffer
        };

        if let Ok(mapping) = staging_buffer.map(Segment::ALL) {
            let mut byte_offset = 0;
            for (_, t) in textures.clone() {
                let bytes = t.data.as_bytes();
                mapping.as_slice()[byte_offset..(byte_offset + bytes.len())].copy_from_slice(bytes);
                byte_offset += bytes.len();
            }
        }

        let mut byte_offset = 0;
        for (i, t) in textures.clone() {
            let target = self.textures[i].as_ref().as_ref().unwrap();
            unsafe {
                cmd_buffer.pipeline_barrier(
                    pso::PipelineStage::TOP_OF_PIPE..pso::PipelineStage::TRANSFER,
                    Dependencies::empty(),
                    std::iter::once(&Barrier::Image {
                        range: SubresourceRange {
                            aspects: Aspects::COLOR,
                            level_start: 0,
                            level_count: Some(t.mip_levels as Level),
                            layer_start: 0,
                            layer_count: Some(1),
                        },
                        families: None,
                        states: (Access::empty(), Layout::Undefined)
                            ..(Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
                        target: target.image(),
                    }),
                );
            }

            for m in 0..t.mip_levels {
                let (width, height) = t.mip_level_width_height(m as usize);
                unsafe {
                    cmd_buffer.copy_buffer_to_image(
                        staging_buffer.buffer(),
                        self.textures[i].as_ref().as_ref().unwrap().image(),
                        hal::image::Layout::TransferDstOptimal,
                        std::iter::once(&hal::command::BufferImageCopy {
                            buffer_offset: byte_offset as hal::buffer::Offset,
                            /// Width of a mem 'row' in texels.
                            buffer_width: width as u32,
                            /// Height of a mem 'image slice' in texels.
                            buffer_height: height as u32,
                            /// The image subresource.
                            image_layers: hal::image::SubresourceLayers {
                                layers: 0..1,
                                aspects: Aspects::COLOR,
                                level: m as hal::image::Level,
                            },
                            /// The offset of the portion of the image to copy.
                            image_offset: hal::image::Offset { x: 0, y: 0, z: 0 },
                            /// Size of the portion of the image to copy.
                            image_extent: hal::image::Extent {
                                width: width as u32,
                                height: height as u32,
                                depth: 1,
                            },
                        }),
                    );
                }

                byte_offset += width * height * std::mem::size_of::<u32>();
            }

            unsafe {
                cmd_buffer.pipeline_barrier(
                    pso::PipelineStage::TRANSFER..pso::PipelineStage::FRAGMENT_SHADER,
                    Dependencies::empty(),
                    std::iter::once(&Barrier::Image {
                        range: SubresourceRange {
                            aspects: Aspects::COLOR,
                            level_start: 0,
                            level_count: Some(t.mip_levels as Level),
                            layer_start: 0,
                            layer_count: Some(1),
                        },
                        families: None,
                        states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                            ..(Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
                        target: self.textures[i].as_ref().as_ref().unwrap().image(),
                    }),
                );
            }
        }

        unsafe {
            cmd_buffer.finish();
        }

        self.queue
            .submit_without_semaphores(std::iter::once(&cmd_buffer), None);
        self.queue.wait_idle().unwrap();
    }

    pub fn set_materials(&mut self, materials: &[DeviceMaterial]) {
        let aligned_size = {
            let minimum_alignment =
                self.allocator.limits.min_uniform_buffer_offset_alignment as usize;
            let mut size = minimum_alignment;
            while size < std::mem::size_of::<DeviceMaterial>() {
                size += minimum_alignment;
            }
            size
        };

        if self.material_buffer.len() < materials.len() * aligned_size {
            self.material_buffer = self
                .allocator
                .allocate_buffer(
                    // Minimum alignment of dynamic uniform buffers is 256 bytes
                    materials.len() * 2 * aligned_size,
                    hal::buffer::Usage::UNIFORM | hal::buffer::Usage::TRANSFER_DST,
                    hal::memory::Properties::DEVICE_LOCAL,
                    None,
                )
                .unwrap();
        }

        let mut staging_buffer = self
            .allocator
            .allocate_buffer(
                materials.len() * aligned_size,
                hal::buffer::Usage::TRANSFER_SRC,
                hal::memory::Properties::CPU_VISIBLE,
                None,
            )
            .unwrap();

        if let Ok(mapping) = staging_buffer.map(Segment::ALL) {
            let dst = mapping.as_slice();
            let src = materials.as_bytes();
            for (i, _) in materials.iter().enumerate() {
                let start = i * aligned_size;
                let end = start + std::mem::size_of::<DeviceMaterial>();

                let src_start = i * std::mem::size_of::<DeviceMaterial>();
                let src_end = (i + 1) * std::mem::size_of::<DeviceMaterial>();
                dst[start..end].copy_from_slice(&src[src_start..src_end]);
            }
        }

        let cmd_buffer = unsafe {
            let mut cmd_buffer = self.cmd_pool.allocate_one(hal::command::Level::Primary);

            cmd_buffer.begin_primary(hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);
            cmd_buffer.copy_buffer(
                staging_buffer.buffer(),
                self.material_buffer.buffer(),
                std::iter::once(&BufferCopy {
                    size: (materials.len() * aligned_size) as _,
                    src: 0,
                    dst: 0,
                }),
            );

            cmd_buffer.finish();
            cmd_buffer
        };

        self.queue
            .submit_without_semaphores(std::iter::once(&cmd_buffer), None);

        unsafe {
            let new_length = materials.len().max(self.mat_sets.len());
            self.mat_sets.resize(new_length * 2, Rc::new(None));

            let mut writes = Vec::with_capacity(self.mat_sets.len() * 7);
            let sampler = ManuallyDrop::into_inner(ptr::read(&self.tex_sampler));

            for i in 0..new_length {
                match self.mat_sets[i].as_ref() {
                    Some(_) => continue,
                    None => {
                        self.mat_sets[i] = Rc::new(Some(
                            self.mat_desc_pool
                                .allocate_set(&self.mat_set_layout)
                                .expect("Could not allocate material descriptor set"),
                        ));
                    }
                }
            }

            for (i, _) in materials.iter().enumerate() {
                let mat = &materials[i];
                let set: &B::DescriptorSet = self.mat_sets[i].as_ref().as_ref().unwrap();
                writes.push(pso::DescriptorSetWrite {
                    set,
                    binding: 0,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Buffer(
                        self.material_buffer.buffer(),
                        hal::buffer::SubRange {
                            offset: (i * aligned_size) as _,
                            size: Some(std::mem::size_of::<DeviceMaterial>() as _),
                        },
                    )),
                });

                writes.push(pso::DescriptorSetWrite {
                    set,
                    binding: 1,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Sampler(&sampler)),
                });

                // Texture 0
                let view = self.texture_views[mat.diffuse_map.max(0) as usize]
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .view();
                writes.push(pso::DescriptorSetWrite {
                    set,
                    binding: 2,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Image(
                        view,
                        image::Layout::ShaderReadOnlyOptimal,
                    )),
                });
                // Texture 1
                let view = self.texture_views[mat.normal_map.max(0) as usize]
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .view();
                writes.push(pso::DescriptorSetWrite {
                    set,
                    binding: 3,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Image(
                        view,
                        image::Layout::ShaderReadOnlyOptimal,
                    )),
                });
                // Texture 2
                let view = self.texture_views[mat.metallic_roughness_map.max(0) as usize]
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .view();
                writes.push(pso::DescriptorSetWrite {
                    set,
                    binding: 4,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Image(
                        view,
                        image::Layout::ShaderReadOnlyOptimal,
                    )),
                });
                // Texture 3
                let view = self.texture_views[mat.emissive_map.max(0) as usize]
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .view();
                writes.push(pso::DescriptorSetWrite {
                    set,
                    binding: 5,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Image(
                        view,
                        image::Layout::ShaderReadOnlyOptimal,
                    )),
                });
                // Texture 4
                let view = self.texture_views[mat.sheen_map.max(0) as usize]
                    .as_ref()
                    .as_ref()
                    .unwrap()
                    .view();
                writes.push(pso::DescriptorSetWrite {
                    set,
                    binding: 6,
                    array_offset: 0,
                    descriptors: Some(pso::Descriptor::Image(
                        view,
                        image::Layout::ShaderReadOnlyOptimal,
                    )),
                });
            }

            self.device.write_descriptor_sets(writes);
        }

        self.queue.wait_idle().unwrap();
    }
}

impl<B: hal::Backend> Drop for RenderPipeline<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();

        unsafe {
            self.device
                .destroy_image_view(ManuallyDrop::into_inner(ptr::read(&self.depth_image_view)));

            self.device
                .destroy_image(ManuallyDrop::into_inner(ptr::read(&self.depth_image)));

            self.textures.clear();

            self.device
                .destroy_descriptor_pool(ManuallyDrop::into_inner(ptr::read(&self.desc_pool)));
            self.device
                .destroy_descriptor_pool(ManuallyDrop::into_inner(ptr::read(&self.mat_desc_pool)));

            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.set_layout,
                )));
            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.mat_set_layout,
                )));

            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.output_set_layout,
                )));

            self.device
                .destroy_framebuffer(ManuallyDrop::into_inner(ptr::read(
                    &self.output_framebuffer,
                )));

            self.device
                .destroy_sampler(ManuallyDrop::into_inner(ptr::read(&self.tex_sampler)));

            self.device
                .destroy_sampler(ManuallyDrop::into_inner(ptr::read(&self.output_sampler)));

            self.device
                .destroy_render_pass(ManuallyDrop::into_inner(ptr::read(&self.render_pass)));
            self.device
                .destroy_graphics_pipeline(ManuallyDrop::into_inner(ptr::read(&self.pipeline)));
            self.device
                .destroy_pipeline_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.pipeline_layout,
                )));

            self.device
                .destroy_image(ManuallyDrop::into_inner(ptr::read(&self.output_image)));
            self.device
                .destroy_image_view(ManuallyDrop::into_inner(ptr::read(&self.output_image_view)));
            self.device
                .destroy_graphics_pipeline(ManuallyDrop::into_inner(ptr::read(
                    &self.output_pipeline,
                )));
            self.device
                .destroy_pipeline_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.output_pipeline_layout,
                )));
        }
    }
}
