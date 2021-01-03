use super::mesh::WgpuMesh;
use crate::mesh::{SkinningPipeline, WgpuSkin};
use crate::WgpuSettings;
use rayon::prelude::*;
use rfw::prelude::*;
use rfw::scene::mesh::VertexMesh;
use std::num::NonZeroU64;
use std::sync::Arc;

pub struct DeviceInstances {
    pub device_matrices: wgpu::Buffer,
    capacity: usize,
    pub bind_group: wgpu::BindGroup,
}

#[derive(Debug, Clone, Default)]
#[repr(C)]
pub struct DeviceInstance {
    pub matrix: Mat4,
    pub normal_matrix: Mat4,
    _dummy0: Mat4,
    _dummy1: Mat4,
}

impl DeviceInstances {
    pub const INSTANCE_SIZE: usize = 256;
    pub fn new(
        capacity: usize,
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (capacity * Self::INSTANCE_SIZE) as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(buffer.slice(0..256)),
            }],
        });

        Self {
            device_matrices: buffer,
            capacity,
            bind_group,
        }
    }

    pub fn len(&self) -> usize {
        self.capacity
    }

    pub const fn offset_for(instance: usize) -> wgpu::BufferAddress {
        (Self::INSTANCE_SIZE * instance) as wgpu::BufferAddress
    }
}

pub struct InstanceDescriptor {
    pub root_bounds: AABB,
    pub mesh_bounds: Vec<AABB>,
    pub ranges: Vec<VertexMesh>,
    pub changed: bool,
}

impl Default for InstanceDescriptor {
    fn default() -> Self {
        Self {
            root_bounds: AABB::empty(),
            mesh_bounds: Vec::new(),
            ranges: Vec::new(),
            changed: true,
        }
    }
}

impl InstanceDescriptor {
    pub fn new(transform: Mat4, mesh: &WgpuMesh) -> Self {
        let root_bounds = mesh.bounds.transformed(transform.to_cols_array());
        let mesh_bounds: Vec<AABB> = mesh
            .ranges
            .iter()
            .map(|m| m.bounds.transformed(transform.to_cols_array()))
            .collect();

        assert_eq!(mesh.ranges.len(), mesh_bounds.len());

        InstanceDescriptor {
            root_bounds,
            mesh_bounds,
            ranges: mesh.ranges.clone(),
            changed: true,
        }
    }
}

#[derive(Debug)]
pub enum InstanceVertexBuffer {
    None,
    Owned((wgpu::Buffer, wgpu::BufferAddress)),
    Reference(Arc<wgpu::Buffer>),
}

impl Default for InstanceVertexBuffer {
    fn default() -> Self {
        Self::None
    }
}

impl InstanceVertexBuffer {
    pub fn buffer(&self) -> Option<&wgpu::Buffer> {
        match self {
            InstanceVertexBuffer::None => None,
            InstanceVertexBuffer::Owned((b, _)) => Some(b),
            InstanceVertexBuffer::Reference(b) => Some(b),
        }
    }

    pub fn has_buffer(&self) -> bool {
        match self {
            InstanceVertexBuffer::None => false,
            InstanceVertexBuffer::Owned(_) => true,
            InstanceVertexBuffer::Reference(_) => true,
        }
    }
}

pub struct InstanceList {
    pub device_instances: DeviceInstances,
    pub matrices: Vec<Mat4>,
    pub normal_matrices: Vec<Mat4>,
    pub mesh_ids: Vec<MeshID>,
    pub skin_ids: Vec<SkinID>,
    pub changed: BitVec,

    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bounds: Vec<InstanceDescriptor>,
    pub vertex_buffers: Vec<InstanceVertexBuffer>,
    skinning_pipeline: SkinningPipeline,
}

