use super::mesh::DeferredMesh;
use crate::mesh::DeferredAnimMesh;
use glam::*;
use rtbvh::{Bounds, AABB};
use scene::{Instance, ObjectRef, TrackedStorage};

pub struct DeviceInstances {
    pub device_matrices: wgpu::Buffer,
    capacity: usize,
    pub bind_group: wgpu::BindGroup,
}

impl DeviceInstances {
    // std::mem::size_of::<Mat4>() * 2
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
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: bind_group_layout,
            bindings: &[wgpu::Binding {
                binding: 0,
                resource: wgpu::BindingResource::Buffer {
                    buffer: &buffer,
                    range: 0..(2 * std::mem::size_of::<Mat4>()) as wgpu::BufferAddress,
                },
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

    pub const fn dynamic_offset_for(instance: usize) -> u32 {
        (256 * instance) as u32
    }
}

pub struct InstanceBounds {
    pub root_bounds: AABB,
    pub mesh_bounds: Vec<AABB>,
    pub changed: bool,
}

impl InstanceBounds {
    pub fn new(instance: &Instance, mesh: &DeferredMesh) -> Self {
        let transform = instance.get_transform();
        let root_bounds = instance.bounds();
        let mesh_bounds: Vec<AABB> = mesh
            .sub_meshes
            .iter()
            .map(|m| m.bounds.transformed(transform.to_cols_array()))
            .collect();

        assert_eq!(mesh.sub_meshes.len(), mesh_bounds.len());

        InstanceBounds {
            root_bounds,
            mesh_bounds,
            changed: true,
        }
    }

    pub fn new_animated(instance: &Instance, mesh: &DeferredAnimMesh) -> Self {
        let transform = instance.get_transform();
        let root_bounds = instance.bounds();
        let mesh_bounds: Vec<AABB> = mesh
            .sub_meshes
            .iter()
            .map(|m| m.bounds.transformed(transform.to_cols_array()))
            .collect();

        assert_eq!(mesh.sub_meshes.len(), mesh_bounds.len());

        InstanceBounds {
            root_bounds,
            mesh_bounds,
            changed: true,
        }
    }
}

pub struct InstanceList {
    pub device_instances: DeviceInstances,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub instances: TrackedStorage<Instance>,
    pub bounds: Vec<InstanceBounds>,
}

#[allow(dead_code)]
impl InstanceList {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[wgpu::BindGroupLayoutEntry {
                // Instance matrices
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: true },
            }],
            label: Some("mesh-bind-group-descriptor-layout"),
        });

        let device_instances = DeviceInstances::new(32, device, &bind_group_layout);

        Self {
            device_instances,
            bind_group_layout,
            instances: TrackedStorage::new(),
            bounds: Vec::new(),
        }
    }

    pub fn set(
        &mut self,
        device: &wgpu::Device,
        id: usize,
        instance: Instance,
        mesh: &DeferredMesh,
    ) {
        self.instances.overwrite(id, instance);
        if id <= self.bounds.len() {
            self.bounds.push(InstanceBounds::new(&instance, mesh));
        } else {
            self.bounds[id] = InstanceBounds::new(&instance, mesh);
        }

        if self.device_instances.len() <= id {
            self.device_instances =
                DeviceInstances::new((id + 1) * 2, device, &self.bind_group_layout);
            self.instances.trigger_changed_all();
        }
    }

    pub fn set_animated(
        &mut self,
        device: &wgpu::Device,
        id: usize,
        instance: Instance,
        mesh: &DeferredAnimMesh,
    ) {
        self.instances.overwrite(id, instance);
        if id <= self.bounds.len() {
            self.bounds
                .push(InstanceBounds::new_animated(&instance, mesh));
        } else {
            self.bounds[id] = InstanceBounds::new_animated(&instance, mesh);
        }

        if self.device_instances.len() <= id {
            self.device_instances =
                DeviceInstances::new((id + 1) * 2, device, &self.bind_group_layout);
            self.instances.trigger_changed_all();
        }
    }

    pub async fn update(
        &mut self,
        device: &wgpu::Device,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
        queue: &wgpu::Queue,
    ) {
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let device_instances = &self.device_instances;

        if !self.instances.is_empty() {
            let instance_copy_size = std::mem::size_of::<Mat4>() * 2;
            let staging_data = device.create_buffer_mapped(&wgpu::BufferDescriptor {
                label: Some("instance-staging-buffer"),
                size: (self.instances.len() * instance_copy_size) as wgpu::BufferAddress,
                usage: wgpu::BufferUsage::COPY_SRC,
            });

            let instances = &self.instances;
            // let staging_buffer = &self.staging_buffer;
            instances.iter_changed().for_each(|(i, instance)| unsafe {
                let transform = instance.get_transform();
                let n_transform = instance.get_normal_transform();

                std::ptr::copy(
                    transform.as_ref() as *const [f32; 16],
                    (staging_data.data.as_mut_ptr() as *mut  [f32; 16]).add(i * 2),
                    1,
                );
                std::ptr::copy(
                    n_transform.as_ref() as *const [f32; 16],
                    (staging_data.data.as_mut_ptr() as *mut [f32; 16]).add(i * 2 + 1),
                    1,
                );
            });

            let staging_buffer = staging_data.finish();

            instances.iter_changed().for_each(|(i, _)| {
                encoder.copy_buffer_to_buffer(
                    &staging_buffer,
                    (i * instance_copy_size) as wgpu::BufferAddress,
                    &device_instances.device_matrices,
                    DeviceInstances::offset_for(i),
                    instance_copy_size as wgpu::BufferAddress,
                );
            });
        }

        self.bounds = self.get_bounds(meshes, anim_meshes);
        queue.submit(&[encoder.finish()]);
    }

    pub fn reset_changed(&mut self) {
        self.instances.reset_changed();
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn changed(&self) -> bool {
        self.instances.any_changed()
    }

    fn get_bounds(
        &self,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
    ) -> Vec<InstanceBounds> {
        (0..self.instances.len())
            .into_iter()
            .filter(|i| self.instances.get(*i).is_some())
            .map(|i| {
                let instance = &self.instances[i];
                let root_bounds = instance.bounds();
                let mesh_bounds = match instance.object_id {
                    ObjectRef::None => panic!("Invalid"),
                    ObjectRef::Static(mesh_id) => {
                        let mesh = &meshes[mesh_id as usize];
                        let transform = instance.get_transform();
                        mesh.sub_meshes
                            .iter()
                            .map(|m| m.bounds.transformed(transform.to_cols_array()))
                            .collect()
                    }
                    ObjectRef::Animated(mesh_id) => {
                        let mesh = &anim_meshes[mesh_id as usize];
                        let transform = instance.get_transform();
                        mesh.sub_meshes
                            .iter()
                            .map(|m| m.bounds.transformed(transform.to_cols_array()))
                            .collect()
                    }
                };

                InstanceBounds {
                    root_bounds,
                    mesh_bounds,
                    changed: self.instances.get_changed(i),
                }
            })
            .collect()
    }

    pub fn iter(&self) -> InstanceIterator<'_> {
        let length = self.instances.len();

        InstanceIterator {
            instances: &self.instances,
            bounds: self.bounds.as_slice(),
            current: 0,
            length,
        }
    }

    pub fn iter_mut(&mut self) -> InstanceIteratorMut<'_> {
        let length = self.instances.len();
        InstanceIteratorMut {
            instances: &mut self.instances,
            bounds: self.bounds.as_mut_slice(),
            current: 0,
            length,
        }
    }
}

