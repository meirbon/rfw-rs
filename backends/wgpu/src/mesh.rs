use rfw::prelude::*;
use std::sync::Arc;

#[derive(Debug)]
pub struct WgpuMesh {
    pub buffer: Option<Arc<wgpu::Buffer>>,
    pub buffer_size: wgpu::BufferAddress,
    pub desc: Mesh3D,
}

impl Default for WgpuMesh {
    fn default() -> Self {
        Self {
            buffer: None,
            buffer_size: 0,
            desc: Default::default(),
        }
    }
}

impl Clone for WgpuMesh {
    fn clone(&self) -> Self {
        Self {
            buffer: None,
            buffer_size: 0,
            desc: self.desc.clone(),
        }
    }
}

#[allow(dead_code)]
impl WgpuMesh {
    pub fn new(device: &wgpu::Device, mesh: &Mesh3D) -> Self {
        let buffer_size = mesh.buffer_size() as wgpu::BufferAddress;
        assert!(buffer_size > 0);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(mesh.name.as_str()),
            size: buffer_size,
            usage: wgpu::BufferUsage::VERTEX
                | wgpu::BufferUsage::COPY_SRC
                | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            buffer: Some(Arc::new(buffer)),
            buffer_size,
            desc: mesh.clone(),
        }
    }

    pub fn len(&self) -> usize {
        self.desc.vertices.len()
    }

    pub fn copy_data(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            self.buffer.as_ref().unwrap(),
            0,
            self.desc.vertices.as_bytes(),
        );
    }
}

pub struct SkinningPipeline {
    pipeline: wgpu::ComputePipeline,
    pipeline_layout: wgpu::PipelineLayout,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl SkinningPipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = include_bytes!("../shaders/skinning.comp.spv");
        let module = device.create_shader_module(wgpu::util::make_spirv(&shader[..]));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skinning-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::StorageBuffer {
                        dynamic: false,
                        min_binding_size: None,
                        readonly: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStage::COMPUTE,
                    ty: wgpu::BindingType::StorageBuffer {
                        dynamic: false,
                        min_binding_size: None,
                        readonly: true,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("skinning-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("skinning-pipeline"),
            layout: Some(&pipeline_layout),
            compute_stage: wgpu::ProgrammableStageDescriptor {
                entry_point: "main",
                module: &module,
            },
        });

        Self {
            pipeline,
            pipeline_layout,
            bind_group_layout,
        }
    }

    pub fn apply_skin(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &WgpuMesh,
        skin: &Skin,
    ) -> wgpu::Buffer {
        let len = mesh.desc.vertices.len() + (64 - mesh.desc.vertices.len() % 64);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skinned-vertices"),
            size: (len * std::mem::size_of::<VertexData>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::VERTEX
                | wgpu::BufferUsage::STORAGE
                | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });
        if true || mesh.buffer.is_none() {
            // CPU-based skinning
            let skinned = mesh.desc.apply_skin(skin);
            queue.write_buffer(&buffer, 0, skinned.vertices.as_bytes());
        } else {
            // GPU-based skinning
            let skin_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("skin-matrices"),
                size: (std::mem::size_of::<Mat4>() * skin.joint_matrices.len())
                    as wgpu::BufferAddress,
                usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: false,
            });

            queue.write_buffer(&skin_buffer, 0, skin.joint_matrices.as_bytes());

            let joints_weights_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("joints-weights"),
                size: (len * std::mem::size_of::<JointData>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: false,
            });

            assert_eq!(std::mem::size_of::<JointData>(), 32);

            queue.write_buffer(
                &joints_weights_buffer,
                0,
                mesh.desc.joints_weights.as_bytes(),
            );

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("skinning-bind-group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(buffer.slice(..)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(skin_buffer.slice(..)),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(joints_weights_buffer.slice(..)),
                    },
                ],
            });

            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            encoder.copy_buffer_to_buffer(
                mesh.buffer.as_ref().unwrap(),
                0,
                &buffer,
                0,
                (mesh.desc.vertices.len() * std::mem::size_of::<VertexData>())
                    as wgpu::BufferAddress,
            );
            {
                let mut compute_pass = encoder.begin_compute_pass();
                compute_pass.set_pipeline(&self.pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);
                compute_pass.dispatch(len as u32 / 64, 1, 1);
            }

            queue.submit(std::iter::once(encoder.finish()));
        }
        buffer
    }
}