#[allow(dead_code)]
impl InstanceList {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                // Instance matrices
                binding: 0,
                count: None,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer {
                    min_binding_size: NonZeroU64::new(256),
                    dynamic: true,
                },
            }],
            label: Some("mesh-bind-group-descriptor-layout"),
        });

        let device_instances = DeviceInstances::new(32, device, &bind_group_layout);

        Self {
            device_instances,
            bind_group_layout,
            matrices: Default::default(),
            normal_matrices: Default::default(),
            changed: Default::default(),
            mesh_ids: Default::default(),
            skin_ids: Default::default(),
            bounds: Default::default(),
            vertex_buffers: Default::default(),
            skinning_pipeline: SkinningPipeline::new(device),
        }
    }

    pub fn remove(&mut self, id: usize) {
        self.bounds[id] = InstanceDescriptor::default();
        self.matrices[id] = Mat4::zero();
        self.normal_matrices[id] = Mat4::zero();
        self.vertex_buffers[id] = InstanceVertexBuffer::None;
    }

    pub fn set(
        &mut self,
        device: &wgpu::Device,
        id: usize,
        instance: InstanceHandle,
        mesh: Option<&WgpuMesh>,
    ) {
        if id >= self.bounds.len() {
            if let Some(mesh) = mesh {
                self.bounds
                    .push(InstanceDescriptor::new(instance.get_matrix(), mesh));
            } else {
                self.bounds.push(InstanceDescriptor::default());
            }

            self.changed.push(true);
            self.matrices.push(instance.get_matrix());
            self.normal_matrices.push(instance.get_normal_matrix());
            self.mesh_ids.push(instance.get_mesh_id());
            self.skin_ids.push(instance.get_skin_id());
        } else {
            if let Some(mesh) = mesh {
                self.bounds[id] = InstanceDescriptor::new(instance.get_matrix(), mesh);
            } else {
                self.bounds[id] = InstanceDescriptor::default();
            }

            self.changed.set(id, true);
            self.matrices[id] = instance.get_matrix();
            self.normal_matrices[id] = instance.get_normal_matrix();
            self.mesh_ids[id] = instance.get_mesh_id();
            self.skin_ids[id] = instance.get_skin_id();
        }

        if self.device_instances.len() <= self.matrices.len() {
            self.device_instances =
                DeviceInstances::new((id + 1) * 2, device, &self.bind_group_layout);
            self.changed.set_all(true);
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        meshes: &TrackedStorage<WgpuMesh>,
        skins: &TrackedStorage<WgpuSkin>,
        settings: &WgpuSettings,
    ) {
        assert!(
            self.device_instances.len() >= self.matrices.len(),
            "capacity for {} instances but there were {} instances",
            self.device_instances.len(),
            self.matrices.len()
        );

        if self.matrices.is_empty() {
            return;
        }

        let mut bytes = vec![DeviceInstance::default(); self.matrices.len()];
        for (i, instance) in bytes.iter_mut().enumerate() {
            instance.matrix = self.matrices[i];
            instance.normal_matrix = self.normal_matrices[i];
        }
        queue.write_buffer(&self.device_instances.device_matrices, 0, bytes.as_bytes());

        let vertex_buffers = &mut self.vertex_buffers;
        vertex_buffers.resize_with(self.matrices.len(), || Default::default());

        let skinning_pipeline = &self.skinning_pipeline;
        let changed = &self.changed;
        let mesh_ids = &self.mesh_ids;
        let skin_ids = &self.skin_ids;

        vertex_buffers
            .iter_mut()
            .enumerate()
            .par_bridge()
            .for_each(|(i, vb)| {
                if !*changed.get(i).unwrap() && vb.has_buffer() {
                    return;
                }

                let mut success = false;
                let mesh_id = mesh_ids[i];
                if mesh_id.is_valid() {
                    let mesh = if let Some(m) = meshes.get(mesh_id.into()) {
                        m
                    } else if cfg!(debug_assertions) {
                        panic!(
                            "Object {} is expected to have been initialized but it was not.",
                            mesh_id
                        );
                    } else {
                        // Gracefully handle error
                        *vb = InstanceVertexBuffer::None;
                        return;
                    };

                    if settings.enable_skinning {
                        if let Some(skin_id) = skin_ids[i].as_index() {
                            if let Some(skin) = skins.get(skin_id) {
                                if let InstanceVertexBuffer::Owned(buffer) = vb {
                                    // Attempt to use pre-existing buffer
                                    skinning_pipeline
                                        .apply_skin_buffer(device, queue, buffer, mesh, skin);
                                } else {
                                    // Create new buffer
                                    *vb = InstanceVertexBuffer::Owned(
                                        skinning_pipeline.apply_skin(device, queue, mesh, skin),
                                    );
                                }
                                success = true;
                            }
                        } else {
                            if let Some(b) = mesh.buffer.as_ref() {
                                *vb = InstanceVertexBuffer::Reference(b.clone());
                                success = true;
                            }
                        }
                    } else {
                        if let Some(b) = mesh.buffer.as_ref() {
                            *vb = InstanceVertexBuffer::Reference(b.clone());
                            success = true;
                        }
                    }
                }

                if !success {
                    *vb = InstanceVertexBuffer::None;
                }
            });

        self.bounds = self.get_bounds(meshes);
    }

    pub fn reset_changed(&mut self) {
        self.changed.set_all(false);
    }

    pub fn len(&self) -> usize {
        self.matrices.len()
    }

    pub fn changed(&self) -> bool {
        self.changed.any()
    }

    pub fn get(&self, index: usize) -> Option<&InstanceDescriptor> {
        self.bounds.get(index)
    }

    fn get_bounds(&self, meshes: &TrackedStorage<WgpuMesh>) -> Vec<InstanceDescriptor> {
        (0..self.len())
            .into_iter()
            .map(|i| {
                let root_bounds = self.bounds[i].root_bounds;
                let (mesh_bounds, ranges) = match self.mesh_ids.get(i) {
                    Some(mesh_id) if mesh_id.is_valid() => {
                        let mesh = &meshes[mesh_id.as_index().unwrap()];
                        let transform = self.matrices[i];
                        (
                            mesh.ranges
                                .iter()
                                .map(|m| m.bounds.transformed(transform.to_cols_array()))
                                .collect(),
                            mesh.ranges.clone(),
                        )
                    }
                    _ => (vec![AABB::empty(); 1], vec![]),
                };

                InstanceDescriptor {
                    root_bounds,
                    mesh_bounds,
                    ranges,
                    changed: *self.changed.get(i).unwrap(),
                }
            })
            .collect()
    }

    pub fn iter(&self) -> InstanceIterator<'_> {
        let length = self.matrices.len();

        InstanceIterator {
            vertex_buffers: self.vertex_buffers.as_slice(),
            bounds: self.bounds.as_slice(),
            current: 0,
            length,
        }
    }

    pub fn iter_sorted(&self, eye: Vec3, direction: Vec3) -> SortedInstanceIterator<'_> {
        let mut ids: Vec<usize> = (0..self.matrices.len())
            .into_iter()
            .filter(|i| (self.bounds[*i].root_bounds.center::<Vec3>() - eye).dot(direction) > 0.0)
            .collect();

        ids.sort_by(|a, b| {
            let a = *a;
            let b = *b;

            let a = &self.bounds[a];
            let b = &self.bounds[b];

            let a: Vec3 = a.root_bounds.center();
            let b: Vec3 = b.root_bounds.center();

            let dist_a = (a - eye).distance_squared(eye);
            let dist_b = (b - eye).distance_squared(eye);

            if dist_a < dist_b {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });

        SortedInstanceIterator {
            ids,
            vertex_buffers: self.vertex_buffers.as_slice(),
            bounds: self.bounds.as_slice(),
        }
    }
}

