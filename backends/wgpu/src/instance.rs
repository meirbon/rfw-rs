use crate::mesh::{SkinningPipeline, WgpuMesh, WgpuSkin};
use rfw::backend::InstancesData3D;
use rfw::math::*;
use rfw::scene::bvh::AABB;
use rfw::utils::BytesConversion;
use std::sync::Arc;

#[derive(Debug)]
pub struct InstanceList {
    instance_capacity: usize,
    instances: u32,
    instance_buffers: Vec<Arc<Option<wgpu::Buffer>>>,
    pub instances_buffer: Arc<Option<wgpu::Buffer>>,
    pub instances_bg: Arc<Option<wgpu::BindGroup>>,
    pub instances_bounds: Vec<AABB>,
    pub supports_skinning: bool,
}

impl Default for InstanceList {
    fn default() -> Self {
        Self {
            instance_capacity: 0,
            instances: 0,
            instance_buffers: Vec::new(),
            instances_buffer: Arc::new(None),
            instances_bg: Arc::new(None),
            instances_bounds: Vec::new(),
            supports_skinning: false,
        }
    }
}

impl Clone for InstanceList {
    fn clone(&self) -> Self {
        Self {
            instance_capacity: self.instance_capacity,
            instances: self.instances,
            instance_buffers: self.instance_buffers.clone(),
            instances_buffer: self.instances_buffer.clone(),
            instances_bg: self.instances_bg.clone(),
            instances_bounds: self.instances_bounds.clone(),
            supports_skinning: self.supports_skinning,
        }
    }
}

#[allow(dead_code)]
impl InstanceList {
    const DEFAULT_CAPACITY: usize = 4;

    pub fn new(device: &wgpu::Device, instances_layout: &wgpu::BindGroupLayout) -> Self {
        let instances_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (Self::DEFAULT_CAPACITY * std::mem::size_of::<Mat4>() * 2) as _,
            usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        }));

        let instances_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: instances_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(
                    instances_buffer.as_ref().unwrap().slice(..),
                ),
            }],
        }));

        Self {
            instance_capacity: Self::DEFAULT_CAPACITY,
            instances: 0,
            instance_buffers: Vec::new(),
            instances_buffer: Arc::new(instances_buffer),
            instances_bg: Arc::new(instances_bg),
            instances_bounds: vec![AABB::empty(); Self::DEFAULT_CAPACITY],
            supports_skinning: false,
        }
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &WgpuMesh,
        instances: InstancesData3D<'_>,
        instances_layout: &wgpu::BindGroupLayout,
        skins: &[WgpuSkin],
        skinning_pipeline: &SkinningPipeline,
    ) {
        self.instances = instances.len() as _;
        if instances.len() > self.instance_capacity as usize || self.instances_buffer.is_none() {
            self.instance_capacity = instances.len().next_power_of_two() as _;
            self.instances_buffer = Arc::new(Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: (self.instance_capacity as usize * std::mem::size_of::<Mat4>() * 2) as _,
                usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
                mapped_at_creation: false,
            })));

            self.instances_bg =
                Arc::new(Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: instances_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(
                            (*self.instances_buffer).as_ref().unwrap().slice(..),
                        ),
                    }],
                })));
        }

        self.instances_bounds
            .resize(instances.len(), AABB::default());
        self.instance_buffers
            .resize(instances.len(), Arc::new(None));

        let mut matrices = Vec::with_capacity(instances.len() * 2);
        for (i, m) in instances.matrices.iter().enumerate() {
            matrices.push(*m);
            matrices.push(m.inverse().transpose());
            self.instances_bounds[i] = mesh.bounds.transformed(m.to_cols_array());
            self.instance_buffers[i] = if let Some(skin) = instances.skin_ids[i].as_index() {
                Arc::new(Some(
                    skinning_pipeline
                        .apply_skin(device, queue, mesh, &skins[skin])
                        .0,
                ))
            } else {
                mesh.buffer.clone()
            };
        }

        queue.write_buffer(
            (*self.instances_buffer).as_ref().unwrap(),
            0,
            matrices.as_bytes(),
        );

        assert!(instances.len() > 0);
        self.supports_skinning = mesh.joints_weights_buffer.is_some();
    }

    pub fn buffer_for(&self, i: usize) -> Option<&wgpu::Buffer> {
        if let Some(buffer) = self.instance_buffers.get(i) {
            buffer.as_ref().as_ref()
        } else {
            None
        }
    }

    pub fn len(&self) -> u32 {
        self.instances
    }

    pub fn is_empty(&self) -> bool {
        self.instances == 0
    }
}
