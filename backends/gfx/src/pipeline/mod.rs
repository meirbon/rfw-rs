use crate::hal;

use hal::*;
use std::mem::ManuallyDrop;
use std::sync::Arc;

pub mod shader;

/*
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
*/

pub struct RenderPass<B: hal::Backend> {
    device: Arc<B::Device>,
    raw: ManuallyDrop<B::RenderPass>,
}

pub struct RenderPassBuilder<B: hal::Backend> {}

pub struct GraphicsPipeline<B: hal::Backend> {
    label: Option<String>,
    device: Arc<B::Device>,
    pipeline: ManuallyDrop<B::GraphicsPipeline>,
}

pub struct GraphicsPipelineDescriptor<'a, B: hal::Backend> {
    vertex: shader::Module<'a, B>,
    fragment: shader::Module<'a, B>,
}

pub struct ComputePipeline<B: hal::Backend> {
    label: Option<String>,
    device: Arc<B::Device>,
    pipeline: ManuallyDrop<B::ComputePipeline>,
}

/*
 let (ms_entry, fs_entry) = (

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
                    vertex_buffers: vec![
                        VertexBufferDesc {
                            binding: 0 as BufferIndex,
                            stride: std::mem::size_of::<VertexData>() as ElemStride,
                            rate: VertexInputRate::Vertex,
                        },
                        VertexBufferDesc {
                            binding: 1 as BufferIndex,
                            stride: std::mem::size_of::<AnimVertexData>() as ElemStride,
                            rate: VertexInputRate::Vertex,
                        },
                    ],
                    // Vertex attributes (IA)
                    attributes: vec![
                        AttributeDesc {
                            /// Vertex array location
                            location: 0 as Location,
                            /// Binding number of the associated vertex mem.
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
                            /// Binding number of the associated vertex mem.
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
                            /// Binding number of the associated vertex mem.
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
                            /// Binding number of the associated vertex mem.
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
                            /// Binding number of the associated vertex mem.
                            binding: 0 as BufferIndex,
                            /// Attribute element description.
                            element: Element {
                                format: hal::format::Format::Rgba32Sfloat,
                                offset: 40,
                            },
                        },
                        AttributeDesc {
                            /// Vertex array location
                            location: 5 as Location,
                            /// Binding number of the associated vertex mem.
                            binding: 1 as BufferIndex,
                            /// Attribute element description.
                            element: Element {
                                format: hal::format::Format::Rgba32Uint,
                                offset: 0,
                            },
                        },
                        AttributeDesc {
                            /// Vertex array location
                            location: 6 as Location,
                            /// Binding number of the associated vertex mem.
                            binding: 1 as BufferIndex,
                            /// Attribute element description.
                            element: Element {
                                format: hal::format::Format::Rgba32Sfloat,
                                offset: 16,
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
                        /// allows a mark to be placed in an index mem where it is
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
*/
