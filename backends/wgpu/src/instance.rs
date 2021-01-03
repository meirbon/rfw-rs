use super::mesh::WgpuMesh;
use crate::mesh::SkinningPipeline;
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
    pub fn new(instance: &Instance3D, bounds: &(AABB, Vec<VertexMesh>)) -> Self {
        let transform = instance.get_transform();
        let root_bounds = instance.bounds();
        let mesh_bounds: Vec<AABB> = bounds
            .1
            .iter()
            .map(|m| m.bounds.transformed(transform.to_cols_array()))
            .collect();

        assert_eq!(bounds.1.len(), mesh_bounds.len());

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
    pub instances: TrackedStorage<Instance3D>,
    pub bounds: Vec<InstanceBounds>,
    pub vertex_buffers: Vec<Option<Arc<wgpu::Buffer>>>,
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
            instances: Default::default(),
            bounds: Default::default(),
            vertex_buffers: Default::default(),
            skinning_pipeline: SkinningPipeline::new(device),
        }
    }

    pub fn set(
        &mut self,
        device: &wgpu::Device,
        id: usize,
        instance: Instance3D,
        bounds: &(AABB, Vec<VertexMesh>),
    ) {
        self.instances.overwrite(id, instance);
        if id <= self.bounds.len() {
            self.bounds.push(InstanceBounds::new(&instance, bounds));
        } else {
            self.bounds[id] = InstanceBounds::new(&instance, bounds);
        }

        if self.device_instances.len() <= id {
            self.device_instances =
                DeviceInstances::new((id + 1) * 2, device, &self.bind_group_layout);
            self.instances.trigger_changed_all();
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        meshes: &TrackedStorage<WgpuMesh>,
        skins: &TrackedStorage<Skin>,
        queue: &wgpu::Queue,
    ) {
        if self.instances.is_empty() {
            return;
        }

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let device_instances = &self.device_instances;
        let instance_copy_size = std::mem::size_of::<Mat4>() * 2;
        let staging_data = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instance-staging-mem"),
            size: (self.instances.len() * instance_copy_size) as wgpu::BufferAddress,
            usage: wgpu::BufferUsage::COPY_SRC,
            mapped_at_creation: true,
        });

        {
            let mut data = staging_data
                .slice(0..(self.instances.len() * instance_copy_size) as wgpu::BufferAddress)
                .get_mapped_range_mut();
            let data_ptr = data.as_mut_ptr();

            let instances = &self.instances;
            // let staging_buffer = &self.staging_buffer;
            instances.iter_changed().for_each(|(i, instance)| unsafe {
                let transform = instance.get_transform();
                let n_transform = instance.get_normal_transform();

                std::ptr::copy(
                    transform.as_ref() as *const [f32; 16],
                    (data_ptr as *mut [f32; 16]).add(i * 2),
                    1,
                );
                std::ptr::copy(
                    n_transform.as_ref() as *const [f32; 16],
                    (data_ptr as *mut [f32; 16]).add(i * 2 + 1),
                    1,
                );
            });
        }

        staging_data.unmap();

        let vertex_buffers = &mut self.vertex_buffers;
        vertex_buffers.resize(self.instances.len(), None);
        let instances = &self.instances;

        self.instances.iter_changed().for_each(|(i, inst)| {
            if inst.object_id.is_none() {
                return;
            }

            encoder.copy_buffer_to_buffer(
                &staging_data,
                (i * instance_copy_size) as wgpu::BufferAddress,
                &device_instances.device_matrices,
                DeviceInstances::offset_for(i),
                instance_copy_size as wgpu::BufferAddress,
            );
        });

        let skinning_pipeline = &self.skinning_pipeline;
        vertex_buffers
            .iter_mut()
            .enumerate()
            .par_bridge()
            .for_each(|(i, vb)| {
                if !instances.get_changed(i) {
                    return;
                }

                *vb = if let Some(object_id) = instances[i].object_id {
                    let mesh = &meshes[object_id as usize];
                    if let Some(skin_id) = instances[i].skin_id {
                        Some(Arc::new(skinning_pipeline.apply_skin(
                            device,
                            queue,
                            mesh,
                            &skins[skin_id as usize],
                        )))
                    } else {
                        mesh.buffer.clone()
                    }
                } else {
                    None
                };
            });

        self.bounds = self.get_bounds(meshes);
        queue.submit(std::iter::once(encoder.finish()));
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

    pub fn get(&self, index: usize) -> Option<(&Instance3D, &InstanceBounds)> {
        if let Some(inst) = self.instances.get(index) {
            Some((inst, &self.bounds[index]))
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<(&mut Instance3D, &mut InstanceBounds)> {
        if let Some(inst) = self.instances.get_mut(index) {
            Some((inst, &mut self.bounds[index]))
        } else {
            None
        }
    }

    fn get_bounds(&self, meshes: &TrackedStorage<WgpuMesh>) -> Vec<InstanceBounds> {
        (0..self.instances.len())
            .into_iter()
            .filter(|i| self.instances.get(*i).is_some())
            .map(|i| {
                let instance = &self.instances[i];
                let root_bounds = instance.bounds();
                let mesh_bounds = match instance.object_id {
                    None => vec![AABB::empty(); 1],
                    Some(mesh_id) => {
                        let mesh = &meshes[mesh_id as usize];
                        let transform = instance.get_transform();
                        mesh.desc.meshes
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
    instances: &'a TrackedStorage<Instance3D>,
    bounds: &'a [InstanceBounds],
    current: usize,
    length: usize,
}

impl<'a> Iterator for InstanceIterator<'a> {
    type Item = (usize, &'a Instance3D, &'a InstanceBounds);
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
    instances: &'a mut TrackedStorage<Instance3D>,
    bounds: &'a mut [InstanceBounds],
    current: usize,
    length: usize,
}

impl<'a> Iterator for InstanceIteratorMut<'a> {
    type Item = (usize, &'a mut Instance3D, &'a mut InstanceBounds);
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
