use ash::vk;
use std::fmt::{Debug, Formatter};
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub struct VkBuffer<T: Debug + Copy + Sized + Default> {
    pub(crate) allocator: Arc<ManuallyDrop<vk_mem::Allocator>>,
    pub(crate) buffer: vk::Buffer,
    pub(crate) allocation: vk_mem::Allocation,
    pub(crate) info: vk_mem::AllocationInfo,
    _dummy: T,
}

impl<T: Debug + Copy + Sized + Default> Debug for VkBuffer<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VkBuffer")
            .field("allocation", &self.allocation)
            .field("info", &self.info)
            .finish()
    }
}

pub struct Mapping<'a, T: Debug + Copy + Sized> {
    allocator: &'a vk_mem::Allocator,
    info: &'a mut vk_mem::Allocation,
    ptr: &'a mut [T],
}

impl<T: Debug + Copy + Sized> Mapping<'_, T> {
    pub fn as_slice(&mut self) -> &mut [T] {
        self.ptr
    }
}

impl<'a, T: Debug + Copy + Sized> Deref for Mapping<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.ptr
    }
}

impl<'a, T: Debug + Copy + Sized> DerefMut for Mapping<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.ptr
    }
}

impl<'a, T: Debug + Copy + Sized> Drop for Mapping<'a, T> {
    fn drop(&mut self) {
        self.allocator.unmap_memory(self.info).unwrap();
    }
}

impl<T: Debug + Copy + Sized + Default> VkBuffer<T> {
    pub fn new(
        allocator: Arc<ManuallyDrop<vk_mem::Allocator>>,
        usage_flags: vk::BufferUsageFlags,
        buffer_type: vk_mem::MemoryUsage,
        create_flags: vk_mem::AllocationCreateFlags,
        count: usize,
    ) -> Result<Self, vk_mem::Error> {
        let create_info = vk::BufferCreateInfo::builder()
            .usage(usage_flags)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .size((count * std::mem::size_of::<T>()) as _);

        let allocation_info = vk_mem::AllocationCreateInfo {
            usage: buffer_type,
            flags: create_flags,
            ..Default::default()
        };

        let (buffer, allocation, info) = allocator.create_buffer(&create_info, &allocation_info)?;

        Ok(Self {
            allocator,
            buffer,
            allocation,
            info,
            _dummy: Default::default(),
        })
    }

    pub fn map_memory(&mut self) -> Option<Mapping<T>> {
        unsafe {
            let ptr = self.allocator.map_memory(&self.allocation).unwrap();
            if ptr.is_null() {
                return None;
            }

            Some(Mapping {
                allocator: &self.allocator,
                info: &mut self.allocation,
                ptr: std::slice::from_raw_parts_mut(
                    ptr as *mut T,
                    self.info.get_size() / std::mem::size_of::<T>(),
                ),
            })
        }
    }

    pub fn len(&self) -> usize {
        self.info.get_size() / std::mem::size_of::<T>()
    }

    pub fn is_empty(&self) -> bool {
        self.info.get_size() == 0
    }

    pub fn get_buffer(&self) -> &vk::Buffer {
        &self.buffer
    }
}

impl<T: Debug + Copy + Sized + Default> Deref for VkBuffer<T> {
    type Target = vk::Buffer;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: Debug + Copy + Sized + Default> DerefMut for VkBuffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl<T: Debug + Copy + Sized + Default> Drop for VkBuffer<T> {
    fn drop(&mut self) {
        self.allocator
            .destroy_buffer(self.buffer, &self.allocation)
            .unwrap();
    }
}
