use crate::{hal, DeviceHandle};

use hal::device::OutOfMemory;
use hal::{adapter::PhysicalDevice, buffer, device::Device, memory, memory::Segment, MemoryTypeId};
use std::ops::Deref;
use std::{mem::ManuallyDrop, sync::Arc};

pub mod image;

#[derive(Debug)]
pub struct Allocator<B: hal::Backend> {
    device: DeviceHandle<B>,
    pub memory_props: Arc<hal::adapter::MemoryProperties>,
    pub limits: hal::Limits,
}

impl<B: hal::Backend> Clone for Allocator<B> {
    fn clone(&self) -> Self {
        Self {
            device: self.device.clone(),
            memory_props: self.memory_props.clone(),
            limits: self.limits,
        }
    }
}

#[allow(dead_code)]
impl<B: hal::Backend> Allocator<B> {
    pub fn new(device: DeviceHandle<B>, adapter: &hal::adapter::Adapter<B>) -> Self {
        let memory_props = adapter.physical_device.memory_properties();
        let limits = adapter.physical_device.limits();

        Self {
            device,
            memory_props: Arc::new(memory_props),
            limits,
        }
    }

    pub fn allocate(
        &self,
        bytes: usize,
        memory_props: memory::Properties,
        preferred: Option<memory::Properties>,
    ) -> Memory<B> {
        assert_ne!(bytes, 0);

        let upload_type = {
            if let Some(preferred) = preferred {
                let attempt = self
                    .memory_props
                    .memory_types
                    .iter()
                    .enumerate()
                    .position(|(_, memory_type)| memory_type.properties.contains(preferred));
                if attempt.is_none() {
                    self.memory_props
                        .memory_types
                        .iter()
                        .enumerate()
                        .position(|(_, memory_type)| memory_type.properties.contains(memory_props))
                        .unwrap()
                        .into()
                } else {
                    attempt.unwrap().into()
                }
            } else {
                self.memory_props
                    .memory_types
                    .iter()
                    .enumerate()
                    .position(|(_, memory_type)| memory_type.properties.contains(memory_props))
                    .unwrap()
                    .into()
            }
        };

        let memory = unsafe {
            ManuallyDrop::new(
                self.device
                    .allocate_memory(upload_type, bytes as u64)
                    .unwrap(),
            )
        };

        Memory {
            device: self.device.clone(),
            memory,
            memory_type: upload_type,
            capacity: bytes,
            memory_props,
        }
    }

    pub fn allocate_buffer(
        &self,
        bytes: usize,
        usage: buffer::Usage,
        memory_props: memory::Properties,
        preferred: Option<memory::Properties>,
    ) -> Result<Buffer<B>, AllocationError> {
        assert_ne!(bytes, 0);
        let buffer_len = bytes;

        let mut buffer = ManuallyDrop::new(
            match unsafe { self.device.create_buffer(buffer_len as u64, usage) } {
                Ok(buffer) => buffer,
                Err(e) => match e {
                    hal::buffer::CreationError::OutOfMemory(device) => {
                        return match device {
                            hal::device::OutOfMemory::Host => Err(AllocationError::OutOfHostMemory),
                            hal::device::OutOfMemory::Device => {
                                Err(AllocationError::OutOfDeviceMemory)
                            }
                        }
                    }
                    hal::buffer::CreationError::UnsupportedUsage { usage } => {
                        return Err(AllocationError::UnsupportedUsage(usage))
                    }
                },
            },
        );

        let buffer_req = unsafe { self.device.get_buffer_requirements(&buffer) };
        let upload_type = {
            if let Some(preferred) = preferred {
                let attempt = self
                    .memory_props
                    .memory_types
                    .iter()
                    .enumerate()
                    .position(|(_, memory_type)| memory_type.properties.contains(preferred));
                if attempt.is_none() {
                    self.memory_props
                        .memory_types
                        .iter()
                        .enumerate()
                        .position(|(_, memory_type)| memory_type.properties.contains(memory_props))
                        .unwrap()
                        .into()
                } else {
                    attempt.unwrap().into()
                }
            } else {
                self.memory_props
                    .memory_types
                    .iter()
                    .enumerate()
                    .position(|(_, memory_type)| memory_type.properties.contains(memory_props))
                    .unwrap()
                    .into()
            }
        };

        let buffer_memory = unsafe {
            let memory = match self.device.allocate_memory(upload_type, buffer_req.size) {
                Ok(mem) => mem,
                Err(e) => panic!("Could not allocate mem memory: {}", e),
            };
            match self.device.bind_buffer_memory(&memory, 0, &mut buffer) {
                Ok(_) => {}
                Err(e) => panic!("Could not bind mem memory: {}", e),
            };
            ManuallyDrop::new(memory)
        };

        Ok(Buffer {
            device: self.device.clone(),
            buffer,
            memory: Memory {
                device: self.device.clone(),
                memory: buffer_memory,
                memory_type: upload_type,
                capacity: buffer_req.size as usize,
                memory_props,
            },
            size_in_bytes: buffer_len as usize,
        })
    }

