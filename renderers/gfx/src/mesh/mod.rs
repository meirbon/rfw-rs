use crate::buffer::{Allocator, Buffer, Memory};
use crate::hal::command::BufferCopy;
use crate::hal::format::{Aspects, Format};
use crate::hal::image::{Access, Kind, Layout, Level, SubresourceRange, Tiling};
use crate::hal::memory::Segment;
use crate::hal::memory::{Barrier, Dependencies};
use crate::hal::pool::CommandPool;
use crate::materials::SceneTexture;
use crate::{hal, instances::SceneList, Queue};

use glam::*;
use hal::{
    buffer,
    command::{self, CommandBuffer},
    device::Device,
    image, memory, pass, pso,
    window::Extent2D,
};
use pass::Subpass;
use pso::*;
use rfw_scene::bvh::AABB;
use rfw_scene::{DeviceMaterial, FrustrumG, VertexData, VertexMesh};
use shared::BytesConversion;
use std::sync::Mutex;
use std::{borrow::Borrow, mem::ManuallyDrop, ptr, sync::Arc};

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

pub struct RenderPipeline<B: hal::Backend> {
    device: Arc<B::Device>,
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

    textures: Vec<SceneTexture<B>>,
    cmd_pool: ManuallyDrop<B::CommandPool>,
    queue: Arc<Mutex<Queue<B>>>,

    mat_desc_pool: ManuallyDrop<B::DescriptorPool>,
    mat_set_layout: ManuallyDrop<B::DescriptorSetLayout>,
    mat_sets: Vec<B::DescriptorSet>,
    material_buffer: Buffer<B>,
    tex_sampler: ManuallyDrop<B::Sampler>,
}

impl<B: hal::Backend> RenderPipeline<B> {
    const DEPTH_FORMAT: hal::format::Format = hal::format::Format::D32Sfloat;
    const UNIFORM_CAMERA_SIZE: usize = std::mem::size_of::<Mat4>() * 2
        + std::mem::size_of::<[u32; 4]>()
        + std::mem::size_of::<Vec4>();

