use rfw::utils::BytesConversion;
use std::ops::{Index, IndexMut, Range};
use std::sync::Arc;

pub struct ManagedBuffer<T: Sized> {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    host_buffer: Vec<T>,
    buffer: wgpu::Buffer,
    usage: wgpu::BufferUsage,
}

#[allow(dead_code)]
impl<T: Sized> ManagedBuffer<T> {
    pub fn new_with_buffer(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        usage: wgpu::BufferUsage,
        host_data: Vec<T>,
    ) -> Self {
        let usage = usage | wgpu::BufferUsage::COPY_DST;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * host_data.len()) as wgpu::BufferAddress,
            usage,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            host_buffer: host_data,
            buffer,
            usage,
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
        let usage = usage | wgpu::BufferUsage::COPY_DST;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * count) as wgpu::BufferAddress,
            mapped_at_creation: false,
            usage,
        });

        Self {
            device,
            queue,
            host_buffer: vec![data; count],
            buffer,
            usage,
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
        let usage = usage | wgpu::BufferUsage::COPY_DST;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * count) as wgpu::BufferAddress,
            usage,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            host_buffer: vec![T::default(); count],
            buffer,
            usage,
        }
    }

    pub fn binding_resource(&self) -> wgpu::BindingResource {
        wgpu::BindingResource::Buffer(self.buffer.slice(..))
    }

    pub fn binding_resource_ranged(
        &self,
        range: Range<wgpu::BufferAddress>,
    ) -> wgpu::BindingResource {
        wgpu::BindingResource::Buffer(self.buffer.slice(range))
    }

    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    pub fn copy_to_device(&self) {
        self.queue
            .write_buffer(&self.buffer, 0, self.host_buffer.as_bytes());
    }

    pub fn copy_using_map(&mut self) {
        // FIXME: Currently, wgpu causes major stalls when a map is requested.
        // To work around this issue, we create a new buffer but it would be preferable to reuse
        // buffers instead.
        self.buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * self.len()) as wgpu::BufferAddress,
            usage: self.usage,
            mapped_at_creation: true,
        });

        self.buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(self.host_buffer.as_bytes());
        self.buffer.unmap();
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

    pub fn resize(&mut self, size: usize)
    where
        T: Default + Clone,
    {
        self.host_buffer.resize(size, T::default());
        self.buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * size) as wgpu::BufferAddress,
            usage: self.usage,
            mapped_at_creation: false,
        });
    }

    pub fn resize_with(&mut self, size: usize, value: T)
    where
        T: Clone,
    {
        self.host_buffer.resize(size, value);
        self.buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (std::mem::size_of::<T>() * size) as wgpu::BufferAddress,
            usage: self.usage,
            mapped_at_creation: false,
        });
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
