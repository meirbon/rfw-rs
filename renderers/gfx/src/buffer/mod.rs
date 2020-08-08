use crate::hal;

use hal::{adapter::PhysicalDevice, buffer, device::Device, memory, memory::Segment, MemoryTypeId};
use std::{mem::ManuallyDrop, sync::Arc};

#[derive(Debug)]
pub struct Allocator<B: hal::Backend> {
    device: Arc<B::Device>,
    pub memory_props: hal::adapter::MemoryProperties,
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
    pub fn new(device: Arc<B::Device>, adapter: &hal::adapter::Adapter<B>) -> Self {
        let memory_props = adapter.physical_device.memory_properties();
        let limits = adapter.physical_device.limits();

        Self {
            device,
            memory_props,
            limits,
        }
    }

    pub fn allocate(&self, bytes: usize, memory_props: memory::Properties) -> Memory<B> {
        assert_ne!(bytes, 0);
        let upload_type = self
            .memory_props
            .memory_types
            .iter()
            .enumerate()
            .position(|(_, mem_type)| {
                // type_mask is a bit field where each bit represents a memory type. If the bit is set
                // to 1 it means we can use that type for our buffer. So this code finds the first
                // memory type that has a `1` (or, is allowed), and is visible to the CPU.
                mem_type.properties.contains(memory_props)
            })
            .unwrap()
            .into();

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
    ) -> Buffer<B> {
        assert_ne!(bytes, 0);
        let buffer_len = bytes;

        let mut buffer = ManuallyDrop::new(
            unsafe { self.device.create_buffer(buffer_len as u64, usage) }.unwrap(),
        );

        let buffer_req = unsafe { self.device.get_buffer_requirements(&buffer) };

        let upload_type = self
            .memory_props
            .memory_types
            .iter()
            .enumerate()
            .position(|(id, mem_type)| {
                // type_mask is a bit field where each bit represents a memory type. If the bit is set
                // to 1 it means we can use that type for our buffer. So this code finds the first
                // memory type that has a `1` (or, is allowed), and is visible to the CPU.
                buffer_req.type_mask & (1 << id) != 0 && mem_type.properties.contains(memory_props)
            })
            .unwrap()
            .into();

        let buffer_memory = unsafe {
            let memory = self
                .device
                .allocate_memory(upload_type, buffer_req.size)
                .unwrap();
            self.device
                .bind_buffer_memory(&memory, 0, &mut buffer)
                .unwrap();
            ManuallyDrop::new(memory)
        };

        Buffer {
            device: self.device.clone(),
            buffer: Some(buffer),
            memory: Memory {
                device: self.device.clone(),
                memory: buffer_memory,
                memory_type: upload_type,
                capacity: buffer_req.size as usize,
                memory_props,
            },
            size_in_bytes: buffer_len as usize,
        }
    }

    pub fn allocate_with_reqs(
        &self,
        requirements: hal::memory::Requirements,
        memory_props: memory::Properties,
    ) -> Memory<B> {
        let device_type = self
            .memory_props
            .memory_types
            .iter()
            .enumerate()
            .position(|(id, memory_type)| {
                requirements.type_mask & (1 << id) != 0
                    && memory_type.properties.contains(memory_props)
            })
            .unwrap()
            .into();

        let memory = unsafe {
            self.device
                .allocate_memory(device_type, requirements.size)
                .unwrap()
        };

        Memory {
            device: self.device.clone(),
            memory: ManuallyDrop::new(memory),
            memory_type: device_type,
            memory_props,
            capacity: requirements.size as usize,
        }
    }
}

#[derive(Debug)]
pub struct Buffer<B: hal::Backend> {
    pub device: Arc<B::Device>,
    buffer: Option<ManuallyDrop<B::Buffer>>,
    memory: Memory<B>,
    pub size_in_bytes: usize,
}

pub enum BufferError {
    NotMappable,
    OutOfHostMemory,
    OutOfDeviceMemory,
    OutOfBounds,
    MappingFailed,
}

#[derive(Debug)]
pub struct Memory<B: hal::Backend> {
    device: Arc<B::Device>,
    memory: ManuallyDrop<B::Memory>,
    memory_type: MemoryTypeId,
    memory_props: memory::Properties,
    capacity: usize,
}

#[allow(dead_code)]
impl<B: hal::Backend> Memory<B> {
    pub fn map(&mut self, segment: Segment) -> Result<Mapping<B>, BufferError> {
        if !self.memory_props.contains(memory::Properties::CPU_VISIBLE) {
            return Err(BufferError::NotMappable);
        }

        let ptr = unsafe {
            match self.device.map_memory(&self.memory, segment.clone()) {
                Ok(mapping) => mapping,
                Err(e) => match e {
                    hal::device::MapError::OutOfMemory(a) => match a {
                        hal::device::OutOfMemory::Host => return Err(BufferError::OutOfHostMemory),
                        hal::device::OutOfMemory::Device => {
                            return Err(BufferError::OutOfDeviceMemory);
                        }
                    },
                    hal::device::MapError::OutOfBounds => return Err(BufferError::OutOfBounds),
                    hal::device::MapError::MappingFailed => return Err(BufferError::MappingFailed),
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

    pub fn borrow(&self) -> &B::Memory {
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
    pub fn map(&mut self, segment: Segment) -> Result<Mapping<B>, BufferError> {
        let ptr = unsafe {
            match self
                .device
                .map_memory(self.memory.borrow(), segment.clone())
            {
                Ok(mapping) => mapping,
                Err(e) => match e {
                    hal::device::MapError::OutOfMemory(a) => match a {
                        hal::device::OutOfMemory::Host => return Err(BufferError::OutOfHostMemory),
                        hal::device::OutOfMemory::Device => {
                            return Err(BufferError::OutOfDeviceMemory);
                        }
                    },
                    hal::device::MapError::OutOfBounds => return Err(BufferError::OutOfBounds),
                    hal::device::MapError::MappingFailed => return Err(BufferError::MappingFailed),
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

    pub fn borrow(&self) -> &B::Buffer {
        self.buffer.as_ref().unwrap()
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
            let mut buf = None;
            std::mem::swap(&mut buf, &mut self.buffer);
            if let Some(buffer) = buf {
                self.device.destroy_buffer(ManuallyDrop::into_inner(buffer));
            }
        }
    }
}
