use rfw::prelude::*;
use std::sync::Arc;

#[derive(Debug)]
pub struct WgpuMesh {
    pub buffer: Option<Arc<wgpu::Buffer>>,
    pub buffer_size: wgpu::BufferAddress,
    pub joints_weights_buffer: Option<wgpu::Buffer>,
    pub ranges: Vec<VertexMesh>,
    pub bounds: AABB,
}

impl Default for WgpuMesh {
    fn default() -> Self {
        Self {
            buffer: None,
            buffer_size: 0,
            joints_weights_buffer: None,
            ranges: Default::default(),
            bounds: AABB::empty(),
        }
    }
}

impl Clone for WgpuMesh {
    fn clone(&self) -> Self {
        Self {
            buffer: None,
            buffer_size: 0,
            joints_weights_buffer: None,
            ranges: Default::default(),
            bounds: AABB::empty(),
        }
    }
}

#[allow(dead_code)]
impl WgpuMesh {
    pub fn new(
        device: &wgpu::Device,
        name: String,
        vertices: Vec<Vertex3D>,
        ranges: Vec<VertexMesh>,
        skin_data: Vec<JointData>,
        bounds: AABB,
    ) -> Self {
        let buffer_size = (vertices.len() * std::mem::size_of::<Vertex3D>()) as wgpu::BufferAddress;
        assert!(buffer_size > 0);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(name.as_str()),
            size: buffer_size,
            usage: if !skin_data.is_empty() {
                wgpu::BufferUsage::VERTEX
                    | wgpu::BufferUsage::COPY_SRC
                    | wgpu::BufferUsage::COPY_DST
            } else {
                wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST
            },
            mapped_at_creation: true,
        });

        buffer
            .slice(0..buffer_size)
            .get_mapped_range_mut()
            .copy_from_slice(vertices.as_bytes());
        buffer.unmap();

        let joints_weights_buffer = if !skin_data.is_empty() {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(name.as_str()),
                size: ((skin_data.len() + (64 - skin_data.len() % 64))
                    * std::mem::size_of::<JointData>())
                    as wgpu::BufferAddress,
                usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: true,
            });

            buffer
                .slice(0..(skin_data.as_bytes().len()) as wgpu::BufferAddress)
                .get_mapped_range_mut()
                .copy_from_slice(skin_data.as_bytes());
            buffer.unmap();
            Some(buffer)
        } else {
            None
        };

        Self {
            buffer: Some(Arc::new(buffer)),
            buffer_size,
            joints_weights_buffer,
            ranges,
            bounds,
        }
    }

    pub fn len(&self) -> usize {
        self.buffer_size as usize / std::mem::size_of::<Vertex3D>()
    }
}

