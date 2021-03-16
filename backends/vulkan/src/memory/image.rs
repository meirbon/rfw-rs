use ash::*;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use vk_mem::*;

pub struct VkImage {
    allocator: Arc<ManuallyDrop<vk_mem::Allocator>>,
    image: vk::Image,
    allocation: vk_mem::Allocation,
    info: vk_mem::AllocationInfo,
}

impl VkImage {
    pub fn new(
        allocator: Arc<ManuallyDrop<vk_mem::Allocator>>,
        create_info: &vk::ImageCreateInfo,
        usage: MemoryUsage,
        create_flags: AllocationCreateFlags,
    ) -> Result<Self> {
        let (image, allocation, info) = allocator.create_image(
            create_info,
            &AllocationCreateInfo {
                usage,
                flags: create_flags,
                ..Default::default()
            },
        )?;

        Ok(Self {
            allocator,
            image,
            allocation,
            info,
        })
    }
}

impl Deref for VkImage {
    type Target = vk::Image;

    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

impl DerefMut for VkImage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.image
    }
}

impl Drop for VkImage {
    fn drop(&mut self) {
        self.allocator
            .destroy_image(self.image, &self.allocation)
            .unwrap();
    }
}
