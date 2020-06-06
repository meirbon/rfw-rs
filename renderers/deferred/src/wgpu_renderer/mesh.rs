use super::CopyCommand;
use rtbvh::AABB;
use scene::{Mesh, VertexData, VertexMesh};

pub struct VertexBuffer {
    pub count: usize,
    pub size_in_bytes: usize,
    pub buffer: wgpu::Buffer,
    pub bounds: AABB,
    pub meshes: Vec<VertexMesh>,
}

pub struct DeferredMesh {
    pub sub_meshes: Vec<VertexMesh>,
    pub vertex_data: Vec<VertexData>,
    pub buffer: wgpu::Buffer,
    pub buffer_size: wgpu::BufferAddress,
}

impl DeferredMesh {
    pub fn new(device: &wgpu::Device, mesh: &Mesh) -> Self {
        let buffer_size = mesh.buffer_size() as wgpu::BufferAddress;
        assert!(buffer_size > 0);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(mesh.name.as_str()),
            size: buffer_size,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });

        let sub_meshes = mesh.meshes.clone();
        let vertex_data = mesh.vertices.clone();

        Self {
            sub_meshes,
            vertex_data,
            buffer,
            buffer_size,
        }
    }

    pub fn len(&self) -> usize {
        self.vertex_data.len()
    }

    pub fn get_copy_command(&self, device: &wgpu::Device) -> CopyCommand {
        let data = unsafe {
            std::slice::from_raw_parts(
                self.vertex_data.as_ptr() as *const u8,
                self.vertex_data.len() * std::mem::size_of::<VertexData>(),
            )
        };

        let staging_buffer = device.create_buffer_with_data(data, wgpu::BufferUsage::COPY_SRC);

        CopyCommand {
            destination_buffer: &self.buffer,
            copy_size: self.buffer_size as wgpu::BufferAddress,
            staging_buffer,
        }
    }
}
