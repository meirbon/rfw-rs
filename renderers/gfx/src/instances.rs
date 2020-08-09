use crate::{hal, Queue};
use glam::*;

use crate::hal::command::CommandBuffer;
use crate::hal::pool::CommandPool;
use crate::hal::{buffer, memory};
use crate::mesh::anim::GfxAnimMesh;
use crate::{buffer::*, mesh::GfxMesh};
use hal::{
    buffer::{SubRange, Usage},
    command::DescriptorSetOffset,
    device::Device,
    memory::{Properties, Segment},
    pso,
};
use pso::DescriptorPool;
use rfw_scene::{
    bvh::{Bounds, AABB},
    FlaggedStorage, ObjectRef, TrackedStorage, VertexData, VertexMesh,
};
use shared::BytesConversion;
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::{collections::HashSet, mem::ManuallyDrop, ops::Range, ptr, sync::Arc};

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Instance {
    pub matrix: Mat4,
    // 64
    pub inverse_matrix: Mat4,
    // 128
    pub normal_matrix: Mat4,
    // 192
    pub bounds: (Vec3A, Vec3A),
    pub original_bounds: (Vec3A, Vec3A),
}

#[derive(Debug, Clone)]
pub struct RenderInstance {
    pub id: u32,
    pub bounds: AABB,
    pub meshes: Vec<VertexMesh>,
}

impl Default for RenderInstance {
    fn default() -> Self {
        Self {
            id: std::u32::MAX,
            bounds: AABB::empty(),
            meshes: Vec::new(),
        }
    }
}

impl Instance {
    pub fn set_bounds(&mut self, bounds: AABB) {
        self.bounds = bounds.transformed(self.matrix.to_cols_array()).into();
        self.original_bounds = bounds.into();
    }
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            matrix: Mat4::identity(),
            inverse_matrix: Mat4::identity(),
            normal_matrix: Mat4::identity(),
            bounds: AABB::default().into(),
            original_bounds: AABB::default().into(),
        }
    }
}

#[derive(Debug)]
pub struct SceneList<B: hal::Backend> {
    device: Arc<B::Device>,
    allocator: Allocator<B>,
    queue: Arc<Mutex<Queue<B>>>,
    cmd_pool: ManuallyDrop<B::CommandPool>,
    meshes: TrackedStorage<GfxMesh<B>>,
    anim_meshes: TrackedStorage<GfxMesh<B>>,
    mesh_instances: TrackedStorage<HashSet<u32>>,
    anim_mesh_instances: TrackedStorage<HashSet<u32>>,
    instance_meshes: TrackedStorage<ObjectRef>,
    instances: TrackedStorage<Instance>,
    render_instances: FlaggedStorage<RenderInstance>,
    buffer: Buffer<B>,

    pub desc_pool: ManuallyDrop<B::DescriptorPool>,
    pub desc_set: B::DescriptorSet,
    pub set_layout: ManuallyDrop<B::DescriptorSetLayout>,
    task_pool: rfw_utils::TaskPool<(GfxMesh<B>, Option<Buffer<B>>)>,
}

#[allow(dead_code)]
impl<B: hal::Backend> SceneList<B> {
    const DEFAULT_CAPACITY: usize = 32;

