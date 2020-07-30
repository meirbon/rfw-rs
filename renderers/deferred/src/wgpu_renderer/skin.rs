use scene::graph::Skin;
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
            skin.joint_matrices.to_bytes().len() as wgpu::BufferAddress;
        let matrices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("joint-matrices"),
            size: joint_matrices_buffer_size,
            usage: wgpu::BufferUsage::STORAGE_READ | wgpu::BufferUsage::MAP_WRITE,
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
            bindings: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::StorageBuffer {
                    dynamic: false,
                    readonly: true,
                },
            }],
            label: Some("skin-bind-group-layout"),
        })
    }

    pub fn create_bind_group(&mut self, device: &wgpu::Device, layout: &wgpu::BindGroupLayout) {
        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: self.matrices_buffer.as_ref().unwrap(),
                    range: 0..self.joint_matrices_buffer_size,
                },
            }],
            label: None,
        }));
    }

    pub async fn update(&self, device: &wgpu::Device) {
        if let Some(buffer) = self.matrices_buffer.as_ref() {
            let mapping = buffer.map_write(0, self.joint_matrices_buffer_size);
            device.poll(wgpu::Maintain::Wait);
            let mut mapping = mapping.await.unwrap();

            mapping
                .as_slice()
                .copy_from_slice(self.skin.joint_matrices.to_bytes());
        }
    }
}
