use crate::mem::ManagedBuffer;
use crate::objects::Matrices;
use metal::{Buffer, DeviceRef, MTLResourceOptions};
use rfw::backend::{InstancesData2D, JointData, MeshData2D, MeshData3D, Vertex2D, Vertex3D};
use rfw::math::Mat4;
use rfw::utils::BytesConversion;

pub struct MetalMesh2D {
    pub(crate) buffer: ManagedBuffer<Vertex2D>,
    pub(crate) vertices: usize,
    pub(crate) instances: usize,
    pub(crate) instance_buffer: ManagedBuffer<Mat4>,
    pub(crate) tex_id: Option<usize>,
}

impl MetalMesh2D {
    pub const DEFAULT_CAPACITY: usize = 4;
    pub fn new(device: &DeviceRef, mesh: MeshData2D) -> Self {
        let buffer = ManagedBuffer::with_data(device, mesh.vertices);
        let instance_buffer = ManagedBuffer::new(device, Self::DEFAULT_CAPACITY);

        Self {
            buffer,
            vertices: mesh.vertices.len(),
            instances: 0,
            instance_buffer,
            tex_id: mesh.tex_id,
        }
    }

    pub fn set_data(&mut self, device: &DeviceRef, mesh: MeshData2D) {
        if self.buffer.len() < mesh.vertices.len() {
            self.buffer = ManagedBuffer::with_data(device, mesh.vertices);
        } else {
            self.buffer.as_mut(|slice| {
                slice[0..mesh.vertices.len()].copy_from_slice(mesh.vertices);
            });
        }

        self.tex_id = mesh.tex_id;
        self.vertices = mesh.vertices.len();
    }

    pub fn set_instances(&mut self, device: &DeviceRef, instances: InstancesData2D<'_>) {
        if instances.len() > self.instance_buffer.len() {
            self.instance_buffer = ManagedBuffer::new(device, instances.len().next_power_of_two());
        }

        self.instance_buffer.as_mut(|slice| {
            slice[0..instances.matrices.len()].copy_from_slice(instances.matrices);
        });

        self.instances = instances.len();
    }
}