    pub fn new(
        device: Arc<B::Device>,
        allocator: Allocator<B>,
        queue: Arc<Mutex<Queue<B>>>,
    ) -> Self {
        let buffer = allocator.allocate_buffer(
            std::mem::size_of::<Instance>() * Self::DEFAULT_CAPACITY,
            Usage::UNIFORM,
            Properties::CPU_VISIBLE,
        );

        let set_layout = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_set_layout(
                    &[pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Uniform,
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: true,
                            },
                        },
                        count: 1,
                        stage_flags: pso::ShaderStageFlags::VERTEX,
                        immutable_samplers: false,
                    }],
                    &[],
                )
            }
            .expect("Can't create descriptor set layout"),
        );

        let mut desc_pool = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_pool(
                    1, // sets
                    &[pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Uniform,
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: true,
                            },
                        },
                        count: 1,
                    }],
                    pso::DescriptorPoolCreateFlags::empty(),
                )
            }
            .expect("Can't create descriptor pool"),
        );
        let desc_set = unsafe { desc_pool.allocate_set(&set_layout) }.unwrap();

        let write = vec![pso::DescriptorSetWrite {
            set: &desc_set,
            binding: 0,
            array_offset: 0,
            descriptors: Some(pso::Descriptor::Buffer(buffer.borrow(), SubRange::WHOLE)),
        }];

        unsafe {
            device.write_descriptor_sets(write);
        }
        let cmd_pool = ManuallyDrop::new(unsafe {
            device
                .create_command_pool(
                    queue
                        .lock()
                        .expect("Could not lock queue")
                        .queue_group
                        .family,
                    hal::pool::CommandPoolCreateFlags::empty(),
                )
                .expect("Can't create command pool")
        });

        Self {
            device,
            allocator,
            queue,
            cmd_pool,
            meshes: TrackedStorage::new(),
            anim_meshes: TrackedStorage::new(),
            mesh_instances: TrackedStorage::new(),
            anim_mesh_instances: TrackedStorage::new(),
            instance_meshes: TrackedStorage::new(),
            instances: TrackedStorage::new(),
            render_instances: FlaggedStorage::new(),
            buffer,
            desc_pool,
            desc_set,
            set_layout,
            task_pool: rfw_utils::TaskPool::new(4),
        }
    }

    pub fn set_instance(&mut self, id: usize, instance: &rfw_scene::Instance) {
        self.instance_meshes.overwrite(id, instance.object_id);
        self.instances.overwrite(
            id,
            Instance {
                matrix: instance.get_transform(),
                inverse_matrix: instance.get_inverse_transform(),
                normal_matrix: instance.get_normal_transform(),
                bounds: instance.bounds().into(),
                original_bounds: instance.local_bounds().into(),
            },
        );
        self.render_instances.overwrite(id);
        self.render_instances[id] = RenderInstance {
            id: id as u32,
            meshes: Vec::new(),
            bounds: AABB::empty(),
        };

        match instance.object_id {
            ObjectRef::None => {}
            ObjectRef::Static(mesh_id) => match self.mesh_instances.get_mut(mesh_id as usize) {
                Some(set) => {
                    set.insert(id as u32);
                }
                None => {
                    let mut set = HashSet::new();
                    set.insert(id as u32);
                    self.mesh_instances.overwrite(id, set);
                }
            },
            ObjectRef::Animated(mesh_id) => {
                match self.anim_mesh_instances.get_mut(mesh_id as usize) {
                    Some(set) => {
                        set.insert(id as u32);
                    }
                    None => {
                        let mut set = HashSet::new();
                        set.insert(id as u32);
                        self.anim_mesh_instances.overwrite(id, set);
                    }
                }
            }
        }
    }

    pub fn set_mesh(&mut self, id: usize, mesh: &rfw_scene::Mesh) {
        let mesh = mesh.clone();
        let queue = self.queue.clone();
        let allocator = self.allocator.clone();
        let mut cmd_buffer = unsafe { self.cmd_pool.allocate_one(hal::command::Level::Primary) };

        self.task_pool.push(move |sender| {
            if mesh.vertices.is_empty() {
                sender.send((GfxMesh::default(), None));
                return;
            }

            let buffer_len = (mesh.vertices.len() * std::mem::size_of::<VertexData>()) as u64;
            assert_ne!(buffer_len, 0);

            // TODO: We should use staging buffers to transfer data to vertex buffers
            let mut buffer = allocator.allocate_buffer(
                buffer_len as usize,
                buffer::Usage::VERTEX | buffer::Usage::TRANSFER_DST,
                memory::Properties::DEVICE_LOCAL,
            );

            let mut staging_buffer = allocator.allocate_buffer(
                buffer_len as usize,
                buffer::Usage::TRANSFER_SRC,
                memory::Properties::CPU_VISIBLE,
            );

            if let Ok(mapping) = staging_buffer.map(memory::Segment {
                offset: 0,
                size: Some(buffer_len),
            }) {
                mapping.as_slice().copy_from_slice(mesh.vertices.as_bytes());
            }

            unsafe {
                cmd_buffer.begin_primary(hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);
                cmd_buffer.copy_buffer(
                    staging_buffer.borrow(),
                    buffer.borrow(),
                    std::iter::once(&hal::command::BufferCopy {
                        size: buffer_len as _,
                        src: 0,
                        dst: 0,
                    }),
                );
                cmd_buffer.finish();
                queue
                    .lock()
                    .unwrap()
                    .submit_without_semaphores(std::iter::once(&cmd_buffer), None);
            }

            sender.send((
                GfxMesh {
                    id,
                    sub_meshes: mesh.meshes.clone(),
                    buffer: Some(Arc::new(buffer)),
                    vertices: mesh.vertices.len(),
                    bounds: mesh.bounds,
                },
                Some(staging_buffer),
            ))
        });
    }

    pub fn synchronize(&mut self) {
        if !self.instances.any_changed() {
            return;
        }

        let copy_size = self.instances.len() * std::mem::size_of::<Instance>();

        if copy_size < self.buffer.size_in_bytes {
            self.buffer = self.allocator.allocate_buffer(
                self.instances.len() * 2 * std::mem::size_of::<Instance>(),
                Usage::UNIFORM,
                Properties::CPU_VISIBLE,
            );

            let write = vec![pso::DescriptorSetWrite {
                set: &self.desc_set,
                binding: 0,
                array_offset: 0,
                descriptors: Some(pso::Descriptor::Buffer(
                    self.buffer.borrow(),
                    SubRange::WHOLE,
                )),
            }];

            unsafe {
                self.device.write_descriptor_sets(write);
            }
        }

        if let Ok(mapping) = self.buffer.map(Segment::ALL) {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    self.instances.as_ptr() as *const u8,
                    mapping.as_ptr(),
                    copy_size,
                );
            }
        }

        self.queue.lock().unwrap().wait_idle().unwrap();
        let task_pool = &mut self.task_pool;
        let meshes = &mut self.meshes;
        let mesh_instances = &mut self.mesh_instances;
        let instances = &mut self.instances;
        let render_instances = &mut self.render_instances;
        let instance_meshes = &self.instance_meshes;

        task_pool.sync(|(new_mesh, _)| {
            let id = new_mesh.id as usize;
            meshes.overwrite(id, new_mesh);
            let mesh = &meshes[id];

            if let Some(set) = mesh_instances.get(mesh.id) {
                for instance_id in set.iter() {
                    let instance_id = *instance_id as usize;
                    instances.trigger_changed(instance_id);
                }
            }
        });

        instances.iter_changed_mut().for_each(|(i, inst)| {
            let (mut aabb, meshes) = match instance_meshes[i] {
                ObjectRef::None => {
                    let vec: Vec<VertexMesh> = Vec::new();
                    (AABB::empty(), vec)
                }
                ObjectRef::Static(m_id) => {
                    let mesh = &meshes[m_id as usize];
                    let sub_meshes: Vec<VertexMesh> = mesh
                        .sub_meshes
                        .iter()
                        .map(|s| VertexMesh {
                            bounds: s.bounds.transformed(inst.matrix.to_cols_array()),
                            first: s.first,
                            last: s.last,
                            mat_id: s.mat_id,
                        })
                        .collect::<Vec<_>>();

                    (mesh.bounds.clone(), sub_meshes)
                }
                ObjectRef::Animated(m_id) => {
                    let mesh = &meshes[m_id as usize];
                    let sub_meshes: Vec<VertexMesh> = mesh
                        .sub_meshes
                        .iter()
                        .map(|s| VertexMesh {
                            bounds: s.bounds.transformed(inst.matrix.to_cols_array()),
                            first: s.first,
                            last: s.last,
                            mat_id: s.mat_id,
                        })
                        .collect::<Vec<_>>();

                    (mesh.bounds.clone(), sub_meshes)
                }
            };

            aabb.transform(inst.matrix.to_cols_array());

            inst.set_bounds(aabb.clone());
            render_instances.overwrite(i);
            render_instances[i] = RenderInstance {
                id: i as u32,
                meshes,
                bounds: aabb,
            };
        });
    }

    pub fn iter_instances<T>(&self, mut render_instance: T)
    where
        T: FnMut(&B::Buffer, DescriptorSetOffset, &RenderInstance),
    {
        self.mesh_instances
            .iter()
            .filter(|(_, set)| !set.is_empty())
            .for_each(|(i, set)| {
                if let Some(mesh) = self.meshes.get(i) {
                    if let Some(buffer) = &self.meshes[i].buffer {
                        let buffer = buffer.borrow();
                        set.iter().for_each(|i| {
                            let i = *i as usize;
                            let offset =
                                (std::mem::size_of::<Instance>() * i) as DescriptorSetOffset;
                            render_instance(buffer, offset, &self.render_instances[i]);
                        });
                    }
                }
            });
    }
}

impl<B: hal::Backend> Drop for SceneList<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();

        unsafe {
            self.device
                .destroy_descriptor_pool(ManuallyDrop::into_inner(ptr::read(&self.desc_pool)));
            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.set_layout,
                )));

            self.device
                .destroy_command_pool(ManuallyDrop::into_inner(ptr::read(&self.cmd_pool)));
        }
    }
}
