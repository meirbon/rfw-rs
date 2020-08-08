use crate::buffer::{Allocator, Buffer, Memory};
use crate::{hal, instances::SceneList};
use gfx_hal::image::{Kind, Tiling};
use glam::*;
use hal::{
    buffer,
    command::{self, CommandBuffer, DescriptorSetOffset},
    device::Device,
    image, memory, pass, pso,
    window::Extent2D,
};
use pass::Subpass;
use pso::*;
use rfw_scene::{Mesh, VertexData};
use shared::BytesConversion;
use std::{borrow::Borrow, mem::ManuallyDrop, ptr, sync::Arc};

pub mod anim;

#[derive(Debug, Clone)]
pub struct GfxMesh<B: hal::Backend> {
    pub buffer: Option<Arc<Buffer<B>>>,
    vertices: usize,
}

impl<B: hal::Backend> Default for GfxMesh<B> {
    fn default() -> Self {
        Self {
            buffer: None,
            vertices: 0,
        }
    }
}

#[allow(dead_code)]
impl<B: hal::Backend> GfxMesh<B> {
    pub fn new(allocator: &Allocator<B>, mesh: &Mesh) -> Self {
        let mut m = Self::default();
        m.set_data(allocator, mesh);
        m
    }

    pub fn set_data(&mut self, allocator: &Allocator<B>, mesh: &Mesh) {
        if mesh.vertices.is_empty() {
            *self = Self::default();
        }

        let non_coherent_alignment = allocator.limits.non_coherent_atom_size as u64;

        let buffer_len = (mesh.vertices.len() * std::mem::size_of::<VertexData>()) as u64;
        assert_ne!(buffer_len, 0);
        let padded_buffer_len = ((buffer_len + non_coherent_alignment - 1)
            / non_coherent_alignment)
            * non_coherent_alignment;

        // TODO: We should use staging buffers to transfer data to vertex buffers
        let mut buffer = allocator.allocate_bytes(
            padded_buffer_len as usize,
            buffer::Usage::VERTEX,
            memory::Properties::CPU_VISIBLE,
        );

        if let Ok(mapping) = buffer.map(memory::Segment {
            offset: 0,
            size: Some(buffer_len),
        }) {
            mapping.as_slice().copy_from_slice(mesh.vertices.as_bytes());
        }

        self.vertices = mesh.vertices.len();
        self.buffer = Some(Arc::new(buffer));
    }

    pub fn len(&self) -> usize {
        self.vertices
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
    depth_image: Option<ManuallyDrop<B::Image>>,
    depth_image_view: Option<ManuallyDrop<B::ImageView>>,
    depth_memory: Memory<B>,
}

impl<B: hal::Backend> RenderPipeline<B> {
    const DEPTH_FORMAT: hal::format::Format = hal::format::Format::D32Sfloat;
    const UNIFORM_CAMERA_SIZE: usize = std::mem::size_of::<Mat4>() * 2
        + std::mem::size_of::<[u32; 4]>()
        + std::mem::size_of::<Vec4>();

    pub fn new(
        device: Arc<B::Device>,
        allocator: Allocator<B>,
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
                device.create_pipeline_layout(vec![&*set_layout, &*scene_list.set_layout], &[])
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

        let uniform_buffer = allocator.allocate_bytes(
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
            depth_image: Some(ManuallyDrop::new(depth_image)),
            depth_image_view: Some(ManuallyDrop::new(depth_image_view)),
            depth_memory,
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
                vec![
                    surface_image.borrow(),
                    self.depth_image_view.as_ref().unwrap(),
                ],
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

        scene.iter_instances(|buffer, offset, instance, range| {
            cmd_buffer.bind_vertex_buffers(0, Some((buffer, buffer::SubRange::WHOLE)));
            cmd_buffer.bind_graphics_descriptor_sets(
                &self.pipeline_layout,
                1,
                vec![&scene.desc_set],
                &[offset as DescriptorSetOffset],
            );
            cmd_buffer.draw(range, instance.id..(instance.id + 1));
        });

        cmd_buffer.end_render_pass();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let mut image = None;
        let mut image_view = None;

        std::mem::swap(&mut image, &mut self.depth_image);
        std::mem::swap(&mut image_view, &mut self.depth_image_view);

        unsafe {
            if let Some(view) = image_view {
                self.device
                    .destroy_image_view(ManuallyDrop::into_inner(view));
            }

            if let Some(image) = image {
                self.device.destroy_image(ManuallyDrop::into_inner(image));
            }
        }

        let allocate =
            self.depth_memory.len() < (width * height) as usize * std::mem::size_of::<f32>();

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
            if allocate {
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

        self.depth_image = Some(ManuallyDrop::new(depth_image));
        self.depth_image_view = Some(ManuallyDrop::new(depth_image_view));
    }
}

impl<B: hal::Backend> Drop for RenderPipeline<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();

        unsafe {
            let mut image = None;
            let mut image_view = None;

            std::mem::swap(&mut image, &mut self.depth_image);
            std::mem::swap(&mut image_view, &mut self.depth_image_view);

            if let Some(view) = image_view {
                self.device
                    .destroy_image_view(ManuallyDrop::into_inner(view));
            }

            if let Some(image) = image {
                self.device.destroy_image(ManuallyDrop::into_inner(image));
            }

            self.device
                .destroy_descriptor_pool(ManuallyDrop::into_inner(ptr::read(&self.desc_pool)));
            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.set_layout,
                )));
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
