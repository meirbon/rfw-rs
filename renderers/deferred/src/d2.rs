use glam::*;
use rfw_scene::r2d::{D2Instance, D2Mesh, D2Vertex};
use rfw_scene::{ChangedIterator, TrackedStorage};
use shared::BytesConversion;
use std::sync::Arc;
use wgpu::util::DeviceExt;

#[derive(Debug)]
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::PipelineLayout,
    bind_group_layout: wgpu::BindGroupLayout,
    descriptors: Vec<InstanceDescriptor>,
    meshes: TrackedStorage<Mesh>,
    matrices_buffer: wgpu::Buffer,
    matrices_buffer_size: wgpu::BufferAddress,
    bind_groups: Vec<wgpu::BindGroup>,
    sampler: wgpu::Sampler,
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("2d-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::StorageBuffer {
                        dynamic: false,
                        min_binding_size: None,
                        readonly: true,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Uint,
                        dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("2d-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex = wgpu::include_spirv!("../shaders/2d.vert.spv");
        let frag = wgpu::include_spirv!("../shaders/2d.frag.spv");

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("2d-pipeline"),
            layout: Some(&layout),
            vertex_stage: wgpu::ProgrammableStageDescriptor {
                module: &device.create_shader_module(vertex),
                entry_point: "main",
            },
            fragment_stage: Some(wgpu::ProgrammableStageDescriptor {
                module: &device.create_shader_module(frag),
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
                format: super::output::DeferredOutput::OUTPUT_FORMAT,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                format: super::output::DeferredOutput::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilStateDescriptor::default(),
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<D2Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttributeDescriptor {
                            offset: 0,
                            format: wgpu::VertexFormat::Float4,
                            shader_location: 0,
                        },
                        wgpu::VertexAttributeDescriptor {
                            offset: 16,
                            format: wgpu::VertexFormat::Float2,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            sample_count: 1,
            sample_mask: !0,
            alpha_to_coverage_enabled: false,
        });

        let matrices_buffer_size =
            512 * std::mem::size_of::<InstanceDescriptor>() as wgpu::BufferAddress;
        let matrices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("2d-instances-buffer"),
            size: matrices_buffer_size,
            usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 5.0,
            compare: None,
            anisotropy_clamp: None,
        });

        Self {
            pipeline,
            layout,
            bind_group_layout,
            descriptors: Default::default(),
            meshes: Default::default(),
            matrices_buffer,
            matrices_buffer_size,
            bind_groups: vec![],
            sampler,
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        if self.meshes.is_empty() || self.descriptors.is_empty() {
            return;
        }

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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

        render_pass.set_pipeline(&self.pipeline);
        for (i, inst) in self.descriptors.iter().enumerate() {
            let mesh_id = inst.mesh_id() as usize;

            if let Some(mesh) = self.meshes.get(mesh_id) {
                if let Some(buffer) = mesh.buffer.as_ref() {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));

                    if let Some(bind_group) =
                        self.bind_groups.get(mesh.tex_id.unwrap_or(0) as usize)
                    {
                        render_pass.set_bind_group(0, bind_group, &[]);
                    } else {
                        render_pass.set_bind_group(0, &self.bind_groups[0], &[]);
                    }
                    let i = i as u32;
                    render_pass.draw(0..mesh.vertex_count, i..(i + 1));
                }
            }
        }
    }

    pub fn update_bind_groups(
        &mut self,
        device: &wgpu::Device,
        texture_views: &[wgpu::TextureView],
    ) {
        self.bind_groups = texture_views
            .iter()
            .map(|v| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(self.matrices_buffer.slice(..)),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(v),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                })
            })
            .collect();
    }

    pub fn update_meshes(&mut self, device: &wgpu::Device, meshes: ChangedIterator<'_, D2Mesh>) {
        for (i, m) in meshes {
            self.meshes.overwrite(i, Mesh::new(device, m));
        }
    }

    pub fn update_instances(
        &mut self,
        queue: &wgpu::Queue,
        instances: ChangedIterator<'_, D2Instance>,
    ) {
        let mut instances: Vec<InstanceDescriptor> = instances
            .as_slice()
            .iter()
            .map(|i| {
                if let Some(mesh_id) = i.mesh {
                    if let Some(mesh) = self.meshes.get(mesh_id as usize) {
                        InstanceDescriptor {
                            matrix: Mat4::from_cols_array(&i.transform),
                            color: mesh.color,
                            aux: [mesh.tex_id.unwrap_or(0), mesh_id, 0, 0],
                        }
                    } else {
                        InstanceDescriptor::default()
                    }
                } else {
                    InstanceDescriptor::default()
                }
            })
            .collect();

        self.descriptors = instances;
        queue.write_buffer(&self.matrices_buffer, 0, self.descriptors.as_bytes());
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct InstanceDescriptor {
    matrix: Mat4,
    color: [f32; 4],
    aux: [u32; 4], // Additional data (currently only texture id & mesh id),
}

impl Default for InstanceDescriptor {
    fn default() -> Self {
        Self {
            matrix: Mat4::identity(),
            color: [0.0; 4],
            aux: [0; 4],
        }
    }
}

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
    pub buffer: Option<Arc<wgpu::Buffer>>,
    pub buffer_size: wgpu::BufferAddress,
    pub color: [f32; 4],
    pub vertex_count: u32,
    pub tex_id: Option<u32>,
}

impl Clone for Mesh {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            buffer_size: self.buffer_size,
            color: self.color,
            vertex_count: self.vertex_count,
            tex_id: self.tex_id,
        }
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            buffer: None,
            buffer_size: 0,
            color: [0.0; 4],
            vertex_count: 0,
            tex_id: None,
        }
    }
}

impl Mesh {
    pub fn new(device: &wgpu::Device, mesh: &D2Mesh) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("2d-mesh"),
            contents: mesh.vertices.as_bytes(),
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });

        let buffer_size = mesh.vertices.as_bytes().len() as wgpu::BufferAddress;
        let vertex_count = mesh.vertices.len() as u32;

        Self {
            buffer: Some(Arc::new(buffer)),
            buffer_size,
            color: mesh.color,
            vertex_count,
            tex_id: mesh.tex_id,
        }
    }

    pub fn free(&mut self) {
        self.buffer = None;
        self.buffer_size = 0;
        self.tex_id = None;
        self.vertex_count = 0;
    }
}
