use crate::buffer::{Allocator, Buffer};
use crate::hal::device::Device;
use crate::hal::pso::DescriptorPool;
use crate::{hal, Queue};
use glam::*;
use hal::*;
use rfw_scene::TrackedStorage;
use rfw_utils::TaskPool;
use shared::BytesConversion;
use std::mem::ManuallyDrop;
use std::ptr;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct GfxSkin<B: hal::Backend> {
    pub buffer: Option<Buffer<B>>,
}

impl<B: hal::Backend> Clone for GfxSkin<B> {
    fn clone(&self) -> Self {
        Self { buffer: None }
    }
}

impl<B: hal::Backend> GfxSkin<B> {
    #[inline]
    pub fn buffer(&self) -> &B::Buffer {
        self.buffer.as_ref().unwrap().buffer()
    }
}

impl<B: hal::Backend> Default for GfxSkin<B> {
    fn default() -> Self {
        Self { buffer: None }
    }
}

#[derive(Debug)]
pub struct SkinList<B: hal::Backend> {
    device: Arc<B::Device>,
    allocator: Allocator<B>,
    queue: Arc<Mutex<Queue<B>>>,
    skins: TrackedStorage<GfxSkin<B>>,
    cmd_pool: ManuallyDrop<B::CommandPool>,
    desc_pool: ManuallyDrop<B::DescriptorPool>,
    pub desc_layout: ManuallyDrop<B::DescriptorSetLayout>,
    desc_sets: Vec<Option<B::DescriptorSet>>,
    task_pool: TaskPool<(usize, GfxSkin<B>)>,
}

impl<B: hal::Backend> SkinList<B> {
    pub fn new(
        device: Arc<B::Device>,
        allocator: Allocator<B>,
        queue: Arc<Mutex<Queue<B>>>,
    ) -> Self {
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

        let desc_layout = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_set_layout(
                    &[pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: pso::DescriptorType::Buffer {
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: false,
                            },
                            ty: pso::BufferDescriptorType::Uniform,
                        },
                        count: 1,
                        stage_flags: pso::ShaderStageFlags::VERTEX,
                        immutable_samplers: false,
                    }],
                    &[],
                )
            }
            .unwrap(),
        );

        let desc_pool = ManuallyDrop::new(
            unsafe {
                device.create_descriptor_pool(
                    256,
                    &[pso::DescriptorRangeDesc {
                        count: 256,
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Uniform,
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: false,
                            },
                        },
                    }],
                    pso::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET,
                )
            }
            .unwrap(),
        );

        Self {
            device,
            allocator,
            queue,
            skins: TrackedStorage::new(),
            cmd_pool,
            desc_pool,
            desc_layout,
            desc_sets: Vec::new(),
            task_pool: TaskPool::new(2),
        }
    }

    pub fn set_skin(&mut self, id: usize, skin: &rfw_scene::graph::Skin) {
        let allocator = self.allocator.clone();

        let skin = skin.clone();
        let gfx_skin = self.skins.take(id);
        self.task_pool.push(move |finish| {
            let gfx_skin = if let Some(gfx_skin) = gfx_skin {
                let mut buffer = if let Some(buffer) = gfx_skin.buffer {
                    if buffer.size_in_bytes
                        < skin.joint_matrices.len() * std::mem::size_of::<Mat4>()
                    {
                        allocator.allocate_buffer(
                            skin.joint_matrices.len() * std::mem::size_of::<Mat4>(),
                            buffer::Usage::UNIFORM,
                            memory::Properties::CPU_VISIBLE,
                        )
                    } else {
                        buffer
                    }
                } else {
                    allocator.allocate_buffer(
                        skin.joint_matrices.len() * std::mem::size_of::<Mat4>(),
                        buffer::Usage::UNIFORM,
                        memory::Properties::CPU_VISIBLE,
                    )
                };

                if let Ok(mapping) = buffer.map(memory::Segment::ALL) {
                    let slice = skin.joint_matrices.as_slice();
                    let bytes = slice.as_bytes();
                    mapping.as_slice()[0..bytes.len()].copy_from_slice(bytes);
                }

                GfxSkin {
                    buffer: Some(buffer),
                }
            } else {
                let mut buffer = allocator.allocate_buffer(
                    skin.joint_matrices.len() * std::mem::size_of::<Mat4>(),
                    buffer::Usage::UNIFORM,
                    memory::Properties::CPU_VISIBLE,
                );

                if let Ok(mapping) = buffer.map(memory::Segment::ALL) {
                    let slice = skin.joint_matrices.as_slice();
                    let bytes = slice.as_bytes();
                    mapping.as_slice()[0..bytes.len()].copy_from_slice(bytes);
                }

                GfxSkin {
                    buffer: Some(buffer),
                }
            };
            finish.send((id, gfx_skin))
        });
    }

    pub fn get_set(&self, id: usize) -> Option<&B::DescriptorSet> {
        if let Some(set) = self.desc_sets.get(id) {
            return match set.as_ref() {
                Some(set) => Some(set),
                None => None,
            };
        }
        None
    }

    pub fn synchronize(&mut self) {
        for (id, result) in self.task_pool.sync() {
            self.skins.overwrite(id, result);
        }

        self.desc_sets.resize_with(self.skins.len(), || None);
        let mut to_allocate = 0;

        for (id, _) in self.skins.iter_changed() {
            if self.desc_sets[id].is_none() {
                to_allocate += 1;
            }
        }

        let layouts: Vec<&B::DescriptorSetLayout> = vec![&self.desc_layout; to_allocate];

        let mut allocated_sets = Vec::with_capacity(to_allocate);
        let mut set_writes = Vec::with_capacity(to_allocate);
        unsafe { self.desc_pool.allocate(layouts, &mut allocated_sets) }.unwrap();

        for (set, (id, _)) in allocated_sets.into_iter().zip(self.skins.iter_changed()) {
            self.desc_sets[id] = Some(set);
        }

        for (id, skin) in self.skins.iter_changed() {
            set_writes.push(pso::DescriptorSetWrite {
                set: self.desc_sets[id].as_ref().unwrap(),
                binding: 0,
                array_offset: 0,
                descriptors: std::iter::once(pso::Descriptor::Buffer(
                    skin.buffer(),
                    buffer::SubRange::WHOLE,
                )),
            });
        }

        if !set_writes.is_empty() {
            unsafe {
                self.device.write_descriptor_sets(set_writes);
            }
        }

        self.skins.reset_changed();
    }
}

impl<B: hal::Backend> Drop for SkinList<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();

        unsafe {
            self.device
                .destroy_command_pool(ManuallyDrop::into_inner(ptr::read(&self.cmd_pool)));
            self.device
                .destroy_descriptor_pool(ManuallyDrop::into_inner(ptr::read(&self.desc_pool)));
            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.desc_layout,
                )));
        }
    }
}