pub struct SortedInstanceIterator<'a> {
    ids: Vec<usize>,
    vertex_buffers: &'a [InstanceVertexBuffer],
    bounds: &'a [InstanceDescriptor],
}

impl<'a> Iterator for SortedInstanceIterator<'a> {
    type Item = (usize, &'a wgpu::Buffer, &'a InstanceDescriptor);
    fn next(&mut self) -> Option<Self::Item> {
        let bounds = self.bounds.as_ptr();

        while let Some(id) = self.ids.pop() {
            if let Some(buffer) = self.vertex_buffers.get(id) {
                if let Some(buffer) = buffer.buffer() {
                    unsafe {
                        return Some((id, buffer, bounds.add(id).as_ref().unwrap()));
                    }
                }
            }
        }

        None
    }
}

pub struct InstanceIterator<'a> {
    vertex_buffers: &'a [InstanceVertexBuffer],
    bounds: &'a [InstanceDescriptor],
    current: usize,
    length: usize,
}

impl<'a> Iterator for InstanceIterator<'a> {
    type Item = (usize, &'a wgpu::Buffer, &'a InstanceDescriptor);
    fn next(&mut self) -> Option<Self::Item> {
        let bounds = self.bounds.as_ptr();

        while self.current < self.length {
            if let Some(buffer) = self.vertex_buffers.get(self.current) {
                self.current += 1;
                if let Some(buffer) = buffer.buffer() {
                    unsafe {
                        return Some((
                            self.current - 1,
                            buffer,
                            bounds.add(self.current - 1).as_ref().unwrap(),
                        ));
                    }
                }
            }
        }

        None
    }
}
