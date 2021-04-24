use rfw::prelude::*;
use std::sync::Arc;

#[derive(Debug)]
pub struct WgpuSkin {
    pub bind_group: Option<Arc<wgpu::BindGroup>>,
    pub joint_matrices: Vec<Mat4>,
    pub buffer: Option<Arc<wgpu::Buffer>>,
    pub buffer_size: wgpu::BufferAddress,
}

impl Clone for WgpuSkin {
    fn clone(&self) -> Self {
        Self {
            bind_group: self.bind_group.clone(),
            joint_matrices: self.joint_matrices.clone(),
            buffer: self.buffer.clone(),
            buffer_size: 0,
        }
    }
}

impl Default for WgpuSkin {
    fn default() -> Self {
        Self {
            bind_group: None,
            joint_matrices: Default::default(),
            buffer: None,
            buffer_size: 0,
        }
    }
}

impl WgpuSkin {
    pub fn create_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skin-bg-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX | wgpu::ShaderStage::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        })
    }

    pub fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, skin: SkinData) -> Self {
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skinning-bg"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &buffer,
                    offset: 0,
                    size: None,
                },
            }],
        });

        Self {
            bind_group: Some(Arc::new(bind_group)),
            joint_matrices: skin.joint_matrices.to_vec(),
            buffer: Some(Arc::new(buffer)),
            buffer_size: size,
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        skin: SkinData,
    ) {
        // Make sure number of matrices does not exceed shader maximum
        assert!(skin.joint_matrices.len() < 1024);
        self.joint_matrices = skin.joint_matrices.to_vec();

        let size = (std::mem::size_of::<Mat4>() * self.joint_matrices.len()) as wgpu::BufferAddress;
        if size > self.buffer_size {
            self.buffer = Some(Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("skin-matrices"),
                size,
                usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: true,
            })));

            self.buffer
                .as_ref()
                .unwrap()
                .slice(0..size)
                .get_mapped_range_mut()
                .copy_from_slice(self.joint_matrices.as_bytes());
            self.buffer.as_ref().unwrap().unmap();

            self.bind_group = Some(Arc::new(device.create_bind_group(
                &wgpu::BindGroupDescriptor {
                    label: Some("skinning-bg"),
                    layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer {
                            buffer: &self.buffer.as_ref().unwrap(),
                            offset: 0,
                            size: None,
                        },
                    }],
                },
            )));
        } else {
            queue.write_buffer(
                self.buffer.as_ref().unwrap(),
                0,
                self.joint_matrices.as_bytes(),
            );
        }
    }
}
