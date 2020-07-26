use super::mesh::DeferredMesh;
use glam::*;
use rayon::prelude::*;
use rtbvh::{Bounds, AABB};
use scene::{BitVec, Instance, ObjectRef};

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
        let mesh_bounds = mesh
            .sub_meshes
            .iter()
            .map(|m| m.bounds.transformed(transform))
            .collect();

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
    pub instances: Vec<Instance>,
    pub bounds: Vec<InstanceBounds>,
    pub changed: BitVec,
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
            instances: Vec::new(),
            changed: BitVec::new(),
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
        if id >= self.instances.len() {
            self.bounds.push(InstanceBounds::new(&instance, mesh));
            self.instances.push(instance);
            self.device_instances
                .push(DeviceInstance::new(device, &self.bind_group_layout));
            self.changed.push(true);
        } else {
            self.bounds[id] = InstanceBounds::new(&instance, mesh);
            self.instances[id] = instance;
            self.changed.set(id, true);
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        meshes: &[DeferredMesh],
    ) -> Vec<super::CopyCommand> {
        let mut commands = Vec::with_capacity(self.instances.len());

        for i in 0..self.instances.len() {
            if !self.changed.get(i).unwrap() {
                continue;
            }

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
                destination_buffer: &self.device_instances[i].device_matrices,
                copy_size: std::mem::size_of::<Mat4>() as wgpu::BufferAddress * 2,
                staging_buffer,
            });
        }

        self.bounds = self.get_bounds(meshes);

        commands
    }

    pub fn reset_changed(&mut self) {
        self.changed.set_all(false);
    }

    pub fn len(&self) -> usize {
        self.instances.len()
    }

    pub fn changed(&self) -> bool {
        self.changed.any()
    }

    fn get_bounds(&self, meshes: &[DeferredMesh]) -> Vec<InstanceBounds> {
        (0..self.instances.len())
            .into_iter()
            .par_bridge()
            .map(|i| {
                let instance = &self.instances[i];
                let root_bounds = instance.bounds();

                let mesh = match instance.object_id {
                    ObjectRef::None => panic!("Invalid"),
                    ObjectRef::Static(mesh_id) => &meshes[mesh_id as usize],
                    ObjectRef::Animated(_) => unimplemented!(),
                };

                let transform = instance.get_transform();
                let mesh_bounds = mesh
                    .sub_meshes
                    .iter()
                    .map(|m| m.bounds.transformed(transform))
                    .collect();

                InstanceBounds {
                    root_bounds,
                    mesh_bounds,
                    changed: *self.changed.get(i).unwrap(),
                }
            })
            .collect()
    }
}
