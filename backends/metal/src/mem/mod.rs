use cocoa::foundation::NSRange;
use metal::{Buffer, BufferRef, DeviceRef, MTLResourceOptions};

pub struct ManagedBuffer<T> {
    buffer: Buffer,
    count: usize,
    _default: T,
}

impl<T: Sized> std::ops::Deref for ManagedBuffer<T> {
    type Target = BufferRef;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl<T: Sized + Default> ManagedBuffer<T> {
    pub fn new(device: &DeviceRef, count: usize) -> Self {
        let bytes = count * std::mem::size_of::<T>();
        let buffer = device.new_buffer(bytes as _, MTLResourceOptions::StorageModeManaged);

        Self {
            buffer,
            count,
            _default: T::default(),
        }
    }

    pub fn with_data(device: &DeviceRef, data: &[T]) -> Self {
        let bytes = data.len() * std::mem::size_of::<T>();
        let buffer = device.new_buffer_with_data(
            data.as_ptr() as *const _,
            bytes as _,
            MTLResourceOptions::StorageModeManaged,
        );

        Self {
            buffer,
            count: data.len(),
            _default: T::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn as_mut<CB>(&mut self, mut cb: CB)
    where
        CB: FnMut(&mut [T]),
    {
        cb(unsafe { std::slice::from_raw_parts_mut(self.buffer.contents() as *mut T, self.count) });
        self.buffer.did_modify_range(NSRange::new(
            0,
            (self.count * std::mem::size_of::<T>()) as _,
        ));
    }
}
