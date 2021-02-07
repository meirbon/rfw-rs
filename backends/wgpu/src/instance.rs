use crate::mesh::{SkinningPipeline, WgpuMesh, WgpuSkin};
use rfw::math::*;
use rfw::scene::bvh::AABB;
use rfw::utils::BytesConversion;
use rfw::{backend::InstancesData3D, prelude::FrustrumPlane};
use std::sync::Arc;

#[derive(Debug)]
pub struct InstanceList {
    instance_capacity: usize,
    instances: u32,
    instance_buffers: Vec<Arc<Option<wgpu::Buffer>>>,
    instance_matrices: Vec<InstanceMatrices>,
    pub instances_buffer: Arc<Option<wgpu::Buffer>>,
    pub instances_bg: Arc<Option<wgpu::BindGroup>>,
    pub instances_bounds: Vec<AABB>,
    pub supports_skinning: bool,
    // pub descriptor: MeshDescriptor,
    // pub mesh_desc: Option<Arc<wgpu::Buffer>>,
    // pub draw_buffer: Option<Arc<wgpu::Buffer>>,
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct InstanceMatrices {
    pub matrix: Mat4,
    pub normal: Mat4,
}

impl Default for InstanceList {
    fn default() -> Self {
        Self {
            instance_capacity: 0,
            instances: 0,
            instance_buffers: Vec::new(),
            instance_matrices: Vec::new(),
            instances_buffer: Arc::new(None),
            instances_bg: Arc::new(None),
            instances_bounds: Vec::new(),
            supports_skinning: false,
            // descriptor: Default::default(),
            // mesh_desc: None,
            // draw_buffer: None,
        }
    }
}

impl Clone for InstanceList {
    fn clone(&self) -> Self {
        Self {
            instance_capacity: self.instance_capacity,
            instances: self.instances,
            instance_buffers: self.instance_buffers.clone(),
            instance_matrices: self.instance_matrices.clone(),
            instances_buffer: self.instances_buffer.clone(),
            instances_bg: self.instances_bg.clone(),
            instances_bounds: self.instances_bounds.clone(),
            supports_skinning: self.supports_skinning,
            // descriptor: self.descriptor,
            // mesh_desc: self.mesh_desc.clone(),
            // draw_buffer: self.draw_buffer.clone(),
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
                resource: wgpu::BindingResource::Buffer {
                    buffer: instances_buffer.as_ref().unwrap(),
                    offset: 0,
                    size: None,
                },
            }],
        }));

        Self {
            instance_capacity: Self::DEFAULT_CAPACITY,
            instances: 0,
            instance_buffers: Vec::new(),
            instance_matrices: Vec::new(),
            instances_buffer: Arc::new(instances_buffer),
            instances_bg: Arc::new(instances_bg),
            instances_bounds: vec![AABB::empty(); Self::DEFAULT_CAPACITY],
            supports_skinning: false,
            // descriptor: MeshDescriptor::default(),
            // mesh_desc: None,
            // draw_buffer: None,
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
                        resource: (*self.instances_buffer).as_ref().unwrap().as_entire_binding(),
                    }],
                })));
        }

        self.instance_matrices
            .resize(instances.len(), Default::default());
        self.instances_bounds
            .resize(instances.len(), AABB::default());
        self.instance_buffers
            .resize(instances.len(), Arc::new(None));

        instances
            .matrices
            .iter()
            .enumerate()
            .zip(self.instance_matrices.iter_mut())
            .zip(self.instance_buffers.iter_mut())
            .zip(self.instances_bounds.iter_mut())
            .for_each(|((((i, m), matrices), buffer), bounds)| {
                *bounds = mesh.bounds.transformed(m.to_cols_array());
                *buffer = if let Some(skin) = instances.skin_ids[i].as_index() {
                    Arc::new(Some(
                        skinning_pipeline
                            .apply_skin(device, queue, mesh, &skins[skin])
                            .0,
                    ))
                } else {
                    mesh.buffer.clone()
                };

                *matrices = InstanceMatrices {
                    matrix: *m,
                    normal: m.inverse().transpose(),
                };
            });

        queue.write_buffer(
            (*self.instances_buffer).as_ref().unwrap(),
            0,
            self.instance_matrices.as_bytes(),
        );

        self.supports_skinning = mesh.joints_weights_buffer.is_some();

        // if self.instances > 5000 {
        //     self.draw_buffer = Some(Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
        //         label: None,
        //         size: self.instances as usize * std::mem::size_of::<DrawCommand>(),
        //         usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
        //         mapped_at_creation: false,
        //     })));
        // }

        // let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        //     label: None,
        //     size: (std::mem::size_of::<FrustrumPlane>() * 6 + std::mem::size_of::<MeshDescriptor>())
        //         as _,
        //     usage: wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
        //     mapped_at_creation: true,
        // });

        // let data = unsafe {
        //     (buffer.slice(..).get_mapped_range_mut().as_mut_ptr() as *mut CullData)
        //         .as_mut()
        //         .unwrap()
        // };
        // let desc = MeshDescriptor {
        //     vertex_count: mesh.ranges.last().unwrap().last,
        //     instance_count: self.instances,
        //     base_vertex: 0,
        //     base_instance: 0,
        //     draw_index: 0,
        //     bb_min: mesh.bounds.min.into(),
        //     bb_max: mesh.bounds.max.into(),
        //     ..Default::default()
        // };

        // buffer.unmap();
        // data.desc = desc;
        // self.mesh_desc = Some(Arc::new(buffer));
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

#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct CullData {
    pub planes: [FrustrumPlane; 6],
    pub desc: MeshDescriptor,
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
struct MeshDescriptor {
    pub vertex_count: u32,
    pub instance_count: u32,
    pub base_vertex: u32,
    pub base_instance: u32,

    pub bb_min: Vec3,
    pub draw_index: u32,
    pub bb_max: Vec3,
    pub _dummy: u32,
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct DrawCommand {
    pub vertex_count: u32,   // The number of vertices to draw.
    pub instance_count: u32, // The number of instances to draw.
    pub base_vertex: u32,    // The Index of the first vertex to draw.
    pub base_instance: u32,  // The instance ID of the first instance to draw.
}
