use crate::{hal, Queue};
use glam::*;

use crate::hal::command::CommandBuffer;
use crate::hal::pool::CommandPool;
use crate::hal::{buffer, memory};
use crate::mesh::anim::GfxAnimMesh;
use crate::{buffer::*, mesh::GfxMesh};
use hal::{
    buffer::{SubRange, Usage},
    device::Device,
    memory::{Properties, Segment},
    pso,
};
use pso::DescriptorPool;
use rfw_scene::{
    bvh::{Bounds, AABB},
    AnimVertexData, FlaggedStorage, ObjectRef, TrackedStorage, VertexData, VertexMesh,
};
use shared::BytesConversion;
use std::sync::Mutex;
use std::{collections::HashSet, mem::ManuallyDrop, ptr, sync::Arc};

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
    pub object: ObjectRef,
    pub skin_id: Option<u32>,
    pub bounds: AABB,
    pub meshes: Vec<VertexMesh>,
}

#[derive(Debug, Clone)]
pub enum RenderBuffers<'a, B: hal::Backend> {
    Static(&'a Buffer<B>),
    /// Buffer with both vertices and animation data
    /// the usize is the offset into the buffer for animation data
    Animated(&'a Buffer<B>, usize),
}

impl Default for RenderInstance {
    fn default() -> Self {
        Self {
            id: std::u32::MAX,
            object: ObjectRef::None,
            skin_id: None,
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

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TaskResult<B: hal::Backend> {
    Mesh(GfxMesh<B>, Option<Arc<Buffer<B>>>, B::CommandBuffer),
    AnimMesh(GfxAnimMesh<B>, Option<Arc<Buffer<B>>>, B::CommandBuffer),
}

#[derive(Debug)]
pub struct SceneList<B: hal::Backend> {
    device: Arc<B::Device>,
    allocator: Allocator<B>,
    queue: Arc<Mutex<Queue<B>>>,
    cmd_pool: ManuallyDrop<B::CommandPool>,
    meshes: FlaggedStorage<GfxMesh<B>>,
    anim_meshes: FlaggedStorage<GfxAnimMesh<B>>,
    mesh_instances: FlaggedStorage<HashSet<u32>>,
    anim_mesh_instances: FlaggedStorage<HashSet<u32>>,
    instances: TrackedStorage<Instance>,
    render_instances: FlaggedStorage<RenderInstance>,
    instance_buffer: Buffer<B>,

    pub desc_pool: ManuallyDrop<B::DescriptorPool>,
    pub desc_set: B::DescriptorSet,
    pub set_layout: ManuallyDrop<B::DescriptorSetLayout>,
    task_pool: rfw_utils::TaskPool<TaskResult<B>>,
}

#[allow(dead_code)]
impl<B: hal::Backend> SceneList<B> {
    const DEFAULT_CAPACITY: usize = 32;

    pub fn new(
        device: Arc<B::Device>,
        allocator: Allocator<B>,
        queue: Arc<Mutex<Queue<B>>>,
    ) -> Self {
        let instance_buffer = allocator
            .allocate_buffer(
                std::mem::size_of::<Instance>() * Self::DEFAULT_CAPACITY,
                Usage::STORAGE | Usage::TRANSFER_DST,
                Properties::CPU_VISIBLE,
            )
            .unwrap();

        let set_layout = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_set_layout(
                    &[pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Storage { read_only: true },
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: false,
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
                            ty: pso::BufferDescriptorType::Storage { read_only: true },
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: false,
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
            descriptors: Some(pso::Descriptor::Buffer(
                instance_buffer.buffer(),
                SubRange::WHOLE,
            )),
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
            meshes: FlaggedStorage::new(),
            anim_meshes: FlaggedStorage::new(),
            mesh_instances: FlaggedStorage::new(),
            anim_mesh_instances: FlaggedStorage::new(),
            instances: TrackedStorage::new(),
            render_instances: FlaggedStorage::new(),
            instance_buffer,
            desc_pool,
            desc_set,
            set_layout,
            task_pool: rfw_utils::TaskPool::new(4),
        }
    }

    pub fn set_instance(&mut self, id: usize, instance: &rfw_scene::Instance) {
        // Remove old instance in mesh instance list
        if let Some(inst) = self.render_instances.get(id) {
            match inst.object {
                ObjectRef::None => {}
                ObjectRef::Static(mesh_id) => {
                    let mesh_id = mesh_id as usize;
                    self.mesh_instances[mesh_id].remove(&(id as u32));
                }
                ObjectRef::Animated(mesh_id) => {
                    let mesh_id = mesh_id as usize;
                    self.anim_mesh_instances[mesh_id].remove(&(id as u32));
                }
            }
        }

        // Add instance to object list
        match instance.object_id {
            ObjectRef::None => {}
            ObjectRef::Static(mesh_id) => match self.mesh_instances.get_mut(mesh_id as usize) {
                Some(set) => {
                    set.insert(id as u32);
                }
                None => {
                    let mut set = HashSet::new();
                    set.insert(id as u32);
                    self.mesh_instances.overwrite_val(mesh_id as usize, set);
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
                        self.anim_mesh_instances
                            .overwrite_val(mesh_id as usize, set);
                    }
                }
            }
        }

        // Update instance data
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

        self.render_instances.overwrite_val(
            id,
            RenderInstance {
                id: id as u32,
                object: instance.object_id,
                skin_id: instance.skin_id,
                meshes: Vec::new(),
                bounds: AABB::empty(),
            },
        );
    }

    pub fn set_mesh(&mut self, id: usize, mesh: &rfw_scene::Mesh) {
        if mesh.vertices.is_empty() {
            self.meshes.overwrite_val(id, GfxMesh::default_id(id));
            return;
        }

        let mesh = mesh.clone();
        let queue = self.queue.clone();
        let allocator = self.allocator.clone();
        let mut cmd_buffer = unsafe { self.cmd_pool.allocate_one(hal::command::Level::Primary) };

        self.task_pool.push(move |sender| {
            let buffer_len = (mesh.vertices.len() * std::mem::size_of::<VertexData>()) as u64;
            assert_ne!(buffer_len, 0);

            let buffer = allocator
                .allocate_buffer(
                    buffer_len as usize,
                    buffer::Usage::VERTEX | buffer::Usage::TRANSFER_DST,
                    memory::Properties::DEVICE_LOCAL,
                )
                .unwrap();

            let mut staging_buffer = allocator
                .allocate_buffer(
                    buffer_len as usize,
                    buffer::Usage::TRANSFER_SRC,
                    memory::Properties::CPU_VISIBLE,
                )
                .unwrap();

            if let Ok(mapping) = staging_buffer.map(memory::Segment {
                offset: 0,
                size: Some(buffer_len),
            }) {
                mapping.as_slice().copy_from_slice(mesh.vertices.as_bytes());
            }

            unsafe {
                cmd_buffer.begin_primary(hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);
                cmd_buffer.copy_buffer(
                    staging_buffer.buffer(),
                    buffer.buffer(),
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

            sender.send(TaskResult::Mesh(
                GfxMesh {
                    id,
                    sub_meshes: mesh.meshes.clone(),
                    buffer: Some(Arc::new(buffer)),
                    vertices: mesh.vertices.len(),
                    bounds: mesh.bounds,
                },
                Some(Arc::new(staging_buffer)),
                cmd_buffer,
            ))
        });
    }

    pub fn set_anim_mesh(&mut self, id: usize, anim_mesh: &rfw_scene::AnimatedMesh) {
        if anim_mesh.vertices.is_empty() {
            self.anim_meshes
                .overwrite_val(id, GfxAnimMesh::default_id(id));
            return;
        }

        let anim_mesh = anim_mesh.clone();
        let queue = self.queue.clone();
        let allocator = self.allocator.clone();
        let mut cmd_buffer = unsafe { self.cmd_pool.allocate_one(hal::command::Level::Primary) };

        self.task_pool.push(move |sender| {
            let buffer_len = (anim_mesh.vertices.len() * std::mem::size_of::<VertexData>()) as u64;
            let anim_buffer_len =
                (anim_mesh.anim_vertex_data.len() * std::mem::size_of::<AnimVertexData>()) as u64;
            assert_ne!(buffer_len, 0);
            assert_ne!(anim_buffer_len, 0);

            let buffer = allocator
                .allocate_buffer(
                    buffer_len as usize + anim_buffer_len as usize,
                    buffer::Usage::VERTEX | buffer::Usage::TRANSFER_DST,
                    memory::Properties::DEVICE_LOCAL,
                )
                .unwrap();

            let mut staging_buffer = allocator
                .allocate_buffer(
                    (buffer_len + anim_buffer_len) as usize,
                    buffer::Usage::TRANSFER_SRC,
                    memory::Properties::CPU_VISIBLE,
                )
                .unwrap();

            if let Ok(mapping) = staging_buffer.map(memory::Segment {
                offset: 0,
                size: Some(buffer_len + anim_buffer_len),
            }) {
                let buffer_len = buffer_len as usize;
                let anim_buffer_len = anim_buffer_len as usize;

                mapping.as_slice()[0..buffer_len].copy_from_slice(anim_mesh.vertices.as_bytes());
                mapping.as_slice()[buffer_len..(buffer_len + anim_buffer_len)]
                    .copy_from_slice(anim_mesh.anim_vertex_data.as_bytes());
            }

            unsafe {
                cmd_buffer.begin_primary(hal::command::CommandBufferFlags::ONE_TIME_SUBMIT);
                cmd_buffer.copy_buffer(
                    staging_buffer.buffer(),
                    buffer.buffer(),
                    std::iter::once(&hal::command::BufferCopy {
                        size: (buffer_len + anim_buffer_len) as _,
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

            sender.send(TaskResult::AnimMesh(
                GfxAnimMesh {
                    id,
                    sub_meshes: anim_mesh.meshes.clone(),
                    buffer: Some(Arc::new(buffer)),
                    anim_offset: buffer_len as usize,
                    vertices: anim_mesh.vertices.len(),
                    bounds: anim_mesh.bounds,
                },
                Some(Arc::new(staging_buffer)),
                cmd_buffer,
            ))
        });
    }

    pub fn synchronize(&mut self) {
        let mut to_free = Vec::new();
        if self.task_pool.has_jobs() {
            if let Ok(mut queue) = self.queue.lock() {
                match queue.wait_idle() {
                    Ok(_) => {}
                    Err(e) => eprintln!("error waiting for transfer queue: {}", e),
                }
            }
        }

        for result in self.task_pool.sync() {
            match result {
                TaskResult::Mesh(new_mesh, _, cmd_buffer) => {
                    let id = new_mesh.id as usize;
                    self.meshes.overwrite_val(id, new_mesh);
                    let mesh = &self.meshes[id];

                    if let Some(set) = self.mesh_instances.get(mesh.id) {
                        for instance_id in set.iter() {
                            let instance_id = *instance_id as usize;
                            self.instances.trigger_changed(instance_id);
                        }
                    }
                    to_free.push(cmd_buffer);
                }
                TaskResult::AnimMesh(new_mesh, _, cmd_buffer) => {
                    let id = new_mesh.id as usize;
                    self.anim_meshes.overwrite_val(id, new_mesh);
                    let mesh = &self.anim_meshes[id];

                    if let Some(set) = self.anim_mesh_instances.get(mesh.id) {
                        for instance_id in set.iter() {
                            let instance_id = *instance_id as usize;
                            self.instances.trigger_changed(instance_id);
                        }
                    }
                    to_free.push(cmd_buffer);
                }
            }
        }

        if !to_free.is_empty() {
            unsafe {
                self.cmd_pool.free(to_free);
            }
        }

        if !self.instances.any_changed() {
            return;
        }

        let instances = &mut self.instances;

        for (i, inst) in instances.iter_mut() {
            let matrix = inst.matrix.to_cols_array();
            let (mut aabb, meshes) = match self.render_instances[i].object {
                ObjectRef::None => {
                    let vec: Vec<VertexMesh> = Vec::new();
                    (AABB::empty(), vec)
                }
                ObjectRef::Static(m_id) => {
                    if let Some(mesh) = self.meshes.get(m_id as usize) {
                        let sub_meshes: Vec<VertexMesh> = mesh
                            .sub_meshes
                            .iter()
                            .map(|s| VertexMesh {
                                bounds: s.bounds.transformed(matrix),
                                first: s.first,
                                last: s.last,
                                mat_id: s.mat_id,
                            })
                            .collect::<Vec<_>>();

                        (mesh.bounds.clone(), sub_meshes)
                    } else {
                        (AABB::empty(), Vec::new())
                    }
                }
                ObjectRef::Animated(m_id) => {
                    if let Some(anim_mesh) = self.anim_meshes.get(m_id as usize) {
                        let sub_meshes: Vec<VertexMesh> = anim_mesh
                            .sub_meshes
                            .iter()
                            .map(|s| VertexMesh {
                                bounds: s.bounds.transformed(matrix),
                                first: s.first,
                                last: s.last,
                                mat_id: s.mat_id,
                            })
                            .collect::<Vec<_>>();

                        (anim_mesh.bounds.clone(), sub_meshes)
                    } else {
                        (AABB::empty(), Vec::new())
                    }
                }
            };

            aabb.transform(matrix);

            inst.set_bounds(aabb.clone());
            self.render_instances[i].meshes = meshes;
            self.render_instances[i].bounds = aabb;
        }

        let copy_size = self.instances.len() * std::mem::size_of::<Instance>();
        if copy_size > self.instance_buffer.size_in_bytes {
            self.instance_buffer = self
                .allocator
                .allocate_buffer(
                    self.instances.len() * 2 * std::mem::size_of::<Instance>(),
                    Usage::STORAGE,
                    Properties::CPU_VISIBLE,
                )
                .unwrap();

            let write = vec![pso::DescriptorSetWrite {
                set: &self.desc_set,
                binding: 0,
                array_offset: 0,
                descriptors: Some(pso::Descriptor::Buffer(
                    self.instance_buffer.buffer(),
                    SubRange::WHOLE,
                )),
            }];

            unsafe {
                self.device.write_descriptor_sets(write);
            }
        }

        if let Ok(mapping) = self.instance_buffer.map(Segment::ALL) {
            let instances = unsafe { self.instances.as_slice() };
            let src = instances.as_bytes();
            let length = src.len();
            let slice = mapping.as_slice();
            slice[0..length].copy_from_slice(src);
        }

        self.instances.reset_changed();
    }

    pub fn iter_instances<T>(&self, mut render_instance: T)
    where
        T: FnMut(&RenderBuffers<'_, B>, &RenderInstance),
    {
        self.mesh_instances
            .iter()
            .filter(|(_, set)| !set.is_empty())
            .for_each(|(i, set)| {
                let buffer = if let Some(mesh) = self.meshes.get(i) {
                    match mesh.buffer.as_ref() {
                        Some(buffer) => Some(RenderBuffers::Static(buffer)),
                        None => None,
                    }
                } else {
                    None
                };

                if let Some(buffer) = buffer {
                    set.iter().for_each(|inst| {
                        let inst = *inst as usize;
                        render_instance(&buffer, &self.render_instances[inst]);
                    });
                }
            });

        self.anim_mesh_instances
            .iter()
            .filter(|(_, set)| !set.is_empty())
            .for_each(|(i, set)| {
                let buffer = if let Some(mesh) = self.anim_meshes.get(i) {
                    match mesh.buffer.as_ref() {
                        Some(buffer) => Some(RenderBuffers::Animated(buffer, mesh.anim_offset)),
                        _ => None,
                    }
                } else {
                    None
                };

                if let Some(buffer) = buffer {
                    set.iter().for_each(|inst| {
                        let inst = *inst as usize;
                        render_instance(&buffer, &self.render_instances[inst]);
                    });
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