    pub fn allocate_with_reqs(
        &self,
        requirements: hal::memory::Requirements,
        memory_props: memory::Properties,
        preferred: Option<memory::Properties>,
    ) -> Result<Memory<B>, AllocationError> {
        let device_type = {
            if let Some(preferred) = preferred {
                let attempt = self.memory_props.memory_types.iter().enumerate().position(
                    |(id, memory_type)| {
                        requirements.type_mask & (1 << id) != 0
                            && memory_type.properties.contains(preferred)
                    },
                );
                if attempt.is_none() {
                    self.memory_props
                        .memory_types
                        .iter()
                        .enumerate()
                        .position(|(id, memory_type)| {
                            requirements.type_mask & (1 << id) != 0
                                && memory_type.properties.contains(memory_props)
                        })
                        .unwrap()
                        .into()
                } else {
                    attempt.unwrap().into()
                }
            } else {
                self.memory_props
                    .memory_types
                    .iter()
                    .enumerate()
                    .position(|(id, memory_type)| {
                        requirements.type_mask & (1 << id) != 0
                            && memory_type.properties.contains(memory_props)
                    })
                    .unwrap()
                    .into()
            }
        };

        let memory = match unsafe { self.device.allocate_memory(device_type, requirements.size) } {
            Ok(memory) => memory,
            Err(e) => match e {
                hal::device::AllocationError::OutOfMemory(device) => match device {
                    OutOfMemory::Host => return Err(AllocationError::OutOfHostMemory),
                    OutOfMemory::Device => return Err(AllocationError::OutOfBounds),
                },
                hal::device::AllocationError::TooManyObjects => {
                    return Err(AllocationError::TooManyObjects);
                }
            },
        };

        Ok(Memory {
            device: self.device.clone(),
            memory: ManuallyDrop::new(memory),
            memory_type: device_type,
            memory_props,
            capacity: requirements.size as usize,
        })
    }
}

#[derive(Debug)]
pub struct Buffer<B: hal::Backend> {
    pub device: DeviceHandle<B>,
    buffer: ManuallyDrop<B::Buffer>,
    memory: Memory<B>,
    pub size_in_bytes: usize,
}

impl<B: hal::Backend> Deref for Buffer<B> {
    type Target = B::Buffer;

    fn deref(&self) -> &Self::Target {
        &*self.buffer
    }
}

#[derive(Debug, Copy, Clone)]
pub enum AllocationError {
    NotMappable,
    OutOfHostMemory,
    OutOfDeviceMemory,
    OutOfBounds,
    MappingFailed,
    UnsupportedUsage(hal::buffer::Usage),
    TooManyObjects,
    BufferIsNotVisible,
}

impl std::fmt::Display for AllocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AllocationError::NotMappable => "Buffer is not mappable",
                AllocationError::OutOfHostMemory => "Out of host memory",
                AllocationError::OutOfDeviceMemory => "Out of device memory",
                AllocationError::OutOfBounds => "Out of bounds",
                AllocationError::MappingFailed => "Mapping failed",
                AllocationError::UnsupportedUsage(_) => "Usage is unsupported",
                AllocationError::TooManyObjects => "Too many objects",
                AllocationError::BufferIsNotVisible => "Buffer does not have CPU_VISIBLE flag set",
            }
        )
    }
}

impl std::error::Error for AllocationError {}

#[derive(Debug)]
pub struct Memory<B: hal::Backend> {
    device: DeviceHandle<B>,
    memory: ManuallyDrop<B::Memory>,
    memory_type: MemoryTypeId,
    memory_props: memory::Properties,
    capacity: usize,
}

impl<B: hal::Backend> Deref for Memory<B> {
    type Target = B::Memory;

    fn deref(&self) -> &Self::Target {
        &*self.memory
    }
}

