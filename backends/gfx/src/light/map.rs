use crate::{DeviceHandle, hal};
use crate::instances::{RenderBuffers, SceneList};
use crate::skinning::SkinList;
use hal::*;
use hal::{command::CommandBuffer, device::Device, pso::DescriptorPool};
use rfw::prelude::*;
use std::{fmt::Debug, mem::ManuallyDrop, sync::Arc};

#[derive(Debug)]
pub struct Array<B: hal::Backend, T: Sized + Light + Clone + Debug + Default> {
    allocator: crate::mem::Allocator<B>,
    lights: TrackedStorage<T>,
    info: Vec<LightInfo>,
    shadow_maps: ShadowMapArray<B>,
}

impl<B: hal::Backend, T: Sized + Light + Clone + Debug + Default> Array<B, T> {
    pub fn new(
        device: DeviceHandle<B>,
        allocator: crate::mem::Allocator<B>,
        filter_pipeline: Arc<FilterPipeline<B>>,
        pipeline_layout: &B::PipelineLayout,
        depth_type: DepthType,
        capacity: usize,
    ) -> Self {
        Self {
            allocator: allocator.clone(),
            lights: TrackedStorage::new(),
            info: Vec::with_capacity(capacity),
            shadow_maps: ShadowMapArray::new(
                device,
                allocator,
                filter_pipeline,
                pipeline_layout,
                depth_type,
                capacity,
            ),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DepthType {
    Linear,
    Perspective,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ShadowMapArray<B: hal::Backend> {
    device: DeviceHandle<B>,

    pub map: ManuallyDrop<B::Image>,
    render_views: Vec<ManuallyDrop<B::ImageView>>,
    frame_buffers: Vec<ManuallyDrop<B::Framebuffer>>,
    pub view: ManuallyDrop<B::ImageView>,
    map_memory: crate::mem::Memory<B>,

    filter_map: ManuallyDrop<B::Image>,
    filter_view: ManuallyDrop<B::ImageView>,
    filter_map_memory: crate::mem::Memory<B>,

    depth_map: ManuallyDrop<B::Image>,
    depth_view: ManuallyDrop<B::ImageView>,
    depth_map_memory: crate::mem::Memory<B>,
    pub uniform_buffer: crate::mem::Buffer<B>,

    filter_pipeline: Arc<FilterPipeline<B>>,
    render_pass: ManuallyDrop<B::RenderPass>,
    pipeline: ManuallyDrop<B::GraphicsPipeline>,
    anim_pipeline: ManuallyDrop<B::GraphicsPipeline>,
    light_infos: Vec<LightInfo>,
}

impl<B: hal::Backend> ShadowMapArray<B> {
    pub const WIDTH: u32 = 2048;
    pub const HEIGHT: u32 = 2048;
    pub const FORMAT: format::Format = format::Format::Rg32Sfloat;
    pub const DEPTH_FORMAT: format::Format = format::Format::D32Sfloat;

    pub fn new(
        device: DeviceHandle<B>,
        allocator: crate::mem::Allocator<B>,
        filter_pipeline: Arc<FilterPipeline<B>>,
        pipeline_layout: &B::PipelineLayout,
        depth_type: DepthType,
        capacity: usize,
    ) -> Self {
        unsafe {
            let mut map = device
                .create_image(
                    image::Kind::D2(Self::WIDTH, Self::HEIGHT, capacity as u16, 1),
                    1 as image::Level,
                    Self::FORMAT,
                    image::Tiling::Optimal,
                    image::Usage::COLOR_ATTACHMENT | image::Usage::SAMPLED,
                    image::ViewCapabilities::KIND_2D_ARRAY,
                )
                .unwrap();
            let req = device.get_image_requirements(&map);
            let map_memory = allocator
                .allocate_with_reqs(req, memory::Properties::DEVICE_LOCAL, None)
                .unwrap();
            device
                .bind_image_memory(map_memory.memory(), 0, &mut map)
                .unwrap();

            let render_views: Vec<_> = (0..capacity)
                .map(|i| {
                    ManuallyDrop::new(
                        device
                            .create_image_view(
                                &map,
                                image::ViewKind::D2,
                                Self::FORMAT,
                                format::Swizzle::NO,
                                image::SubresourceRange {
                                    aspects: format::Aspects::COLOR,
                                    level_start: 0,
                                    level_count: Some(1),
                                    layer_start: i as _,
                                    layer_count: Some((i + 1) as _),
                                },
                            )
                            .unwrap(),
                    )
                })
                .collect();

            let view = device
                .create_image_view(
                    &map,
                    image::ViewKind::D2Array,
                    Self::FORMAT,
                    format::Swizzle::NO,
                    image::SubresourceRange {
                        aspects: format::Aspects::COLOR,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(capacity as _),
                    },
                )
                .unwrap();

            let mut filter_map = device
                .create_image(
                    image::Kind::D2(Self::WIDTH, Self::HEIGHT, capacity as u16, 1),
                    1 as image::Level,
                    Self::FORMAT,
                    image::Tiling::Optimal,
                    image::Usage::COLOR_ATTACHMENT | image::Usage::SAMPLED,
                    image::ViewCapabilities::KIND_2D_ARRAY,
                )
                .unwrap();
            let req = device.get_image_requirements(&map);
            let filter_map_memory = allocator
                .allocate_with_reqs(req, memory::Properties::DEVICE_LOCAL, None)
                .unwrap();
            device
                .bind_image_memory(filter_map_memory.memory(), 0, &mut filter_map)
                .unwrap();

            let filter_view = device
                .create_image_view(
                    &map,
                    image::ViewKind::D2Array,
                    Self::FORMAT,
                    format::Swizzle::NO,
                    image::SubresourceRange {
                        aspects: format::Aspects::COLOR | format::Aspects::DEPTH,
                        level_start: 0,
                        level_count: Some(1),
                        layer_start: 0,
                        layer_count: Some(capacity as _),
                    },
                )
                .unwrap();

            let (depth_map, depth_view, depth_map_memory) = {
                let mut image = device
                    .create_image(
                        image::Kind::D2(Self::WIDTH, Self::HEIGHT, 1, 1),
                        1,
                        Self::DEPTH_FORMAT,
                        image::Tiling::Optimal,
                        image::Usage::DEPTH_STENCIL_ATTACHMENT,
                        image::ViewCapabilities::empty(),
                    )
                    .expect("Could not create depth image.");

                let req = device.get_image_requirements(&image);
                let depth_memory = allocator
                    .allocate_with_reqs(req, memory::Properties::DEVICE_LOCAL, None)
                    .unwrap();

                device
                    .bind_image_memory(depth_memory.memory(), 0, &mut image)
                    .unwrap();

                let image_view = device
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

                (image, image_view, depth_memory)
            };

            let uniform_buffer = allocator
                .allocate_buffer(
                    (capacity * std::mem::size_of::<LightInfo>()) as _,
                    hal::buffer::Usage::UNIFORM,
                    hal::memory::Properties::CPU_VISIBLE,
                    Some(
                        hal::memory::Properties::CPU_VISIBLE
                            | hal::memory::Properties::DEVICE_LOCAL,
                    ),
                )
                .unwrap();

            let vert_shader: &[u8] = include_bytes!("../../shaders/shadow_mesh.vert");
            let anim_vert_shader: &[u8] = include_bytes!("../../shaders/shadow_mesh_anim.vert");
            let frag_linear: &[u8] = include_bytes!("../../shaders/shadow_linear.frag");
            let frag_perspective: &[u8] = include_bytes!("../../shaders/shadow_perspective.frag");

            let vert_module = device
                .create_shader_module(vert_shader.as_quad_bytes())
                .unwrap();
            let anim_vert_module = device
                .create_shader_module(anim_vert_shader.as_quad_bytes())
                .unwrap();
            let frag_module = match depth_type {
                DepthType::Linear => device
                    .create_shader_module(frag_linear.as_quad_bytes())
                    .unwrap(),
                DepthType::Perspective => device
                    .create_shader_module(frag_perspective.as_quad_bytes())
                    .unwrap(),
            };

            let color_attachment = pass::Attachment {
                format: Some(Self::FORMAT),
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

            let render_pass = device
                .create_render_pass(&[color_attachment, depth_attachment], &[subpass], &[])
                .unwrap();

            let pipeline = device
                .create_graphics_pipeline(
                    &pso::GraphicsPipelineDesc {
                        primitive_assembler: pso::PrimitiveAssemblerDesc::Vertex {
                            buffers: &[pso::VertexBufferDesc {
                                binding: 0 as pso::BufferIndex,
                                stride: std::mem::size_of::<VertexData>() as pso::ElemStride,
                                rate: pso::VertexInputRate::Vertex,
                            }],
                            /// Vertex attributes (IA)
                            attributes: &[pso::AttributeDesc {
                                /// Vertex array location
                                location: 0 as pso::Location,
                                /// Binding number of the associated vertex mem.
                                binding: 0 as pso::BufferIndex,
                                /// Attribute element description.
                                element: pso::Element {
                                    format: hal::format::Format::Rgba32Sfloat,
                                    offset: 0,
                                },
                            }],
                            input_assembler: pso::InputAssemblerDesc {
                                primitive: pso::Primitive::TriangleList,
                                with_adjacency: false,
                                restart_index: None,
                            },
                            vertex: pso::EntryPoint {
                                specialization: pso::Specialization::EMPTY,
                                module: &vert_module,
                                entry: "main",
                            },
                            tessellation: None,
                            geometry: None,
                        },
                        fragment: Some(pso::EntryPoint {
                            specialization: pso::Specialization::EMPTY,
                            module: &frag_module,
                            entry: "main",
                        }),
                        rasterizer: pso::Rasterizer {
                            polygon_mode: pso::PolygonMode::Fill,
                            cull_face: pso::Face::BACK,
                            front_face: pso::FrontFace::CounterClockwise,
                            depth_clamping: false,
                            depth_bias: None,
                            conservative: false,
                            line_width: pso::State::Static(1.0),
                        },
                        blender: Default::default(),
                        depth_stencil: pso::DepthStencilDesc {
                            depth: Some(pso::DepthTest {
                                fun: pso::Comparison::LessEqual,
                                write: true,
                            }),
                            depth_bounds: false,
                            stencil: None,
                        },
                        multisampling: None,
                        baked_states: pso::BakedStates {
                            blend_color: None,
                            depth_bounds: Some(0.0_f32..1.0_f32),
                            scissor: Some(pso::Rect {
                                x: 0,
                                y: 0,
                                w: Self::WIDTH as _,
                                h: Self::HEIGHT as _,
                            }),
                            viewport: Some(pso::Viewport {
                                depth: 0.0..1.0,
                                rect: pso::Rect {
                                    x: 0,
                                    y: 0,
                                    w: Self::WIDTH as _,
                                    h: Self::HEIGHT as _,
                                },
                            }),
                        },
                        layout: pipeline_layout,
                        subpass: pass::Subpass {
                            index: 0,
                            main_pass: &render_pass,
                        },
                        flags: pso::PipelineCreationFlags::empty(),
                        parent: pso::BasePipeline::None,
                    },
                    None,
                )
                .unwrap();

            let anim_pipeline = device
                .create_graphics_pipeline(
                    &pso::GraphicsPipelineDesc {
                        primitive_assembler: pso::PrimitiveAssemblerDesc::Vertex {
                            buffers: &[
                                pso::VertexBufferDesc {
                                    binding: 0 as pso::BufferIndex,
                                    stride: std::mem::size_of::<VertexData>() as pso::ElemStride,
                                    rate: pso::VertexInputRate::Vertex,
                                },
                                pso::VertexBufferDesc {
                                    binding: 1 as pso::BufferIndex,
                                    stride: std::mem::size_of::<AnimVertexData>()
                                        as pso::ElemStride,
                                    rate: pso::VertexInputRate::Vertex,
                                },
                            ],
                            attributes: &[
                                pso::AttributeDesc {
                                    location: 0 as pso::Location,
                                    binding: 0 as pso::BufferIndex,
                                    element: pso::Element {
                                        format: hal::format::Format::Rgba32Sfloat,
                                        offset: 0,
                                    },
                                },
                                pso::AttributeDesc {
                                    /// Vertex array location
                                    location: 1 as pso::Location,
                                    /// Binding number of the associated vertex mem.
                                    binding: 1 as pso::BufferIndex,
                                    /// Attribute element description.
                                    element: pso::Element {
                                        format: hal::format::Format::Rgba32Uint,
                                        offset: 0,
                                    },
                                },
                                pso::AttributeDesc {
                                    location: 2 as pso::Location,
                                    binding: 1 as pso::BufferIndex,
                                    element: pso::Element {
                                        format: hal::format::Format::Rgba32Sfloat,
                                        offset: 16,
                                    },
                                },
                            ],
                            input_assembler: pso::InputAssemblerDesc {
                                primitive: pso::Primitive::TriangleList,
                                with_adjacency: false,
                                restart_index: None,
                            },
                            vertex: pso::EntryPoint {
                                specialization: pso::Specialization::EMPTY,
                                module: &anim_vert_module,
                                entry: "main",
                            },
                            tessellation: None,
                            geometry: None,
                        },
                        fragment: Some(pso::EntryPoint {
                            specialization: pso::Specialization::EMPTY,
                            module: &frag_module,
                            entry: "main",
                        }),
                        rasterizer: pso::Rasterizer {
                            polygon_mode: pso::PolygonMode::Fill,
                            cull_face: pso::Face::BACK,
                            front_face: pso::FrontFace::CounterClockwise,
                            depth_clamping: false,
                            depth_bias: None,
                            conservative: false,
                            line_width: pso::State::Static(1.0),
                        },
                        blender: Default::default(),
                        depth_stencil: pso::DepthStencilDesc {
                            depth: Some(pso::DepthTest {
                                fun: pso::Comparison::LessEqual,
                                write: true,
                            }),
                            depth_bounds: false,
                            stencil: None,
                        },
                        multisampling: None,
                        baked_states: pso::BakedStates {
                            blend_color: None,
                            depth_bounds: Some(0.0_f32..1.0_f32),
                            scissor: Some(pso::Rect {
                                x: 0,
                                y: 0,
                                w: Self::WIDTH as _,
                                h: Self::HEIGHT as _,
                            }),
                            viewport: Some(pso::Viewport {
                                depth: 0.0..1.0,
                                rect: pso::Rect {
                                    x: 0,
                                    y: 0,
                                    w: Self::WIDTH as _,
                                    h: Self::HEIGHT as _,
                                },
                            }),
                        },
                        layout: pipeline_layout,
                        subpass: pass::Subpass {
                            index: 0,
                            main_pass: &render_pass,
                        },
                        flags: pso::PipelineCreationFlags::empty(),
                        parent: pso::BasePipeline::None,
                    },
                    None,
                )
                .unwrap();

            let frame_buffers: Vec<_> = (0..capacity)
                .map(|i| {
                    ManuallyDrop::new(
                        device
                            .create_framebuffer(
                                &render_pass,
                                vec![&*render_views[i], &depth_view],
                                image::Extent {
                                    width: Self::WIDTH,
                                    height: Self::HEIGHT,
                                    depth: 1,
                                },
                            )
                            .unwrap(),
                    )
                })
                .collect();

            Self {
                device,

                map: ManuallyDrop::new(map),
                render_views,
                frame_buffers,
                view: ManuallyDrop::new(view),
                map_memory,

                filter_map: ManuallyDrop::new(filter_map),
                filter_view: ManuallyDrop::new(filter_view),
                filter_map_memory,

                depth_map: ManuallyDrop::new(depth_map),
                depth_view: ManuallyDrop::new(depth_view),
                depth_map_memory,

                uniform_buffer,

                filter_pipeline,
                render_pass: ManuallyDrop::new(render_pass),
                pipeline: ManuallyDrop::new(pipeline),
                anim_pipeline: ManuallyDrop::new(anim_pipeline),
                light_infos: Vec::new(),
            }
        }
    }

    pub fn render(
        &self,
        cmd_buffer: &mut B::CommandBuffer,
        pipeline_layout: &B::PipelineLayout,
        desc_set: &B::DescriptorSet,
        scene: &SceneList<B>,
        skins: &SkinList<B>,
    ) {
        for i in 0..self.light_infos.len() {
            let frustrum = FrustrumG::from_matrix(self.light_infos[i].pm);

            unsafe {
                cmd_buffer.begin_render_pass(
                    &*self.render_pass,
                    &*self.frame_buffers[i],
                    pso::Rect {
                        x: 0,
                        y: 0,
                        w: Self::WIDTH as _,
                        h: Self::HEIGHT as _,
                    },
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
                    let iter = instance
                        .meshes
                        .iter()
                        .filter(|m| frustrum.aabb_in_frustrum(&m.bounds).should_render());

                    let mut first = true;
                    iter.for_each(|mesh| {
                        if first {
                            cmd_buffer.bind_graphics_descriptor_sets(
                                pipeline_layout,
                                0,
                                std::iter::once(desc_set),
                                &[],
                            );

                            cmd_buffer.bind_graphics_descriptor_sets(
                                pipeline_layout,
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
                                RenderBuffers::Animated(buffer, anim_offset) => {
                                    if let Some(skin_id) = instance.skin_id {
                                        let skin_id = skin_id as usize;
                                        if let Some(skin_set) = skins.get_set(skin_id) {
                                            cmd_buffer.bind_graphics_pipeline(&self.anim_pipeline);
                                            cmd_buffer.bind_graphics_descriptor_sets(
                                                pipeline_layout,
                                                1,
                                                vec![&scene.desc_set, skin_set],
                                                &[],
                                            );

                                            cmd_buffer.bind_vertex_buffers(
                                                0,
                                                std::iter::once((
                                                    buffer.buffer(),
                                                    buffer::SubRange {
                                                        size: Some(*anim_offset as buffer::Offset),
                                                        offset: 0,
                                                    },
                                                )),
                                            );
                                            cmd_buffer.bind_vertex_buffers(
                                                1,
                                                std::iter::once((
                                                    buffer.buffer(),
                                                    buffer::SubRange {
                                                        size: Some(
                                                            (buffer.size_in_bytes - *anim_offset)
                                                                as buffer::Offset,
                                                        ),
                                                        offset: *anim_offset as _,
                                                    },
                                                )),
                                            );
                                        } else {
                                            cmd_buffer.bind_graphics_pipeline(&self.pipeline);
                                            cmd_buffer.bind_vertex_buffers(
                                                0,
                                                std::iter::once((
                                                    buffer.buffer(),
                                                    buffer::SubRange::WHOLE,
                                                )),
                                            );
                                        }
                                    } else {
                                        cmd_buffer.bind_graphics_pipeline(&self.pipeline);
                                        cmd_buffer.bind_vertex_buffers(
                                            0,
                                            std::iter::once((
                                                buffer.buffer(),
                                                buffer::SubRange::WHOLE,
                                            )),
                                        );
                                    }
                                }
                            }
                            first = false;
                        }

                        cmd_buffer.draw(mesh.first..mesh.last, instance.id..(instance.id + 1));
                    });
                });
            }
        }
    }
}

impl<B: hal::Backend> Drop for ShadowMapArray<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();
        unsafe {
            self.frame_buffers.iter().for_each(|f| {
                self.device
                    .destroy_framebuffer(ManuallyDrop::into_inner(std::ptr::read(f)));
            });
            self.render_views.iter().for_each(|v| {
                self.device
                    .destroy_image_view(ManuallyDrop::into_inner(std::ptr::read(v)));
            });

            self.device
                .destroy_image_view(ManuallyDrop::into_inner(std::ptr::read(&self.view)));
            self.device
                .destroy_image_view(ManuallyDrop::into_inner(std::ptr::read(&self.filter_view)));
            self.device
                .destroy_image_view(ManuallyDrop::into_inner(std::ptr::read(&self.depth_view)));

            self.device
                .destroy_image(ManuallyDrop::into_inner(std::ptr::read(&self.map)));
            self.device
                .destroy_image(ManuallyDrop::into_inner(std::ptr::read(&self.filter_map)));
            self.device
                .destroy_image(ManuallyDrop::into_inner(std::ptr::read(&self.depth_map)));

            self.device
                .destroy_graphics_pipeline(ManuallyDrop::into_inner(std::ptr::read(
                    &self.pipeline,
                )));
            self.device
                .destroy_graphics_pipeline(ManuallyDrop::into_inner(std::ptr::read(
                    &self.anim_pipeline,
                )));

            self.device
                .destroy_render_pass(ManuallyDrop::into_inner(std::ptr::read(&self.render_pass)));
        }
    }
}

#[derive(Debug)]
pub struct FilterPipeline<B: hal::Backend> {
    device: DeviceHandle<B>,
    desc_pool: ManuallyDrop<B::DescriptorPool>,
    desc_layout: ManuallyDrop<B::DescriptorSetLayout>,
    pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    pipeline: ManuallyDrop<B::ComputePipeline>,
}

#[derive(Debug, Copy, Clone)]
struct FilterPushConstant {
    direction: [f32; 2],
    layer: u32,
}

impl<'a> FilterPushConstant {
    pub fn as_bytes(&'a self) -> &'a [u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const FilterPushConstant as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }

    pub fn as_quad_bytes(&'a self) -> &'a [u32] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const FilterPushConstant as *const u32,
                std::mem::size_of::<Self>() / 4,
            )
        }
    }
}

