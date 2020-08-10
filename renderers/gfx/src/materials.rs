use std::mem::ManuallyDrop;

use crate::buffer::*;
use crate::hal;
use hal::device::Device;
use hal::format::{Aspects, Format, Swizzle};
use hal::image::{Kind, Level, SubresourceRange, Tiling, Usage, ViewCapabilities, ViewKind};
use hal::memory::Properties;
use hal::Backend;
use std::sync::Arc;

#[allow(dead_code)]
pub struct SceneTexture<B: Backend> {
    device: Arc<B::Device>,
    texture: Option<ManuallyDrop<B::Image>>,
    texture_view: Option<ManuallyDrop<B::ImageView>>,
    memory: Memory<B>,
}

impl<B: Backend> SceneTexture<B> {
    pub fn new(
        device: Arc<B::Device>,
        allocator: &Allocator<B>,
        width: u32,
        height: u32,
        mip_levels: u32,
        format: Format,
    ) -> Self {
        unsafe {
            let mut image = device
                .create_image(
                    Kind::D2(width, height, 1, 1),
                    mip_levels as Level,
                    format,
                    Tiling::Optimal,
                    Usage::SAMPLED,
                    ViewCapabilities::empty(),
                )
                .expect("Could not create texture array image");

            let req = device.get_image_requirements(&image);
            let memory = allocator.allocate_with_reqs(req, Properties::DEVICE_LOCAL);

            device
                .bind_image_memory(memory.borrow(), 0, &mut image)
                .expect("Could not bind image memory");
            let image_view = device
                .create_image_view(
                    &image,
                    ViewKind::D2,
                    format,
                    Swizzle::NO,
                    SubresourceRange {
                        aspects: Aspects::COLOR,
                        layers: 0..1,
                        levels: 0..(mip_levels as Level),
                    },
                )
                .expect("Could not create texture array view");

            Self {
                device,
                texture: Some(ManuallyDrop::new(image)),
                texture_view: Some(ManuallyDrop::new(image_view)),
                memory: memory,
            }
        }
    }

    pub fn borrow(&self) -> &B::Image {
        self.texture.as_ref().expect("Texture was none")
    }

    pub fn view(&self) -> &B::ImageView {
        self.texture_view.as_ref().expect("View was none")
    }
}

impl<'a, B: Backend> Drop for SceneTexture<B> {
    fn drop(&mut self) {
        unsafe {
            if let Some(view) = self.texture_view.take() {
                self.device
                    .destroy_image_view(ManuallyDrop::into_inner(view));
            }
            if let Some(texture) = self.texture.take() {
                self.device.destroy_image(ManuallyDrop::into_inner(texture));
            }
        }
    }
}