#[allow(dead_code)]
impl<B: hal::Backend> Memory<B> {
    pub fn map(&mut self, segment: Segment) -> Result<Mapping<B>, AllocationError> {
        if !self.memory_props.contains(memory::Properties::CPU_VISIBLE) {
            return Err(AllocationError::NotMappable);
        }

        let ptr = unsafe {
            match self.device.map_memory(&self.memory, segment.clone()) {
                Ok(mapping) => mapping,
                Err(e) => match e {
                    hal::device::MapError::OutOfMemory(a) => match a {
                        hal::device::OutOfMemory::Host => {
                            return Err(AllocationError::OutOfHostMemory)
                        }
                        hal::device::OutOfMemory::Device => {
                            return Err(AllocationError::OutOfDeviceMemory);
                        }
                    },
                    hal::device::MapError::OutOfBounds => return Err(AllocationError::OutOfBounds),
                    hal::device::MapError::MappingFailed => {
                        return Err(AllocationError::MappingFailed)
                    }
                    hal::device::MapError::Access => {
                        return Err(AllocationError::BufferIsNotVisible)
                    }
                },
            }
        };

        let length = match segment.size {
            Some(size) => (size - segment.offset) as usize,
            None => self.capacity,
        };

        Ok(Mapping {
            device: &self.device,
            memory: &mut self.memory,
            ptr,
            length,
            segment,
        })
    }

    pub fn len(&self) -> usize {
        self.capacity
    }

    pub fn mem_type(&self) -> MemoryTypeId {
        self.memory_type
    }

    pub fn memory(&self) -> &B::Memory {
        &self.memory
    }

    pub fn as_ref(&self) -> &B::Memory {
        &self.memory
    }

    pub fn as_mut(&mut self) -> &mut B::Memory {
        &mut self.memory
    }
}

impl<B: hal::Backend> Drop for Memory<B> {
    fn drop(&mut self) {
        unsafe {
            self.device
                .free_memory(ManuallyDrop::into_inner(std::ptr::read(&self.memory)));
        }
    }
}

#[derive(Debug)]
pub struct Mapping<'a, B: hal::Backend> {
    device: &'a B::Device,
    memory: &'a mut B::Memory,
    ptr: *mut u8,
    length: usize,
    segment: Segment,
}

#[allow(dead_code)]
impl<'a, B: hal::Backend> Mapping<'a, B> {
    pub fn as_slice(&self) -> &'a mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.length) }
    }

    pub unsafe fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    pub fn unmap(self) {
        // We can skip all steps here, memory will be written and unmapped when this structs gets dropped
    }

    /// Returns length in bytes
    pub fn len(&self) -> usize {
        self.length
    }
}

impl<'a, B: hal::Backend> Drop for Mapping<'a, B> {
    fn drop(&mut self) {
        unsafe {
            let memory: &B::Memory = self.memory;
            self.device
                .flush_mapped_memory_ranges(std::iter::once((memory, self.segment.clone())))
                .unwrap();
            self.device.unmap_memory(self.memory);
        }
    }
}

#[allow(dead_code)]
impl<B: hal::Backend> Buffer<B> {
    pub fn map(&mut self, segment: Segment) -> Result<Mapping<B>, AllocationError> {
        let ptr = unsafe {
            match self
                .device
                .map_memory(self.memory.memory(), segment.clone())
            {
                Ok(mapping) => mapping,
                Err(e) => match e {
                    hal::device::MapError::OutOfMemory(a) => match a {
                        hal::device::OutOfMemory::Host => {
                            return Err(AllocationError::OutOfHostMemory)
                        }
                        hal::device::OutOfMemory::Device => {
                            return Err(AllocationError::OutOfDeviceMemory);
                        }
                    },
                    hal::device::MapError::OutOfBounds => return Err(AllocationError::OutOfBounds),
                    hal::device::MapError::MappingFailed => {
                        return Err(AllocationError::MappingFailed)
                    }
                    hal::device::MapError::Access => {
                        return Err(AllocationError::BufferIsNotVisible)
                    }
                },
            }
        };

        let length = match segment.size {
            Some(size) => (size - segment.offset) as usize,
            None => self.size_in_bytes,
        };

        Ok(Mapping {
            device: &self.device,
            memory: self.memory.as_mut(),
            ptr,
            length,
            segment,
        })
    }

    pub fn len(&self) -> usize {
        self.size_in_bytes
    }

    pub fn buffer(&self) -> &B::Buffer {
        &*self.buffer
    }

    pub fn memory(&self) -> &B::Memory {
        self.memory.as_ref()
    }
}

impl<B> Drop for Buffer<B>
where
    B: hal::Backend,
{
    fn drop(&mut self) {
        unsafe {
            self.device
                .destroy_buffer(ManuallyDrop::into_inner(std::ptr::read(&self.buffer)));
        }
    }
}
