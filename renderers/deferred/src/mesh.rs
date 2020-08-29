use rfw_scene::{AnimVertexData, AnimatedMesh, Mesh, VertexData, VertexMesh};
use shared::BytesConversion;

#[derive(Debug)]
pub struct DeferredAnimMesh {
    pub sub_meshes: Vec<VertexMesh>,
    pub vertex_data: Vec<VertexData>,
    pub anim_vertex_data: Vec<AnimVertexData>,
    pub buffer: Option<wgpu::Buffer>,
    pub buffer_start: wgpu::BufferAddress,
    pub buffer_end: wgpu::BufferAddress,
    pub anim_start: wgpu::BufferAddress,
    pub anim_end: wgpu::BufferAddress,
}

impl Clone for DeferredAnimMesh {
    fn clone(&self) -> Self {
        Self {
            sub_meshes: self.sub_meshes.clone(),
            vertex_data: self.vertex_data.clone(),
            anim_vertex_data: self.anim_vertex_data.clone(),
            buffer: None,
            buffer_start: 0,
            buffer_end: 0,
            anim_start: 0,
            anim_end: 0,
        }
    }
}

impl Default for DeferredAnimMesh {
    fn default() -> Self {
        Self {
            sub_meshes: Vec::new(),
            vertex_data: Vec::new(),
            anim_vertex_data: Vec::new(),
            buffer: None,
            buffer_start: 0,
            buffer_end: 0,
            anim_start: 0,
            anim_end: 0,
        }
    }
}

#[derive(Debug)]
pub struct DeferredMesh {
    pub sub_meshes: Vec<VertexMesh>,
    pub vertex_data: Vec<VertexData>,
    pub buffer: Option<wgpu::Buffer>,
    pub buffer_size: wgpu::BufferAddress,
}

impl Default for DeferredMesh {
    fn default() -> Self {
        Self {
            sub_meshes: Vec::new(),
            vertex_data: Vec::new(),
            buffer: None,
            buffer_size: 0,
        }
    }
}

impl Clone for DeferredMesh {
    fn clone(&self) -> Self {
        Self {
            sub_meshes: self.sub_meshes.clone(),
            vertex_data: self.vertex_data.clone(),
            buffer: None,
            buffer_size: 0,
        }
    }
}

#[allow(dead_code)]
impl DeferredMesh {
    pub fn new(device: &wgpu::Device, mesh: &Mesh) -> Self {
        let buffer_size = mesh.buffer_size() as wgpu::BufferAddress;
        assert!(buffer_size > 0);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(mesh.name.as_str()),
            size: buffer_size,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let sub_meshes = mesh.meshes.clone();
        let vertex_data = mesh.vertices.clone();

        Self {
            sub_meshes,
            vertex_data,
            buffer: Some(buffer),
            buffer_size,
        }
    }

    pub fn len(&self) -> usize {
        self.vertex_data.len()
    }

    pub fn copy_data(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            self.buffer.as_ref().unwrap(),
            0,
            self.vertex_data.as_bytes(),
        );
    }
}

#[allow(dead_code)]
impl DeferredAnimMesh {
    pub fn new(device: &wgpu::Device, mesh: &AnimatedMesh) -> Self {
        let buffer_size = (mesh.vertices.as_bytes().len() + mesh.anim_vertex_data.as_bytes().len())
            as wgpu::BufferAddress;
        assert!(buffer_size > 0);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(mesh.name.as_str()),
            size: buffer_size,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let sub_meshes = mesh.meshes.clone();
        let vertex_data = mesh.vertices.clone();
        let anim_vertex_data = mesh.anim_vertex_data.clone();

        Self {
            sub_meshes,
            vertex_data,
            anim_vertex_data,
            buffer: Some(buffer),
            buffer_start: 0,
            buffer_end: mesh.vertices.as_bytes().len() as wgpu::BufferAddress,
            anim_start: mesh.vertices.as_bytes().len() as wgpu::BufferAddress,
            anim_end: mesh.vertices.as_bytes().len() as wgpu::BufferAddress
                + mesh.anim_vertex_data.as_bytes().len() as wgpu::BufferAddress,
        }
    }

    pub fn len(&self) -> usize {
        self.vertex_data.len()
    }

    pub fn copy_data(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            self.buffer.as_ref().unwrap(),
            0,
            self.vertex_data.as_bytes(),
        );

        queue.write_buffer(
            self.buffer.as_ref().unwrap(),
            self.anim_start,
            self.anim_vertex_data.as_bytes(),
        );
    }
}
