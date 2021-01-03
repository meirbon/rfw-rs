use rfw::utils::BytesConversion;
use std::ops::{Index, IndexMut, Range};
use std::sync::Arc;

pub struct ManagedBuffer<T: Sized> {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    host_buffer: Vec<T>,
    buffer: wgpu::Buffer,
}

#[allow(dead_code)]
impl<T: Sized> ManagedBuffer<T> {
    pub fn new_with_buffer(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        usage: wgpu::BufferUsage,
        host_data: Vec<T>,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * host_data.len()) as wgpu::BufferAddress,
            usage: usage | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            host_buffer: host_data,
            buffer,
        }
    }

    pub fn new_with(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        usage: wgpu::BufferUsage,
        count: usize,
        data: T,
    ) -> Self
    where
        T: Clone,
    {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * count) as wgpu::BufferAddress,
            usage: usage | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            host_buffer: vec![data; count],
            buffer,
        }
    }

    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        usage: wgpu::BufferUsage,
        count: usize,
    ) -> Self
    where
        T: Default + Clone,
    {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * count) as wgpu::BufferAddress,
            usage: usage | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            host_buffer: vec![T::default(); count],
            buffer,
        }
    }

    pub fn binding_resource(&self, bounds: Range<wgpu::BufferAddress>) -> wgpu::BindingResource {
        wgpu::BindingResource::Buffer(self.buffer.slice(bounds))
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn copy_to_device(&self) {
        self.queue
            .write_buffer(&self.buffer, 0, self.host_buffer.as_bytes());
    }

    pub fn len(&self) -> usize {
        self.host_buffer.len()
    }

    pub fn byte_size(&self) -> usize {
        self.host_buffer.len() * std::mem::size_of::<T>()
    }

    pub fn as_slice(&self) -> &[T] {
        self.host_buffer.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.host_buffer.as_mut_slice()
    }
}

impl<T: Sized> Index<usize> for ManagedBuffer<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.host_buffer[index]
    }
}

impl<T: Sized> IndexMut<usize> for ManagedBuffer<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.host_buffer[index]
    }
}
