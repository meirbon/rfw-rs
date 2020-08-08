use rfw_scene::{AnimVertexData, AnimatedMesh, Mesh, VertexData, VertexMesh};
use shared::BytesConversion;

#[derive(Debug)]
pub struct DeferredAnimMesh {
    pub sub_meshes: Vec<VertexMesh>,
    pub vertex_data: Vec<VertexData>,
    pub anim_vertex_data: Vec<AnimVertexData>,
    pub buffer: Option<wgpu::Buffer>,
    pub buffer_size: wgpu::BufferAddress,
    pub anim_buffer: Option<wgpu::Buffer>,
    pub anim_buffer_size: wgpu::BufferAddress,
}

impl Clone for DeferredAnimMesh {
    fn clone(&self) -> Self {
        Self {
            sub_meshes: self.sub_meshes.clone(),
            vertex_data: self.vertex_data.clone(),
            anim_vertex_data: self.anim_vertex_data.clone(),
            buffer: None,
            buffer_size: 0,
            anim_buffer: None,
            anim_buffer_size: 0,
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
            buffer_size: 0,
            anim_buffer: None,
            anim_buffer_size: 0,
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

    pub async fn copy_data(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mesh-update"),
        });

        let data = unsafe {
            std::slice::from_raw_parts(
                self.vertex_data.as_ptr() as *const u8,
                self.vertex_data.len() * std::mem::size_of::<VertexData>(),
            )
        };

        let staging_buffer = device.create_buffer_with_data(data, wgpu::BufferUsage::COPY_SRC);
        encoder.copy_buffer_to_buffer(
            &staging_buffer,
            0,
            self.buffer.as_ref().unwrap(),
            0,
            self.buffer_size as wgpu::BufferAddress,
        );
        queue.submit(&[encoder.finish()]);
    }
}

#[allow(dead_code)]
impl DeferredAnimMesh {
    pub fn new(device: &wgpu::Device, mesh: &AnimatedMesh) -> Self {
        let buffer_size = mesh.vertices.as_bytes().len() as wgpu::BufferAddress;
        assert!(buffer_size > 0);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(mesh.name.as_str()),
            size: buffer_size,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });

        let anim_buffer_size = mesh.anim_vertex_data.as_bytes().len() as wgpu::BufferAddress;
        assert!(anim_buffer_size > 0);
        let anim_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(mesh.name.as_str()),
            size: anim_buffer_size,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });

        let sub_meshes = mesh.meshes.clone();
        let vertex_data = mesh.vertices.clone();
        let anim_vertex_data = mesh.anim_vertex_data.clone();

        Self {
            sub_meshes,
            vertex_data,
            anim_vertex_data,
            buffer: Some(buffer),
            buffer_size,
            anim_buffer: Some(anim_buffer),
            anim_buffer_size,
        }
    }

    pub fn len(&self) -> usize {
        self.vertex_data.len()
    }

    pub async fn copy_data(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("anim-mesh-copy"),
        });
        let staging_buffer1 = device
            .create_buffer_with_data(self.vertex_data.as_bytes(), wgpu::BufferUsage::COPY_SRC);

        encoder.copy_buffer_to_buffer(
            &staging_buffer1,
            0,
            self.buffer.as_ref().unwrap(),
            0,
            self.buffer_size as wgpu::BufferAddress,
        );

        let staging_buffer2 = device.create_buffer_with_data(
            self.anim_vertex_data.as_bytes(),
            wgpu::BufferUsage::COPY_SRC,
        );

        encoder.copy_buffer_to_buffer(
            &staging_buffer2,
            0,
            self.anim_buffer.as_ref().unwrap(),
            0,
            self.anim_buffer_size as wgpu::BufferAddress,
        );

        queue.submit(&[encoder.finish()]);
    }
}