    pub fn new(
        device: Arc<B::Device>,
        allocator: Allocator<B>,
        queue: Arc<Mutex<Queue<B>>>,
        format: hal::format::Format,
        width: u32,
        height: u32,
        scene_list: &SceneList<B>,
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
                        stage_flags: pso::ShaderStageFlags::VERTEX,
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
                    1, // sets
                    &[pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Uniform,
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: false,
                            },
                        },
                        count: 1,
                    }],
                    pso::DescriptorPoolCreateFlags::empty(),
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
                layouts: image::Layout::Undefined..image::Layout::Present,
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
                    vec![&*set_layout, &*scene_list.set_layout, &*mat_set_layout],
                    &[],
                )
            }
            .expect("Can't create pipeline layout"),
        );

        let pipeline = {
            let ms_module = {
                let spirv = include_bytes!("../../shaders/mesh.vert.spv");
                unsafe { device.create_shader_module(spirv.as_quad_bytes()) }.unwrap()
            };

            let fs_module = {
                let spirv = include_bytes!("../../shaders/mesh.frag.spv");
                unsafe { device.create_shader_module(spirv.as_quad_bytes()) }.unwrap()
            };

            let pipeline = {
                let (ms_entry, fs_entry) = (
                    pso::EntryPoint {
                        entry: "main",
                        module: &ms_module,
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
                    /// A set of graphics shaders to use for the pipeline.
                    shaders: GraphicsShaderSet {
                        vertex: ms_entry,
                        fragment: Some(fs_entry),
                        hull: None,
                        domain: None,
                        geometry: None,
                    },
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
                        line_width: State::Dynamic,
                    },
                    // Vertex buffers (IA)
                    vertex_buffers: vec![VertexBufferDesc {
                        binding: 0 as BufferIndex,
                        stride: std::mem::size_of::<VertexData>() as ElemStride,
                        rate: VertexInputRate::Vertex,
                    }], // Vec<VertexBufferDesc>,
                    // Vertex attributes (IA)
                    attributes: vec![
                        AttributeDesc {
                            /// Vertex array location
                            location: 0 as Location,
                            /// Binding number of the associated vertex buffer.
                            binding: 0 as BufferIndex,
                            /// Attribute element description.
                            element: Element {
                                format: hal::format::Format::Rgba32Sfloat,
                                offset: 0,
                            },
                        },
                        AttributeDesc {
                            /// Vertex array location
                            location: 1 as Location,
                            /// Binding number of the associated vertex buffer.
                            binding: 0 as BufferIndex,
                            /// Attribute element description.
                            element: Element {
                                format: hal::format::Format::Rgb32Sfloat,
                                offset: 16,
                            },
                        },
                        AttributeDesc {
                            /// Vertex array location
                            location: 2 as Location,
                            /// Binding number of the associated vertex buffer.
                            binding: 0 as BufferIndex,
                            /// Attribute element description.
                            element: Element {
                                format: hal::format::Format::R32Uint,
                                offset: 28,
                            },
                        },
                        AttributeDesc {
                            /// Vertex array location
                            location: 3 as Location,
                            /// Binding number of the associated vertex buffer.
                            binding: 0 as BufferIndex,
                            /// Attribute element description.
                            element: Element {
                                format: hal::format::Format::Rg32Sfloat,
                                offset: 32,
                            },
                        },
                        AttributeDesc {
                            /// Vertex array location
                            location: 4 as Location,
                            /// Binding number of the associated vertex buffer.
                            binding: 0 as BufferIndex,
                            /// Attribute element description.
                            element: Element {
                                format: hal::format::Format::Rgba32Sfloat,
                                offset: 40,
                            },
                        },
                    ],
                    // Input assembler attributes, describes how
                    // vertices are assembled into primitives (such as triangles).
                    input_assembler: InputAssemblerDesc {
                        /// Type of the primitive
                        primitive: Primitive::TriangleList,
                        /// When adjacency information is enabled, every even-numbered vertex
                        /// (every other starting from the first) represents an additional
                        /// vertex for the primitive, while odd-numbered vertices (every other starting from the
                        /// second) represent adjacent vertices.
                        ///
                        /// For example, with `[a, b, c, d, e, f, g, h]`, `[a, c,
                        /// e, g]` form a triangle strip, and `[b, d, f, h]` are the adjacent vertices, where `b`, `d`,
                        /// and `f` are adjacent to the first triangle in the strip, and `d`, `f`, and `h` are adjacent
                        /// to the second.
                        with_adjacency: false,
                        /// Describes whether or not primitive restart is supported for
                        /// an input assembler. Primitive restart is a feature that
                        /// allows a mark to be placed in an index buffer where it is
                        /// is "broken" into multiple pieces of geometry.
                        ///
                        /// See <https://www.khronos.org/opengl/wiki/Vertex_Rendering#Primitive_Restart>
                        /// for more detail.
                        restart_index: None,
                    },
                    // Description of how blend operations should be performed.
                    blender: BlendDesc {
                        /// The logic operation to apply to the blending equation, if any.
                        logic_op: None,
                        /// Which color targets to apply the blending operation to.
                        targets: vec![pso::ColorBlendDesc {
                            mask: pso::ColorMask::ALL,
                            blend: None,
                        }],
                    },
                    // Depth stencil (DSV)
                    depth_stencil: DepthStencilDesc {
                        depth: Some(DepthTest {
                            fun: Comparison::LessEqual,
                            write: true,
                        }),
                        depth_bounds: false,
                        stencil: None,
                    },
                    // Multisampling.
                    multisampling: Some(Multisampling {
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
                    baked_states: BakedStates::default(),
                    // Pipeline layout.
                    layout: &*pipeline_layout,
                    // Subpass in which the pipeline can be executed.
                    subpass,
                    // Options that may be set to alter pipeline properties.
                    flags: PipelineCreationFlags::empty(),
                    /// The parent pipeline, which may be
                    /// `BasePipeline::None`.
                    parent: BasePipeline::None,
                };

                unsafe { device.create_graphics_pipeline(&pipeline_desc, None) }
            };

            unsafe {
                device.destroy_shader_module(ms_module);
            }
            unsafe {
                device.destroy_shader_module(fs_module);
            }

            match pipeline {
                Ok(pipeline) => ManuallyDrop::new(pipeline),
                Err(e) => panic!("Could not compile pipeline {}", e),
            }
        };

        let uniform_buffer = allocator.allocate_buffer(
            Self::UNIFORM_CAMERA_SIZE,
            hal::buffer::Usage::UNIFORM,
            hal::memory::Properties::CPU_VISIBLE,
        );

        let write = vec![pso::DescriptorSetWrite {
            set: &desc_set,
            binding: 0,
            array_offset: 0,
            descriptors: Some(pso::Descriptor::Buffer(
                uniform_buffer.borrow(),
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
        let depth_memory = allocator.allocate_with_reqs(req, memory::Properties::DEVICE_LOCAL);
        let depth_image_view = unsafe {
            device
                .bind_image_memory(depth_memory.borrow(), 0, &mut depth_image)
                .unwrap();

            device
                .create_image_view(
                    &depth_image,
                    image::ViewKind::D2,
                    Self::DEPTH_FORMAT,
                    hal::format::Swizzle::NO,
                    hal::image::SubresourceRange {
                        aspects: hal::format::Aspects::DEPTH,
                        levels: 0..1,
                        layers: 0..1,
                    },
                )
                .unwrap()
        };

        let cmd_pool = unsafe {
            device
                .create_command_pool(
                    queue
                        .lock()
                        .expect("Could not lock queue")
                        .queue_group
                        .family,
                    hal::pool::CommandPoolCreateFlags::empty(),
                )
                .expect("Can't create command pool")
        };

        let material_buffer = allocator.allocate_buffer(
            std::mem::size_of::<DeviceMaterial>() * 32,
            hal::buffer::Usage::UNIFORM | hal::buffer::Usage::TRANSFER_DST,
            hal::memory::Properties::DEVICE_LOCAL,
        );

        let tex_sampler = ManuallyDrop::new(unsafe {
            device
                .create_sampler(&hal::image::SamplerDesc {
                    min_filter: hal::image::Filter::Linear,
                    /// Magnification filter method to use.
                    mag_filter: hal::image::Filter::Nearest,
                    /// Mip filter method to use.
                    mip_filter: hal::image::Filter::Nearest,
                    /// Wrapping mode for each of the U, V, and W axis (S, T, and R in OpenGL
                    /// speak).
                    wrap_mode: (
                        hal::image::WrapMode::Tile,
                        hal::image::WrapMode::Tile,
                        hal::image::WrapMode::Tile,
                    ),
                    /// This bias is added to every computed mipmap level (N + lod_bias). For
                    /// example, if it would select mipmap level 2 and lod_bias is 1, it will
                    /// use mipmap level 3.
                    lod_bias: hal::image::Lod(0.0),
                    /// This range is used to clamp LOD level used for sampling.
                    lod_range: hal::image::Lod(0.0)
                        ..hal::image::Lod(rfw_scene::Texture::MIP_LEVELS as f32),
                    /// Comparison mode, used primary for a shadow map.
                    comparison: None,
                    /// Border color is used when one of the wrap modes is set to border.
                    border: hal::image::PackedColor::from([0.0; 4]),
                    /// Specifies whether the texture coordinates are normalized.
                    normalized: true,
                    /// Anisotropic filtering.
                    /// Can be `Some(_)` only if `Features::SAMPLER_ANISOTROPY` is enabled.
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
            cmd_pool: ManuallyDrop::new(cmd_pool),
            textures: Vec::new(),

            mat_desc_pool,
            mat_set_layout,
            mat_sets: Vec::new(),
            material_buffer,
            tex_sampler,
        }
    }

    pub unsafe fn create_frame_buffer<T: Borrow<B::ImageView>>(
        &self,
        surface_image: &T,
        dimensions: Extent2D,
    ) -> B::Framebuffer {
        self.device
            .create_framebuffer(
                &self.render_pass,
                vec![surface_image.borrow(), &self.depth_image_view],
                hal::image::Extent {
                    width: dimensions.width,
                    height: dimensions.height,
                    depth: 1,
                },
            )
            .unwrap()
    }

    pub fn update_camera(&mut self, camera: &rfw_scene::Camera) {
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
        frame_buffer: &B::Framebuffer,
        viewport: &Viewport,
        scene: &SceneList<B>,
        frustrum: &FrustrumG,
    ) {
        cmd_buffer.bind_graphics_pipeline(&self.pipeline);
        cmd_buffer.bind_graphics_descriptor_sets(
            &self.pipeline_layout,
            0,
            std::iter::once(&self.desc_set),
            &[],
        );

        cmd_buffer.begin_render_pass(
            &self.render_pass,
            frame_buffer,
            viewport.rect,
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

        cmd_buffer.bind_graphics_descriptor_sets(
            &self.pipeline_layout,
            1,
            std::iter::once(&scene.desc_set),
            &[],
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
                    cmd_buffer.bind_vertex_buffers(0, Some((buffer, buffer::SubRange::WHOLE)));
                    first = false;
                }

                cmd_buffer.bind_graphics_descriptor_sets(
                    &self.pipeline_layout,
                    2,
                    std::iter::once(
                        self.mat_sets
                            .get(mesh.mat_id as usize)
                            .expect(format!("Could not get material set {}", mesh.mat_id).as_str()),
                    ),
                    &[],
                );
                cmd_buffer.draw(mesh.first..mesh.last, instance.id..(instance.id + 1));
            });
        });

        cmd_buffer.end_render_pass();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unsafe {
            self.device
                .destroy_image_view(ManuallyDrop::into_inner(ptr::read(&self.depth_image_view)));
            self.device
                .destroy_image(ManuallyDrop::into_inner(ptr::read(&self.depth_image)));
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
                    .allocate_with_reqs(req, memory::Properties::DEVICE_LOCAL);
            }

            self.device
                .bind_image_memory(self.depth_memory.borrow(), 0, &mut image)
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
                        levels: 0..1,
                        layers: 0..1,
                    },
                )
                .unwrap();

            (image, image_view)
        };

        self.depth_image = ManuallyDrop::new(depth_image);
        self.depth_image_view = ManuallyDrop::new(depth_image_view);
    }

    pub fn set_textures(&mut self, textures: &[rfw_scene::Texture]) {
        let mut texels = 0;
        let textures: Vec<_> = textures
            .iter()
            .map(|t| {
                let mut t = t.clone();
                t.generate_mipmaps(5);
                t
            })
            .collect();
        self.textures = textures
            .iter()
            .map(|t| {
                texels += t.data.len();
                SceneTexture::new(
                    self.device.clone(),
                    &self.allocator,
                    t.width,
                    t.height,
                    t.mip_levels,
                    Format::Bgra8Unorm,
                )
            })
            .collect();

        let mut staging_buffer = self.allocator.allocate_buffer(
            texels * std::mem::size_of::<u32>(),
            hal::buffer::Usage::TRANSFER_SRC,
            hal::memory::Properties::CPU_VISIBLE,
        );

        let mut cmd_buffer = unsafe {
            let mut cmd_buffer = self.cmd_pool.allocate_one(hal::command::Level::Primary);

            cmd_buffer.begin_primary(hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);
            cmd_buffer
        };

        if let Ok(mapping) = staging_buffer.map(Segment::ALL) {
            let mut byte_offset = 0;
            for (i, t) in textures.iter().enumerate() {
                let bytes = t.data.as_bytes();
                mapping.as_slice()[byte_offset..(byte_offset + bytes.len())].copy_from_slice(bytes);
                byte_offset += bytes.len();
            }
        }

        let mut byte_offset = 0;
        for (i, t) in textures.iter().enumerate() {
            unsafe {
                cmd_buffer.pipeline_barrier(
                    PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
                    Dependencies::empty(),
                    std::iter::once(&Barrier::Image {
                        range: SubresourceRange {
                            aspects: Aspects::COLOR,
                            layers: 0..1,
                            levels: 0..(t.mip_levels as Level),
                        },
                        families: None,
                        states: (Access::empty(), Layout::Undefined)
                            ..(Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
                        target: self.textures[i].borrow(),
                    }),
                );
            }

            for m in 0..t.mip_levels {
                let (width, height) = t.mip_level_width_height(m as usize);
                unsafe {
                    cmd_buffer.copy_buffer_to_image(
                        staging_buffer.borrow(),
                        self.textures[i].borrow(),
                        hal::image::Layout::TransferDstOptimal,
                        std::iter::once(&hal::command::BufferImageCopy {
                            buffer_offset: byte_offset as hal::buffer::Offset,
                            /// Width of a buffer 'row' in texels.
                            buffer_width: width as u32,
                            /// Height of a buffer 'image slice' in texels.
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
                    PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
                    Dependencies::empty(),
                    std::iter::once(&Barrier::Image {
                        range: SubresourceRange {
                            aspects: Aspects::COLOR,
                            layers: 0..1,
                            levels: 0..(t.mip_levels as Level),
                        },
                        families: None,
                        states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                            ..(Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
                        target: self.textures[i].borrow(),
                    }),
                );
            }
        }

        unsafe {
            cmd_buffer.finish();
        }

        let mut queue = self.queue.lock().expect("Could not get queue lock");

        queue.submit_without_semaphores(std::iter::once(&cmd_buffer), None);
        queue.wait_idle().unwrap();
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
            self.material_buffer = self.allocator.allocate_buffer(
                // Minimum alignment of dynamic uniform buffers is 256 bytes
                materials.len() * 2 * aligned_size,
                hal::buffer::Usage::UNIFORM | hal::buffer::Usage::TRANSFER_DST,
                hal::memory::Properties::DEVICE_LOCAL,
            );
        }

        let mut staging_buffer = self.allocator.allocate_buffer(
            materials.len() * aligned_size,
            hal::buffer::Usage::TRANSFER_SRC,
            hal::memory::Properties::CPU_VISIBLE,
        );

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
                staging_buffer.borrow(),
                self.material_buffer.borrow(),
                std::iter::once(&BufferCopy {
                    size: (materials.len() * aligned_size) as _,
                    src: 0,
                    dst: 0,
                }),
            );

            cmd_buffer.finish();
            cmd_buffer
        };

        if let Ok(mut queue) = self.queue.lock() {
            queue.submit_without_semaphores(std::iter::once(&cmd_buffer), None);
        }

        unsafe {
            if !self.mat_sets.is_empty() {
                let mut sets = Vec::new();
                std::mem::swap(&mut sets, &mut self.mat_sets);
                self.mat_desc_pool.free_sets(sets);
            }
            self.mat_sets = materials
                .iter()
                .enumerate()
                .map(
                    |(i, _)| match self.mat_desc_pool.allocate_set(&self.mat_set_layout) {
                        Ok(set) => set,
                        Err(e) => panic!("Could not allocate set {}, err: {}", i, e),
                    },
                )
                .collect();

            let mut writes = Vec::with_capacity(self.mat_sets.len() * 7);
            let sampler = ManuallyDrop::into_inner(ptr::read(&self.tex_sampler));
            self.mat_sets
                .iter()
                .zip(materials.iter().enumerate())
                .for_each(|(set, (i, mat))| {
                    writes.push(pso::DescriptorSetWrite {
                        set,
                        binding: 0,
                        array_offset: 0,
                        descriptors: Some(pso::Descriptor::Buffer(
                            self.material_buffer.borrow(),
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
                    let tex = &self.textures[mat.diffuse_map.max(0) as usize];
                    writes.push(pso::DescriptorSetWrite {
                        set,
                        binding: 2,
                        array_offset: 0,
                        descriptors: Some(pso::Descriptor::Image(
                            tex.view(),
                            image::Layout::ShaderReadOnlyOptimal,
                        )),
                    });
                    // Texture 1
                    let tex = &self.textures[mat.normal_map.max(0) as usize];
                    writes.push(pso::DescriptorSetWrite {
                        set,
                        binding: 3,
                        array_offset: 0,
                        descriptors: Some(pso::Descriptor::Image(
                            tex.view(),
                            image::Layout::ShaderReadOnlyOptimal,
                        )),
                    });
                    // Texture 2
                    let tex = &self.textures[mat.roughness_map.max(0) as usize];
                    writes.push(pso::DescriptorSetWrite {
                        set,
                        binding: 4,
                        array_offset: 0,
                        descriptors: Some(pso::Descriptor::Image(
                            tex.view(),
                            image::Layout::ShaderReadOnlyOptimal,
                        )),
                    });
                    // Texture 3
                    let tex = &self.textures[mat.emissive_map.max(0) as usize];
                    writes.push(pso::DescriptorSetWrite {
                        set,
                        binding: 5,
                        array_offset: 0,
                        descriptors: Some(pso::Descriptor::Image(
                            tex.view(),
                            image::Layout::ShaderReadOnlyOptimal,
                        )),
                    });
                    // Texture 4
                    let tex = &self.textures[mat.sheen_map.max(0) as usize];
                    writes.push(pso::DescriptorSetWrite {
                        set,
                        binding: 6,
                        array_offset: 0,
                        descriptors: Some(pso::Descriptor::Image(
                            tex.view(),
                            image::Layout::ShaderReadOnlyOptimal,
                        )),
                    });
                });

            self.device.write_descriptor_sets(writes);
        }

        if let Ok(mut queue) = self.queue.lock() {
            queue.wait_idle().unwrap();
        }
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
                .destroy_command_pool(ManuallyDrop::into_inner(ptr::read(&self.cmd_pool)));

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
                .destroy_sampler(ManuallyDrop::into_inner(ptr::read(&self.tex_sampler)));

            self.device
                .destroy_render_pass(ManuallyDrop::into_inner(ptr::read(&self.render_pass)));
            self.device
                .destroy_graphics_pipeline(ManuallyDrop::into_inner(ptr::read(&self.pipeline)));
            self.device
                .destroy_pipeline_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.pipeline_layout,
                )));
        }
    }
}