impl<B: hal::Backend> FilterPipeline<B> {
    pub fn new(device: DeviceHandle<B>) -> Self {
        unsafe {
            // 4 Types of lights, thus max 4 sets
            let desc_pool = device
                .create_descriptor_pool(
                    8,
                    &[
                        pso::DescriptorRangeDesc {
                            count: 8,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: false },
                            },
                        },
                        pso::DescriptorRangeDesc {
                            count: 8,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: true },
                            },
                        },
                    ],
                    pso::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET,
                )
                .unwrap();
            let desc_layout = device
                .create_descriptor_set_layout(
                    &[
                        pso::DescriptorSetLayoutBinding {
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: false },
                            },
                            binding: 0,
                            count: 1,
                            immutable_samplers: false,
                            stage_flags: pso::ShaderStageFlags::COMPUTE,
                        },
                        pso::DescriptorSetLayoutBinding {
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: true },
                            },
                            binding: 1,
                            count: 1,
                            immutable_samplers: false,
                            stage_flags: pso::ShaderStageFlags::COMPUTE,
                        },
                    ],
                    &[],
                )
                .unwrap();

            let pipeline_layout = device
                .create_pipeline_layout(
                    std::iter::once(&desc_layout),
                    std::iter::once(&(pso::ShaderStageFlags::COMPUTE, 0..12)),
                )
                .unwrap();

            let spirv: &[u8] = include_bytes!("../../shaders/shadow_filter.comp.spv");
            let shader = device.create_shader_module(spirv.as_quad_bytes()).unwrap();

            let pipeline = device
                .create_compute_pipeline(
                    &pso::ComputePipelineDesc {
                        shader: pso::EntryPoint {
                            entry: "main",
                            module: &shader,
                            specialization: pso::Specialization::EMPTY,
                        },
                        layout: &pipeline_layout,
                        flags: pso::PipelineCreationFlags::empty(),
                        parent: pso::BasePipeline::None,
                    },
                    None,
                )
                .unwrap();

            Self {
                device,
                desc_pool: ManuallyDrop::new(desc_pool),
                desc_layout: ManuallyDrop::new(desc_layout),
                pipeline_layout: ManuallyDrop::new(pipeline_layout),
                pipeline: ManuallyDrop::new(pipeline),
            }
        }
    }

    pub fn launch(&self, cmd_buffer: &mut B::CommandBuffer, set: &FilterDescSet<B>, layer: u32) {
        unsafe {
            // Filter in x direction
            let push_constants = FilterPushConstant {
                direction: [1.0, 0.0],
                layer,
            };
            cmd_buffer.bind_compute_pipeline(&*self.pipeline);
            cmd_buffer.bind_compute_descriptor_sets(
                &*self.pipeline_layout,
                0,
                std::iter::once(&*set.set1),
                &[],
            );
            cmd_buffer.push_compute_constants(
                &*self.pipeline_layout,
                0,
                push_constants.as_quad_bytes(),
            );
            cmd_buffer.dispatch([
                (set.width as f32 / 16.0).ceil() as u32,
                (set.height as f32 / 16.0_f32).ceil() as u32,
                1,
            ]);

            // Filter in y direction
            let push_constants = FilterPushConstant {
                direction: [0.0, 1.0],
                layer,
            };
            cmd_buffer.bind_compute_descriptor_sets(
                &self.pipeline_layout,
                0,
                std::iter::once(&*set.set2),
                &[],
            );
            cmd_buffer.push_compute_constants(
                &self.pipeline_layout,
                0,
                push_constants.as_quad_bytes(),
            );
            cmd_buffer.dispatch([
                (set.width as f32 / 16.0).ceil() as u32,
                (set.height as f32 / 16.0_f32).ceil() as u32,
                1,
            ]);
        }
    }

    pub fn allocate_set(
        &mut self,
        width: u32,
        height: u32,
        source_output: &B::ImageView,
        filter_view: &B::ImageView,
    ) -> FilterDescSet<B> {
        unsafe {
            let set1 = self.desc_pool.allocate_set(&self.desc_layout).unwrap();
            let set2 = self.desc_pool.allocate_set(&self.desc_layout).unwrap();

            let writes = vec![
                pso::DescriptorSetWrite {
                    binding: 0,
                    array_offset: 0,
                    descriptors: vec![
                        pso::Descriptor::Image(filter_view, image::Layout::General),
                        pso::Descriptor::Image(source_output, image::Layout::General),
                    ],
                    set: &set1,
                },
                pso::DescriptorSetWrite {
                    binding: 1,
                    array_offset: 0,
                    descriptors: vec![
                        pso::Descriptor::Image(source_output, image::Layout::General),
                        pso::Descriptor::Image(filter_view, image::Layout::General),
                    ],
                    set: &set2,
                },
            ];

            self.device.write_descriptor_sets(writes);

            FilterDescSet {
                width,
                height,
                set1: ManuallyDrop::new(set1),
                set2: ManuallyDrop::new(set2),
            }
        }
    }

    pub fn update_set(
        &mut self,
        source_output: &B::ImageView,
        filter_view: &B::ImageView,
        set: &FilterDescSet<B>,
    ) {
        let writes = vec![
            pso::DescriptorSetWrite {
                binding: 0,
                array_offset: 0,
                descriptors: vec![
                    pso::Descriptor::Image(filter_view, image::Layout::General),
                    pso::Descriptor::Image(source_output, image::Layout::General),
                ],
                set: &*set.set1,
            },
            pso::DescriptorSetWrite {
                binding: 1,
                array_offset: 0,
                descriptors: vec![
                    pso::Descriptor::Image(source_output, image::Layout::General),
                    pso::Descriptor::Image(filter_view, image::Layout::General),
                ],
                set: &*set.set2,
            },
        ];

        unsafe { self.device.write_descriptor_sets(writes) };
    }

    pub fn free_set(&mut self, set: FilterDescSet<B>) {
        let sets = vec![
            ManuallyDrop::into_inner(set.set1),
            ManuallyDrop::into_inner(set.set2),
        ];

        unsafe {
            self.desc_pool.free(sets);
        }
    }
}

pub struct FilterDescSet<B: hal::Backend> {
    width: u32,
    height: u32,
    set1: ManuallyDrop<B::DescriptorSet>,
    set2: ManuallyDrop<B::DescriptorSet>,
}

impl<B: hal::Backend> Drop for FilterPipeline<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();
        unsafe {
            self.desc_pool.reset();
            self.device
                .destroy_descriptor_pool(ManuallyDrop::into_inner(std::ptr::read(&self.desc_pool)));
            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(std::ptr::read(
                    &self.desc_layout,
                )));
            self.device
                .destroy_compute_pipeline(ManuallyDrop::into_inner(std::ptr::read(&self.pipeline)));
            self.device
                .destroy_pipeline_layout(ManuallyDrop::into_inner(std::ptr::read(
                    &self.pipeline_layout,
                )));
        }
    }
}
