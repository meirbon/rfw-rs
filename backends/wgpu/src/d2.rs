use crate::{
    list::{InstanceList, VertexList},
    WgpuTexture,
};
use rfw::prelude::*;
use std::sync::Arc;

#[derive(Debug)]
pub struct InstanceList2D {
    instance_capacity: usize,
    instances: u32,
    pub instances_buffer: Arc<Option<wgpu::Buffer>>,
}

impl Default for InstanceList2D {
    fn default() -> Self {
        Self {
            instance_capacity: 0,
            instances: 0,
            instances_buffer: Arc::new(None),
        }
    }
}

impl Clone for InstanceList2D {
    fn clone(&self) -> Self {
        Self {
            instance_capacity: self.instance_capacity,
            instances: self.instances,
            instances_buffer: self.instances_buffer.clone(),
        }
    }
}

#[allow(dead_code)]
impl InstanceList2D {
    const DEFAULT_CAPACITY: usize = 4;

    pub fn new(device: &wgpu::Device) -> Self {
        let instances_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (Self::DEFAULT_CAPACITY * std::mem::size_of::<Mat4>()) as _,
            usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        }));

        Self {
            instance_capacity: Self::DEFAULT_CAPACITY as _,
            instances: 0,
            instances_buffer: Arc::new(instances_buffer),
        }
    }

    pub fn len(&self) -> u32 {
        self.instances
    }

    pub fn is_empty(&self) -> bool {
        self.instances == 0
    }
}

#[derive(Debug)]
pub struct Renderer {
    // pipeline: wgpu::RenderPipeline,
    pipeline_list: wgpu::RenderPipeline,
    // layout: wgpu::PipelineLayout,
    layout_list: wgpu::PipelineLayout,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        textures_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let layout_list = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("2d-layout"),
            bind_group_layouts: &[camera_layout, &textures_bind_group_layout],
            push_constant_ranges: &[],
        });

        let vert = include_bytes!("../shaders/2d_list.vert.spv");
        let frag = include_bytes!("../shaders/2d_list.frag.spv");
        let vert = &vert[0..vert.len()];
        let frag = &frag[0..frag.len()];

        let vertex = wgpu::ShaderModuleDescriptor {
            flags: Default::default(),
            label: None,
            source: wgpu::ShaderSource::SpirV(vert.as_quad_bytes().into()),
        };
        let frag = wgpu::ShaderModuleDescriptor {
            flags: Default::default(),
            label: None,
            source: wgpu::ShaderSource::SpirV(frag.as_quad_bytes().into()),
        };

        let pipeline_list = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("2d-pipeline"),
            layout: Some(&layout_list),
            vertex: wgpu::VertexState {
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex2D>() as wgpu::BufferAddress,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            format: wgpu::VertexFormat::Float3,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            format: wgpu::VertexFormat::Uint,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            format: wgpu::VertexFormat::Float2,
                            shader_location: 2,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            format: wgpu::VertexFormat::Float4,
                            shader_location: 3,
                        },
                    ],
                    step_mode: wgpu::InputStepMode::Vertex,
                }],
                entry_point: "main",
                module: &device.create_shader_module(&vertex),
            },
            fragment: Some(wgpu::FragmentState {
                entry_point: "main",
                module: &device.create_shader_module(&frag),
                targets: &[wgpu::ColorTargetState {
                    format: super::output::WgpuOutput::OUTPUT_FORMAT,
                    alpha_blend: wgpu::BlendState::REPLACE,
                    color_blend: wgpu::BlendState {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    write_mask: wgpu::ColorWrite::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: wgpu::CullMode::None,
                polygon_mode: wgpu::PolygonMode::Fill,
                strip_index_format: None,
                topology: wgpu::PrimitiveTopology::TriangleList,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: super::output::WgpuOutput::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                bias: wgpu::DepthBiasState::default(),
                clamp_depth: false,
                stencil: wgpu::StencilState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        });

        Self {
            // pipeline,
            pipeline_list,
            // layout,
            layout_list,
        }
    }

    pub fn render_list(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera_bg: &wgpu::BindGroup,
        textures_bg: &wgpu::BindGroup,
        list: &VertexList<Vertex2D>,
        instances: &InstanceList<Mat4>,
        output: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[wgpu::RenderPassColorAttachmentDescriptor {
                attachment: output,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            }],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachmentDescriptor {
                attachment: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0_f32),
                    store: true,
                }),
                stencil_ops: None,
            }),
        });

        render_pass.set_pipeline(&self.pipeline_list);
        render_pass.set_vertex_buffer(0, list.get_vertex_buffer().buffer().slice(..));
        render_pass.set_bind_group(0, camera_bg, &[]);
        render_pass.set_bind_group(1, textures_bg, &[]);

        let v_ranges = list.get_ranges();
        let ranges = instances.get_ranges();

        for (i, r) in ranges.iter() {
            let v = v_ranges.get(i).unwrap();
            render_pass.draw(v.start..v.end, r.start..r.end);
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct InstanceDescriptor {
    aux: [u32; 4], // Additional data (currently only texture id & mesh id),
}

impl Default for InstanceDescriptor {
    fn default() -> Self {
        Self { aux: [0; 4] }
    }
}

#[allow(dead_code)]
impl InstanceDescriptor {
    pub fn tex_id(&self) -> Option<u32> {
        if self.aux[0] > 0 {
            Some(self.aux[0])
        } else {
            None
        }
    }

    pub fn mesh_id(&self) -> u32 {
        self.aux[1]
    }
}

#[derive(Debug)]
pub struct Mesh {
    pub tex_id: Option<usize>,
    pub bind_group: Option<Arc<wgpu::BindGroup>>,
}

impl Clone for Mesh {
    fn clone(&self) -> Self {
        Self {
            tex_id: self.tex_id,
            bind_group: self.bind_group.clone(),
        }
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            tex_id: None,
            bind_group: None,
        }
    }
}

#[allow(dead_code)]
impl Mesh {
    pub fn new(mesh: MeshData2D) -> Self {
        Self {
            tex_id: mesh.tex_id,
            bind_group: None,
        }
    }

    pub fn update_bind_group(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) {
        self.bind_group = Some(Arc::new(device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                label: None,
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(texture),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            },
        )));
    }
}
