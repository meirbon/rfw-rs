use crate::hal;
use crate::hal::device::Device;
use hal::*;
use shared::BytesConversion;
use std::mem::ManuallyDrop;

pub struct Module<'a, B: hal::Backend> {
    device: &'a B::Device,
    pub raw: ManuallyDrop<B::ShaderModule>,
}

impl<'a, B: hal::Backend> Module<'a, B> {
    pub fn new<T: Sized>(device: &B::Device, spirv: &[T]) -> Result<Self, device::ShaderError> {
        let bytes = spirv.as_quad_bytes();

        unsafe {
            let module = device.create_shader_module(bytes)?;

            Ok(Self {
                device,
                raw: ManuallyDrop::new(module),
            })
        }
    }

    pub fn as_ref(&self) -> &B::ShaderModule {
        &*self.raw
    }
}

impl<'a, B: hal::Backend> Drop for Module<'a, B> {
    fn drop(&mut self) {
        unsafe {
            self.device
                .destroy_shader_module(ManuallyDrop::into_inner(std::ptr::read(&self.module)));
        }
    }
}
