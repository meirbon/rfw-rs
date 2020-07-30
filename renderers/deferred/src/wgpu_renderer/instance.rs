use super::mesh::DeferredMesh;
use crate::wgpu_renderer::mesh::DeferredAnimMesh;
use glam::*;
use rtbvh::{Bounds, AABB};
use scene::{Instance, ObjectRef, TrackedStorage};
use crate::wgpu_renderer::CopyStagingBuffer;

pub struct DeviceInstance {
    pub device_matrices: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
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
            .map(|m| m.bounds.transformed(transform))
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
            .map(|m| m.bounds.transformed(transform))
            .collect();

        assert_eq!(mesh.sub_meshes.len(), mesh_bounds.len());

        InstanceBounds {
            root_bounds,
            mesh_bounds,
            changed: true,
        }
    }
}

impl DeviceInstance {
    pub fn new(device: &wgpu::Device, bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<Mat4>() as wgpu::BufferAddress * 2,
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
            bind_group,
        }
    }
}

pub struct InstanceList {
    pub device_instances: Vec<DeviceInstance>,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub instances: TrackedStorage<Instance>,
    pub bounds: Vec<InstanceBounds>,
    pub staging_buffer: wgpu::Buffer,
    pub staging_size: wgpu::BufferAddress,
}

impl InstanceList {
    pub fn new(device: &wgpu::Device) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            bindings: &[wgpu::BindGroupLayoutEntry {
                // Instance matrices
                binding: 0,
                visibility: wgpu::ShaderStage::VERTEX,
                ty: wgpu::BindingType::UniformBuffer { dynamic: false },
            }],
            label: Some("mesh-bind-group-descriptor-layout"),
        });

        let staging_size = (32 * std::mem::size_of::<Mat4>() * 2) as wgpu::BufferAddress;
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance-list-staging-buffer"),
            size: staging_size,
            usage: wgpu::BufferUsage::MAP_WRITE | wgpu::BufferUsage::COPY_SRC,
        });

        Self {
            device_instances: Vec::new(),
            bind_group_layout,
            instances: TrackedStorage::new(),
            bounds: Vec::new(),
            staging_buffer,
            staging_size,
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
            self.device_instances
                .push(DeviceInstance::new(device, &self.bind_group_layout));
        } else {
            self.bounds[id] = InstanceBounds::new(&instance, mesh);
            self.device_instances[id] = DeviceInstance::new(device, &self.bind_group_layout);
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
            self.device_instances
                .push(DeviceInstance::new(device, &self.bind_group_layout));
        } else {
            self.bounds[id] = InstanceBounds::new_animated(&instance, mesh);
            self.device_instances[id] = DeviceInstance::new(device, &self.bind_group_layout);
        }
    }

    pub async fn update(
        &mut self,
        device: &wgpu::Device,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let mut commands = Vec::with_capacity(self.instances.len());

        let device_instances = &self.device_instances;

        let instance_copy_size = std::mem::size_of::<Mat4>() * 2;
        // Resize if needed
        if (self.instances.len() * instance_copy_size) < self.staging_size as usize {
            self.staging_size = (self.instances.len() * 2 * instance_copy_size) as wgpu::BufferAddress;
            self.staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("instance-list-staging-buffer"),
                size: self.staging_size,
                usage: wgpu::BufferUsage::MAP_WRITE | wgpu::BufferUsage::COPY_SRC,
            });
        }

        let staging_data = self.staging_buffer.map_write(0, self.staging_size);
        device.poll(wgpu::Maintain::Wait);
        let mut staging_data = staging_data.await.unwrap();

        let copy_data = staging_data.as_slice();

        let instances = &self.instances;
        let staging_buffer = &self.staging_buffer;
        instances.iter_changed().for_each(|(i, instance)| {
            unsafe {
                let transform = instance.get_transform();
                let n_transform = instance.get_normal_transform();

                std::ptr::copy(&transform as *const Mat4, (copy_data.as_mut_ptr() as *mut Mat4).add(i * 2), 1);
                std::ptr::copy(&n_transform as *const Mat4, (copy_data.as_mut_ptr() as *mut Mat4).add(i * 2 + 1), 1);
            }

            commands.push(super::CopyCommand {
                destination_buffer: &device_instances[i].device_matrices,
                offset: (i * instance_copy_size) as wgpu::BufferAddress,
                copy_size: instance_copy_size as wgpu::BufferAddress,
                staging_buffer: CopyStagingBuffer::Reference(staging_buffer),
            });
        });

        self.bounds = self.get_bounds(meshes, anim_meshes);
        commands.iter().for_each(|c| {
            c.record(encoder);
        });
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
                            .map(|m| m.bounds.transformed(transform))
                            .collect()
                    }
                    ObjectRef::Animated(mesh_id) => {
                        let mesh = &anim_meshes[mesh_id as usize];
                        let transform = instance.get_transform();
                        mesh.sub_meshes
                            .iter()
                            .map(|m| m.bounds.transformed(transform))
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
            device_instances: self.device_instances.as_slice(),
            bounds: self.bounds.as_slice(),
            current: 0,
            length,
        }
    }

    pub fn iter_mut(&mut self) -> InstanceIteratorMut<'_> {
        let length = self.instances.len();
        InstanceIteratorMut {
            instances: &mut self.instances,
            device_instances: self.device_instances.as_mut_slice(),
            bounds: self.bounds.as_mut_slice(),
            current: 0,
            length,
        }
    }
}

pub struct InstanceIterator<'a> {
    instances: &'a TrackedStorage<Instance>,
    device_instances: &'a [DeviceInstance],
    bounds: &'a [InstanceBounds],
    current: usize,
    length: usize,
}

impl<'a> Iterator for InstanceIterator<'a> {
    type Item = (usize, &'a Instance, &'a DeviceInstance, &'a InstanceBounds);
    fn next(&mut self) -> Option<Self::Item> {
        let (instances, device_instances, bounds) = unsafe {
            (
                self.instances.as_ptr(),
                self.device_instances.as_ptr(),
                self.bounds.as_ptr(),
            )
        };

        while self.current < self.length {
            if let Some(_) = self.instances.get(self.current) {
                let value = unsafe {
                    (
                        self.current,
                        instances.add(self.current).as_ref().unwrap(),
                        device_instances.add(self.current).as_ref().unwrap(),
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
    device_instances: &'a mut [DeviceInstance],
    bounds: &'a mut [InstanceBounds],
    current: usize,
    length: usize,
}

impl<'a> Iterator for InstanceIteratorMut<'a> {
    type Item = (
        usize,
        &'a mut Instance,
        &'a mut DeviceInstance,
        &'a mut InstanceBounds,
    );
    fn next(&mut self) -> Option<Self::Item> {
        let (instances, device_instances, bounds) = unsafe {
            (
                self.instances.as_mut_ptr(),
                self.device_instances.as_mut_ptr(),
                self.bounds.as_mut_ptr(),
            )
        };

        while self.current < self.length {
            if let Some(_) = self.instances.get(self.current) {
                let value = unsafe {
                    (
                        self.current,
                        instances.add(self.current).as_mut().unwrap(),
                        device_instances.add(self.current).as_mut().unwrap(),
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
