use crate::hal::device::Device;
use crate::hal::pso::DescriptorPool;
use crate::mem::{Allocator, Buffer};
use crate::{hal, DeviceHandle, Queue};
use hal::*;
use rfw::prelude::*;
use std::mem::ManuallyDrop;
use std::ptr;

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
    device: DeviceHandle<B>,
    allocator: Allocator<B>,
    queue: Queue<B>,
    skins: TrackedStorage<GfxSkin<B>>,
    desc_pool: ManuallyDrop<B::DescriptorPool>,
    pub desc_layout: ManuallyDrop<B::DescriptorSetLayout>,
    desc_sets: Vec<Option<B::DescriptorSet>>,
    task_pool: ManagedTaskPool<(usize, GfxSkin<B>)>,
}

impl<B: hal::Backend> SkinList<B> {
    pub fn new(
        device: DeviceHandle<B>,
        allocator: Allocator<B>,
        queue: Queue<B>,
        task_pool: &TaskPool,
    ) -> Self {
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
            desc_pool,
            desc_layout,
            desc_sets: Vec::new(),
            task_pool: ManagedTaskPool::from(task_pool),
        }
    }

    pub fn set_skin(&mut self, id: usize, skin: &Skin) {
        let allocator = self.allocator.clone();

        let skin = skin.clone();
        let gfx_skin = self.skins.take(id);
        self.task_pool.push(move |finish| {
            let gfx_skin = if let Some(gfx_skin) = gfx_skin {
                let mut buffer = if let Some(buffer) = gfx_skin.buffer {
                    if buffer.size_in_bytes
                        < skin.joint_matrices.len() * std::mem::size_of::<Mat4>()
                    {
                        allocator
                            .allocate_buffer(
                                skin.joint_matrices.len() * std::mem::size_of::<Mat4>(),
                                buffer::Usage::UNIFORM,
                                memory::Properties::CPU_VISIBLE,
                                Some(
                                    memory::Properties::CPU_VISIBLE
                                        | memory::Properties::DEVICE_LOCAL,
                                ),
                            )
                            .unwrap()
                    } else {
                        buffer
                    }
                } else {
                    allocator
                        .allocate_buffer(
                            skin.joint_matrices.len() * std::mem::size_of::<Mat4>(),
                            buffer::Usage::UNIFORM,
                            memory::Properties::CPU_VISIBLE,
                            Some(
                                memory::Properties::CPU_VISIBLE | memory::Properties::DEVICE_LOCAL,
                            ),
                        )
                        .unwrap()
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
                let mut buffer = allocator
                    .allocate_buffer(
                        skin.joint_matrices.len() * std::mem::size_of::<Mat4>(),
                        buffer::Usage::UNIFORM,
                        memory::Properties::CPU_VISIBLE,
                        None,
                    )
                    .unwrap();

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

    pub fn synchronize(&mut self) {
        for result in self.task_pool.sync() {
            if let Some((id, result)) = result {
                self.skins.overwrite(id, result);
            }
        }

        self.desc_sets.resize_with(self.skins.len(), || None);

        let mut set_writes = Vec::with_capacity(self.skins.len());
        for (id, _) in self.skins.iter_changed() {
            if self.desc_sets[id].is_none() {
                self.desc_sets[id] =
                    Some(unsafe { self.desc_pool.allocate_set(&self.desc_layout).unwrap() });
            }
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
                .destroy_descriptor_pool(ManuallyDrop::into_inner(ptr::read(&self.desc_pool)));
            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(ptr::read(
                    &self.desc_layout,
                )));
        }
    }
}
