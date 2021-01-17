use crate::WgpuTexture;
use rfw::prelude::*;
use std::sync::Arc;

#[derive(Debug)]
pub struct InstanceList2D {
    instance_capacity: usize,
    instances: u32,
    pub instances_buffer: Arc<Option<wgpu::Buffer>>,
    pub instances_bg: Arc<Option<wgpu::BindGroup>>,
}

impl Default for InstanceList2D {
    fn default() -> Self {
        Self {
            instance_capacity: 0,
            instances: 0,
            instances_buffer: Arc::new(None),
            instances_bg: Arc::new(None),
        }
    }
}

impl Clone for InstanceList2D {
    fn clone(&self) -> Self {
        Self {
            instance_capacity: self.instance_capacity,
            instances: self.instances,
            instances_buffer: self.instances_buffer.clone(),
            instances_bg: self.instances_bg.clone(),
        }
    }
}

#[allow(dead_code)]
impl InstanceList2D {
    const DEFAULT_CAPACITY: usize = 4;

    pub fn new(device: &wgpu::Device, instances_layout: &wgpu::BindGroupLayout) -> Self {
        let instances_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (Self::DEFAULT_CAPACITY * std::mem::size_of::<Mat4>()) as _,
            usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        }));

        let instances_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: instances_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(
                    instances_buffer.as_ref().unwrap().slice(..),
                ),
            }],
        }));

        Self {
            instance_capacity: Self::DEFAULT_CAPACITY as _,
            instances: 0,
            instances_buffer: Arc::new(instances_buffer),
            instances_bg: Arc::new(instances_bg),
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: InstancesData2D<'_>,
        instances_layout: &wgpu::BindGroupLayout,
    ) {
        self.instances = instances.len() as _;
        if instances.len() > self.instance_capacity || self.instances_buffer.is_none() {
            self.instance_capacity = instances.len().next_power_of_two();
            let instances_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (self.instance_capacity * std::mem::size_of::<Mat4>()) as _,
                usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: true,
            });

            instances_buffer
                .slice(..)
                .get_mapped_range_mut()
                .copy_from_slice(instances.matrices.as_bytes());
            instances_buffer.unmap();
            self.instances_bg =
                Arc::new(Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: instances_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(instances_buffer.slice(..)),
                    }],
                })));

            self.instances_buffer = Arc::new(Some(instances_buffer));
        } else {
            queue.write_buffer(
                (*self.instances_buffer).as_ref().unwrap(),
                0,
                instances.matrices.as_bytes(),
            );
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
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::PipelineLayout,
    bind_group_layout: wgpu::BindGroupLayout,
    meshes: TrackedStorage<Mesh>,
    instances: FlaggedStorage<InstanceList2D>,
    matrices_buffer: wgpu::Buffer,
    matrices_buffer_size: wgpu::BufferAddress,
    sampler: wgpu::Sampler,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        camera_layout: &wgpu::BindGroupLayout,
        instance_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("2d-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::SampledTexture {
                        component_type: wgpu::TextureComponentType::Uint,
                        dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Sampler { comparison: false },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("2d-layout"),
            bind_group_layouts: &[camera_layout, instance_layout, &bind_group_layout],
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
                format: super::output::WgpuOutput::OUTPUT_FORMAT,
                alpha_blend: wgpu::BlendDescriptor::REPLACE,
                color_blend: wgpu::BlendDescriptor {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                write_mask: wgpu::ColorWrite::ALL,
            }],
            depth_stencil_state: Some(wgpu::DepthStencilStateDescriptor {
                format: super::output::WgpuOutput::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilStateDescriptor::default(),
            }),
            vertex_state: wgpu::VertexStateDescriptor {
                index_format: wgpu::IndexFormat::Uint32,
                vertex_buffers: &[wgpu::VertexBufferDescriptor {
                    stride: std::mem::size_of::<Vertex2D>() as wgpu::BufferAddress,
                    step_mode: wgpu::InputStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttributeDescriptor {
                            offset: 0,
                            format: wgpu::VertexFormat::Float3,
                            shader_location: 0,
                        },
                        wgpu::VertexAttributeDescriptor {
                            offset: 12,
                            format: wgpu::VertexFormat::Uint,
                            shader_location: 1,
                        },
                        wgpu::VertexAttributeDescriptor {
                            offset: 16,
                            format: wgpu::VertexFormat::Float2,
                            shader_location: 2,
                        },
                        wgpu::VertexAttributeDescriptor {
                            offset: 24,
                            format: wgpu::VertexFormat::Float4,
                            shader_location: 3,
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
            meshes: Default::default(),
            instances: Default::default(),
            matrices_buffer,
            matrices_buffer_size,
            sampler,
        }
    }

    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera_bg: &wgpu::BindGroup,
        output: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        if self.meshes.is_empty() {
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
        for (i, mesh) in self.meshes.iter() {
            let (buffer, bg, instance_bg) = match (
                mesh.buffer.as_ref(),
                mesh.bind_group.as_ref(),
                self.instances[i].instances_bg.as_ref(),
            ) {
                (Some(a), Some(b), Some(c)) => (a, b, c),
                _ => continue,
            };

            render_pass.set_vertex_buffer(0, buffer.slice(..));
            render_pass.set_bind_group(0, camera_bg, &[]);
            render_pass.set_bind_group(1, instance_bg, &[]);
            render_pass.set_bind_group(2, bg, &[]);
            render_pass.draw(0..mesh.vertex_count, 0..self.instances[i].instances);
        }
    }

    pub fn set_instances(
        &mut self,
        id: usize,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: InstancesData2D<'_>,
        instances_layout: &wgpu::BindGroupLayout,
    ) {
        let instances = if let Some(instances) = self.instances.get_mut(id) {
            instances
        } else {
            self.instances
                .overwrite_val(id, InstanceList2D::new(device, instances_layout));
            self.instances.get_mut(id).unwrap()
        };

        instances.update(device, queue, data, instances_layout)
    }

    pub fn update_bind_groups(
        &mut self,
        device: &wgpu::Device,
        textures: &[WgpuTexture],
        changed: &BitSlice,
    ) {
        let bind_group_layout = &self.bind_group_layout;
        let sampler = &self.sampler;

        for (_, m) in self.meshes.iter_mut() {
            let texture = if let Some(id) = m.tex_id {
                if !changed[id] && m.bind_group.is_some() {
                    continue;
                }
                textures[id].view.as_ref().as_ref().unwrap()
            } else {
                textures[0].view.as_ref().as_ref().unwrap()
            };

            m.update_bind_group(device, bind_group_layout, texture, sampler);
        }
    }

    pub fn set_mesh(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: usize,
        mesh: MeshData2D,
    ) {
        if let Some(m) = self.meshes.get_mut(id) {
            m.update(device, queue, mesh);
        } else {
            self.meshes.overwrite(id, Mesh::new(device, mesh));
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
    pub buffer: Option<Arc<wgpu::Buffer>>,
    pub buffer_size: wgpu::BufferAddress,
    pub vertex_count: u32,
    pub tex_id: Option<usize>,
    pub instances: u32,
    pub bind_group: Option<Arc<wgpu::BindGroup>>,
}

impl Clone for Mesh {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            buffer_size: self.buffer_size,
            vertex_count: self.vertex_count,
            tex_id: self.tex_id,
            instances: self.instances,
            bind_group: self.bind_group.clone(),
        }
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            buffer: None,
            buffer_size: 0,
            vertex_count: 0,
            tex_id: None,
            instances: 0,
            bind_group: None,
        }
    }
}

#[allow(dead_code)]
impl Mesh {
    pub fn new(device: &wgpu::Device, mesh: MeshData2D) -> Self {
        let bytes = mesh.vertices.as_bytes();
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.len() as _,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: true,
        });
        buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(bytes);
        buffer.unmap();

        let buffer_size = bytes.len() as wgpu::BufferAddress;
        let vertex_count = mesh.vertices.len() as u32;

        Self {
            buffer: Some(Arc::new(buffer)),
            buffer_size,
            vertex_count,
            tex_id: mesh.tex_id,
            instances: 0,
            bind_group: None,
        }
    }

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, mesh: MeshData2D) {
        let bytes = mesh.vertices.as_bytes();
        if self.buffer.is_none() || self.buffer_size < (bytes.len() as wgpu::BufferAddress) {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: bytes.len() as _,
                usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: true,
            });

            buffer
                .slice(..)
                .get_mapped_range_mut()
                .copy_from_slice(bytes);
            buffer.unmap();

            self.buffer = Some(Arc::new(buffer));
        } else {
            queue.write_buffer(self.buffer.as_ref().unwrap(), 0, bytes);
        }

        self.buffer_size = bytes.len() as wgpu::BufferAddress;
        self.vertex_count = mesh.vertices.len() as u32;
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

    pub fn free(&mut self) {
        self.buffer = None;
        self.buffer_size = 0;
        self.tex_id = None;
        self.vertex_count = 0;
    }
}
