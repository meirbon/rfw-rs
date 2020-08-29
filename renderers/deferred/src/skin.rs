use rfw_scene::graph::Skin;
use shared::BytesConversion;

#[derive(Debug)]
pub struct DeferredSkin {
    skin: Skin,
    matrices_buffer: Option<wgpu::Buffer>,
    joint_matrices_buffer_size: wgpu::BufferAddress,
    pub bind_group: Option<wgpu::BindGroup>,
}

impl Clone for DeferredSkin {
    fn clone(&self) -> Self {
        Self {
            skin: self.skin.clone(),
            matrices_buffer: None,
            joint_matrices_buffer_size: 0,
            bind_group: None,
        }
    }
}

impl Default for DeferredSkin {
    fn default() -> Self {
        Self {
            skin: Skin::default(),
            matrices_buffer: None,
            joint_matrices_buffer_size: 0,
            bind_group: None,
        }
    }
}

impl DeferredSkin {
    pub fn new(device: &wgpu::Device, skin: Skin) -> Self {
        let joint_matrices_buffer_size =
            skin.joint_matrices.as_bytes().len() as wgpu::BufferAddress;
        let matrices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("joint-matrices"),
            size: joint_matrices_buffer_size,
            usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            skin,
            matrices_buffer: Some(matrices_buffer),
            joint_matrices_buffer_size,
            bind_group: None,
        }
    }

    pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::StorageBuffer {
                    min_binding_size: None,
                    dynamic: false,
                    readonly: true,
                },
                count: None,
            }],
            label: Some("skin-bind-group-layout"),
        })
    }

    pub fn create_bind_group(&mut self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) {
        self.bind_group = Some(
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(
                        self.matrices_buffer
                            .as_ref()
                            .unwrap()
                            .slice(0..self.joint_matrices_buffer_size),
                    ),
                }],
                label: None,
            }),
        );
    }

    pub fn update(&self, queue: &wgpu::Queue) {
        if let Some(buffer) = self.matrices_buffer.as_ref() {
            let data = self.skin.joint_matrices.as_bytes();
            queue.write_buffer(buffer, 0, data);
        }
    }
}
