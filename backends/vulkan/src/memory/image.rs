use ash::*;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use vk_mem::*;

use super::Mapping;

pub struct VkImage {
    allocator: Arc<ManuallyDrop<vk_mem::Allocator>>,
    pub(crate) image: vk::Image,
    extent: vk::Extent3D,
    mip_levels: u32,
    array_layers: u32,
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
            extent: create_info.extent,
            mip_levels: create_info.mip_levels,
            array_layers: create_info.array_layers,
            allocation,
            info,
        })
    }

    pub fn extent(&self) -> vk::Extent3D {
        self.extent
    }

    pub fn mip_levels(&self) -> u32 {
        self.mip_levels
    }

    pub fn array_layers(&self) -> u32 {
        self.array_layers
    }

    /// # Safety
    ///
    /// Multiple mappings could result in undefined behaviour when one unmaps memory while another mapping is still being used.
    pub unsafe fn map_memory(&self) -> Option<Mapping<u8>> {
        let ptr = if let Ok(ptr) = self.allocator.map_memory(&self.allocation) {
            ptr
        } else {
            return None;
        };

        if ptr.is_null() {
            return None;
        }

        Some(Mapping {
            allocator: &self.allocator,
            info: &self.allocation,
            ptr: std::slice::from_raw_parts_mut(
                ptr as *mut u8,
                self.info.get_size() / std::mem::size_of::<u8>(),
            ),
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
