use std::fmt::Debug;

use ash::*;
use std::sync::Arc;

mod buffer;
mod image;

pub use buffer::*;
pub use image::*;
use std::mem::ManuallyDrop;

pub struct VkAllocator {
    allocator: Arc<ManuallyDrop<vk_mem::Allocator>>,
}

impl VkAllocator {
    pub fn new(create_info: &vk_mem::AllocatorCreateInfo) -> Result<Self, vk_mem::Error> {
        let allocator = vk_mem::Allocator::new(create_info)?;
        Ok(Self {
            allocator: Arc::new(ManuallyDrop::new(allocator)),
        })
    }

    pub fn create_buffer<T: Debug + Copy + Sized + Default>(
        &self,
        usage_flags: vk::BufferUsageFlags,
        buffer_type: vk_mem::MemoryUsage,
        create_flags: vk_mem::AllocationCreateFlags,
        count: usize,
    ) -> Result<VkBuffer<T>, vk_mem::Error> {
        VkBuffer::new(
            self.allocator.clone(),
            usage_flags,
            buffer_type,
            create_flags,
            count,
        )
    }

    pub fn create_image(
        &self,
        create_info: &vk::ImageCreateInfo,
        buffer_type: vk_mem::MemoryUsage,
        create_flags: vk_mem::AllocationCreateFlags,
    ) -> Result<VkImage, vk_mem::Error> {
        VkImage::new(
            self.allocator.clone(),
            create_info,
            buffer_type,
            create_flags,
        )
    }

    pub fn destroy(mut self) {
        drop(self);
    }
}

impl Drop for VkAllocator {
    fn drop(&mut self) {
        unsafe {
            let mut allocator = std::ptr::read(&**self.allocator);
            allocator.destroy();
        }
    }
}