pub struct InstanceIterator<'a> {
    instances: &'a TrackedStorage<Instance>,
    bounds: &'a [InstanceBounds],
    current: usize,
    length: usize,
}

impl<'a> Iterator for InstanceIterator<'a> {
    type Item = (usize, &'a Instance, &'a InstanceBounds);
    fn next(&mut self) -> Option<Self::Item> {
        let (instances, bounds) = unsafe { (self.instances.as_ptr(), self.bounds.as_ptr()) };

        while self.current < self.length {
            if let Some(_) = self.instances.get(self.current) {
                let value = unsafe {
                    (
                        self.current,
                        instances.add(self.current).as_ref().unwrap(),
                        bounds.add(self.current).as_ref().unwrap(),
                    )
                };
                self.current += 1;
                return Some(value);
            }
        }

        None
    }
}

pub struct InstanceIteratorMut<'a> {
    instances: &'a mut TrackedStorage<Instance>,
    bounds: &'a mut [InstanceBounds],
    current: usize,
    length: usize,
}

impl<'a> Iterator for InstanceIteratorMut<'a> {
    type Item = (usize, &'a mut Instance, &'a mut InstanceBounds);
    fn next(&mut self) -> Option<Self::Item> {
        let (instances, bounds) =
            unsafe { (self.instances.as_mut_ptr(), self.bounds.as_mut_ptr()) };

        while self.current < self.length {
            if let Some(_) = self.instances.get(self.current) {
                let value = unsafe {
                    (
                        self.current,
                        instances.add(self.current).as_mut().unwrap(),
                        bounds.add(self.current).as_mut().unwrap(),
                    )
                };
                self.current += 1;
                return Some(value);
            }
        }

        None
    }
}
