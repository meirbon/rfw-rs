use crate::hal;
use hal::*;

use crate::hal::device::Device;
use crate::mem::{Allocator, Memory};
use crate::DeviceHandle;
use std::mem::ManuallyDrop;
use std::ops::Deref;

#[derive(Debug)]
pub struct Texture<B: hal::Backend> {
    device: DeviceHandle<B>,
    texture: ManuallyDrop<B::Image>,
    memory: Memory<B>,
    descriptor: TextureDescriptor,
}

#[derive(Debug)]
pub struct TextureView<B: hal::Backend> {
    device: DeviceHandle<B>,
    view: ManuallyDrop<B::ImageView>,
}

impl<B: hal::Backend> Deref for TextureView<B> {
    type Target = B::ImageView;

    fn deref(&self) -> &Self::Target {
        &*self.view
    }
}

#[derive(Debug)]
pub struct TextureDescriptor {
    pub kind: image::Kind,
    pub mip_levels: image::Level,
    pub format: format::Format,
    pub tiling: image::Tiling,
    pub usage: image::Usage,
    pub capabilities: image::ViewCapabilities,
}

#[derive(Debug)]
pub struct TextureViewDescriptor {
    pub view_kind: image::ViewKind,
    pub swizzle: format::Swizzle,
    pub range: image::SubresourceRange,
}

#[allow(dead_code)]
impl<B: hal::Backend> Texture<B> {
    pub fn new(
        device: DeviceHandle<B>,
        allocator: &Allocator<B>,
        descriptor: TextureDescriptor,
    ) -> Result<Self, image::CreationError> {
        unsafe {
            let mut texture = ManuallyDrop::new(device.create_image(
                descriptor.kind,
                descriptor.mip_levels,
                descriptor.format,
                descriptor.tiling,
                descriptor.usage,
                descriptor.capabilities,
            )?);

            let requirements = device.get_image_requirements(&*texture);
            // TODO: error handling
            let memory = allocator
                .allocate_with_reqs(requirements, memory::Properties::DEVICE_LOCAL, None)
                .unwrap();
            device
                .bind_image_memory(&*memory, 0, &mut *texture)
                .unwrap();

            Ok(Self {
                device,
                texture,
                memory,
                descriptor,
            })
        }
    }

    pub fn create_view(
        &self,
        descriptor: TextureViewDescriptor,
    ) -> Result<TextureView<B>, image::ViewCreationError> {
        let view = ManuallyDrop::new(unsafe {
            self.device.create_image_view(
                &*self.texture,
                descriptor.view_kind,
                self.descriptor.format,
                descriptor.swizzle,
                descriptor.range,
            )?
        });

        Ok(TextureView {
            device: self.device.clone(),
            view,
        })
    }

    pub fn format(&self) -> format::Format {
        self.descriptor.format
    }

    pub fn mip_levels(&self) -> image::Level {
        self.descriptor.mip_levels
    }

    pub fn extent(&self) -> image::Extent {
        self.descriptor.kind.extent()
    }

    pub fn kind(&self) -> image::Kind {
        self.descriptor.kind
    }

    pub fn image(&self) -> &B::Image {
        &*self.texture
    }
}

impl<B: hal::Backend> Deref for Texture<B> {
    type Target = B::Image;

    fn deref(&self) -> &Self::Target {
        &*self.texture
    }
}

impl<B: hal::Backend> Drop for Texture<B> {
    fn drop(&mut self) {
        unsafe {
            self.device
                .destroy_image(ManuallyDrop::into_inner(std::ptr::read(&self.texture)));
        }
    }
}

impl<B: hal::Backend> TextureView<B> {
    pub fn view(&self) -> &B::ImageView {
        &*self.view
    }
}

impl<B: hal::Backend> Drop for TextureView<B> {
    fn drop(&mut self) {
        unsafe {
            self.device
                .destroy_image_view(ManuallyDrop::into_inner(std::ptr::read(&self.view)));
        }
    }
}
