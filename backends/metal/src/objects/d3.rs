use crate::mem::ManagedBuffer;
use metal::*;
use rfw::prelude::*;

#[derive(Debug, Default, Copy, Clone)]
#[repr(C)]
pub struct Matrices {
    pub transform: Mat4,
    pub normal_transform: Mat4,
}

pub struct MetalMesh3D {
    pub(crate) buffer: Buffer,
    pub(crate) skin_buffer: Option<Buffer>,
    pub(crate) vertices: usize,
    pub(crate) skin_len: usize,
    pub(crate) instances: usize,
    pub(crate) instance_buffer: ManagedBuffer<Matrices>,
}

impl MetalMesh3D {
    pub const DEFAULT_CAPACITY: usize = 4;
    pub fn new(device: &DeviceRef, mesh: MeshData3D) -> Self {
        let buffer = device.new_buffer_with_data(
            mesh.vertices.as_ptr() as _,
            (mesh.vertices.len() * std::mem::size_of::<Vertex3D>()) as _,
            MTLResourceOptions::StorageModeManaged,
        );

        let skin_buffer = if !mesh.skin_data.is_empty() {
            Some(device.new_buffer_with_data(
                mesh.skin_data.as_ptr() as _,
                (mesh.skin_data.len() * std::mem::size_of::<JointData>()) as _,
                MTLResourceOptions::StorageModeManaged,
            ))
        } else {
            None
        };

        let instance_buffer = ManagedBuffer::new(device, Self::DEFAULT_CAPACITY);

        Self {
            buffer,
            skin_buffer,
            vertices: mesh.vertices.len(),
            skin_len: mesh.skin_data.len(),
            instances: 0,
            instance_buffer,
        }
    }

    pub fn set_instances(&mut self, device: &DeviceRef, instances: InstancesData3D<'_>) {
        if instances.len() > self.instance_buffer.len() {
            self.instance_buffer = ManagedBuffer::new(device, Self::DEFAULT_CAPACITY);
        }

        self.instance_buffer.as_mut(|slice| {
            for i in 0..instances.len() {
                slice[i].transform = instances.matrices[i];
                slice[i].normal_transform = instances.matrices[i].inverse().transpose();
            }
        });

        self.instances = instances.len();
    }
}
