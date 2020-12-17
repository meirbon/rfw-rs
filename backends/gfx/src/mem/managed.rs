use super::*;
use crate::hal;

use rfw::prelude::*;
use std::ops::{Index, IndexMut};

pub struct ManagedBuffer<T: Sized + Default + Clone, B: hal::Backend> {
    buffer: Buffer<B>,
    host_data: Vec<T>,
    dirty: bool,
}

#[allow(dead_code)]
impl<T: Sized + Default + Clone, B: hal::Backend> ManagedBuffer<T, B> {
    pub fn new(buffer: Buffer<B>) -> Self {
        let size = std::mem::size_of::<T>();
        assert_eq!(buffer.size_in_bytes % size, 0);
        assert!(buffer.size_in_bytes > 0);
        assert!(
            buffer
                .memory
                .memory_props
                .contains(memory::Properties::CPU_VISIBLE),
            "Managed buffers currently only support CPU visible memory"
        );

        let count = buffer.size_in_bytes / size;
        let host_data = vec![T::default(); count];

        Self {
            buffer,
            host_data,
            dirty: false,
        }
    }

    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }

        if let Ok(mapping) = self.buffer.map(memory::Segment::ALL) {
            let slice = mapping.as_slice();
            let bytes: &[u8] = self.host_data.as_bytes();

            slice[0..bytes.len()].copy_from_slice(bytes);
        }

        self.dirty = false;
    }

    pub fn len(&self) -> usize {
        self.host_data.len()
    }

    pub fn size_in_bytes(&self) -> usize {
        self.host_data.len() * std::mem::size_of::<T>()
    }

    pub fn as_slice(&self) -> &[T] {
        self.host_data.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.dirty = true;
        self.host_data.as_mut_slice()
    }

    pub fn as_ptr(&self) -> *const T {
        self.host_data.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.dirty = true;
        self.host_data.as_mut_ptr()
    }

    pub fn buffer(&self) -> &B::Buffer {
        self.buffer.buffer()
    }

    pub fn memory(&self) -> &B::Memory {
        self.buffer.memory()
    }
}

impl<T: Sized + Default + Clone, B: hal::Backend> Index<usize> for ManagedBuffer<T, B> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.host_data[index]
    }
}

impl<T: Sized + Default + Clone, B: hal::Backend> IndexMut<usize> for ManagedBuffer<T, B> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.dirty = true;
        &mut self.host_data[index]
    }
}