pub struct SkinningPipeline {
    pipeline: wgpu::ComputePipeline,
    _pipeline_layout: wgpu::PipelineLayout,
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
            _pipeline_layout: pipeline_layout,
            bind_group_layout,
        }
    }

    pub fn apply_skin(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &WgpuMesh,
        skin: &WgpuSkin,
    ) -> (wgpu::Buffer, wgpu::BufferAddress) {
        let len = mesh.len() + (64 - mesh.len() % 64);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skinned-vertices"),
            size: (len * std::mem::size_of::<Vertex3D>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::VERTEX
                | wgpu::BufferUsage::STORAGE
                | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(
            mesh.buffer.as_ref().unwrap(),
            0,
            &buffer,
            0,
            (mesh.len() * std::mem::size_of::<Vertex3D>()) as wgpu::BufferAddress,
        );

        if mesh.buffer.is_some() && mesh.joints_weights_buffer.is_some() && skin.buffer.is_some() {
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
                        resource: wgpu::BindingResource::Buffer(
                            skin.buffer.as_ref().unwrap().slice(..),
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(
                            mesh.joints_weights_buffer.as_ref().unwrap().slice(
                                0..(len * std::mem::size_of::<JointData>()) as wgpu::BufferAddress,
                            ),
                        ),
                    },
                ],
            });

            let mut compute_pass = encoder.begin_compute_pass();
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch(len as u32 / 64, 1, 1);
        }

        queue.submit(std::iter::once(encoder.finish()));

        (buffer, len as wgpu::BufferAddress)
    }

    pub fn apply_skin_buffer(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer: &mut (wgpu::Buffer, wgpu::BufferAddress),
        mesh: &WgpuMesh,
        skin: &WgpuSkin,
    ) {
        let len = mesh.len() + (64 - mesh.len() % 64);
        if (buffer.1 as usize) < len {
            // Recreate buffer if it is not large enough
            let b = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("skinned-vertices"),
                size: (len * std::mem::size_of::<Vertex3D>()) as wgpu::BufferAddress,
                usage: wgpu::BufferUsage::VERTEX
                    | wgpu::BufferUsage::STORAGE
                    | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: false,
            });
            buffer.0 = b;
            buffer.1 = len as wgpu::BufferAddress;
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("apply-skin"),
        });
        encoder.copy_buffer_to_buffer(
            mesh.buffer.as_ref().unwrap(),
            0,
            &buffer.0,
            0,
            (mesh.len() * std::mem::size_of::<Vertex3D>()) as wgpu::BufferAddress,
        );

        assert!(mesh.buffer.is_some());
        assert!(mesh.joints_weights_buffer.is_some());
        assert!(skin.buffer.is_some());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skinning-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(buffer.0.slice(..)),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(
                        skin.buffer.as_ref().unwrap().slice(..),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        mesh.joints_weights_buffer.as_ref().unwrap().slice(..),
                    ),
                },
            ],
        });

        let mut compute_pass = encoder.begin_compute_pass();
        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch(len as u32 / 64, 1, 1);
        drop(compute_pass);

        queue.submit(std::iter::once(encoder.finish()));
    }
}

#[derive(Debug)]
pub struct WgpuSkin {
    pub joint_matrices: Vec<Mat4>,
    pub buffer: Option<wgpu::Buffer>,
    pub buffer_size: wgpu::BufferAddress,
}

impl Clone for WgpuSkin {
    fn clone(&self) -> Self {
        Self {
            joint_matrices: self.joint_matrices.clone(),
            buffer: None,
            buffer_size: 0,
        }
    }
}

impl Default for WgpuSkin {
    fn default() -> Self {
        Self {
            joint_matrices: Default::default(),
            buffer: None,
            buffer_size: 0,
        }
    }
}

impl WgpuSkin {
    pub fn new(device: &wgpu::Device, skin: SkinData) -> Self {
        // Make sure number of matrices does not exceed shader maximum
        assert!(skin.joint_matrices.len() < 1024);

        let size = (std::mem::size_of::<Mat4>() * skin.joint_matrices.len()) as wgpu::BufferAddress;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skin-matrices"),
            size,
            usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: true,
        });

        buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(skin.joint_matrices.as_bytes());
        buffer.unmap();

        Self {
            joint_matrices: skin.joint_matrices.to_vec(),
            buffer: Some(buffer),
            buffer_size: size,
        }
    }

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, skin: SkinData) {
        // Make sure number of matrices does not exceed shader maximum
        assert!(skin.joint_matrices.len() < 1024);
        self.joint_matrices = skin.joint_matrices.to_vec();

        let size = (std::mem::size_of::<Mat4>() * self.joint_matrices.len()) as wgpu::BufferAddress;
        if size > self.buffer_size {
            self.buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("skin-matrices"),
                size,
                usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: true,
            }));

            self.buffer
                .as_ref()
                .unwrap()
                .slice(0..size)
                .get_mapped_range_mut()
                .copy_from_slice(self.joint_matrices.as_bytes());
            self.buffer.as_ref().unwrap().unmap();
        } else {
            queue.write_buffer(
                self.buffer.as_ref().unwrap(),
                0,
                self.joint_matrices.as_bytes(),
            );
        }
    }
}
