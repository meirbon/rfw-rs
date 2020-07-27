use super::mesh::DeferredMesh;
use crate::wgpu_renderer::mesh::DeferredAnimMesh;
use glam::*;
use rtbvh::{Bounds, AABB};
use scene::{Instance, ObjectRef, TrackedStorage};

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

        Self {
            device_instances: Vec::new(),
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
        self.bounds
            .push(InstanceBounds::new(&instance, mesh));
        self.instances.overwrite(id, instance);
        self.device_instances
            .push(DeviceInstance::new(device, &self.bind_group_layout));
    }

    pub fn set_animated(
        &mut self,
        device: &wgpu::Device,
        id: usize,
        instance: Instance,
        mesh: &DeferredAnimMesh,
    ) {
        self.bounds
            .push(InstanceBounds::new_animated(&instance, mesh));
        self.instances.overwrite(id, instance);
        self.device_instances
            .push(DeviceInstance::new(device, &self.bind_group_layout));
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        meshes: &TrackedStorage<DeferredMesh>,
        anim_meshes: &TrackedStorage<DeferredAnimMesh>,
    ) -> Vec<super::CopyCommand> {
        let mut commands = Vec::with_capacity(self.instances.len());

        let device_instances = &self.device_instances;

        (0..self.instances.len()).into_iter().filter(|i| {
            match self.instances.get(*i) {
                None => false,
                Some(_) => self.instances.get_changed(*i),
            }
        }).for_each(|i| {
            let instance = &self.instances[i];
            let data = [instance.get_transform(), instance.get_normal_transform()];
            let staging_buffer = device.create_buffer_with_data(
                unsafe {
                    std::slice::from_raw_parts(
                        data.as_ptr() as *const u8,
                        std::mem::size_of::<Mat4>() * 2,
                    )
                },
                wgpu::BufferUsage::COPY_SRC,
            );

            commands.push(super::CopyCommand {
                destination_buffer: &device_instances[i].device_matrices,
                copy_size: std::mem::size_of::<Mat4>() as wgpu::BufferAddress * 2,
                staging_buffer,
            });
        });

        self.bounds = self.get_bounds(meshes, anim_meshes);

        commands
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
}
