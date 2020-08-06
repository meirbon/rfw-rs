use std::ops::Index;
use std::ops::IndexMut;

pub struct ManagedBuffer<T: Sized + Default + Clone> {
    host_buffer: Vec<T>,
    device_buffer: wgpu::Buffer,
    staging_buffer: Option<wgpu::Buffer>,
    usage: wgpu::BufferUsage,
    dirty: bool,
}

#[allow(dead_code)]
impl<T: Sized + Default + Clone> ManagedBuffer<T> {
    pub fn new(device: &wgpu::Device, capacity: usize, usage: wgpu::BufferUsage) -> Self {
        let usage = usage | wgpu::BufferUsage::COPY_DST;
        let device_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (capacity * std::mem::size_of::<T>()) as wgpu::BufferAddress,
            usage,
        });

        Self {
            host_buffer: vec![T::default(); capacity.max(1)],
            device_buffer,
            staging_buffer: None,
            usage,
            dirty: true,
        }
    }

    pub fn resize(&mut self, device: &wgpu::Device, new_size: usize) {
        if self.host_buffer.len() >= new_size {
            return;
        }

        // Create a larger buffer to ensure resizing does not happen often
        let new_size = new_size * 2;

        self.host_buffer.resize(new_size, T::default());
        self.device_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (new_size * std::mem::size_of::<T>()) as wgpu::BufferAddress,
            usage: self.usage,
        });
        self.dirty = true;
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.host_buffer.as_ptr() as *const u8, self.bytes()) }
    }

    pub fn as_slice(&self) -> &[T] {
        self.host_buffer.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.dirty = true;
        self.host_buffer.as_mut_slice()
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.host_buffer.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.dirty = true;
        self.host_buffer.get_mut(index)
    }

    pub fn len(&self) -> usize {
        self.host_buffer.len()
    }

    pub fn bytes(&self) -> usize {
        self.host_buffer.len() * std::mem::size_of::<T>()
    }

    pub fn copy_from_slice(&mut self, data: &[T]) {
        assert!(
            self.host_buffer.len() >= data.len(),
            "Data to copy was longer ({}) than buffer ({})",
            data.len(),
            self.len()
        );
        for i in 0..data.len() {
            self.host_buffer[i] = data[i].clone();
        }
        self.dirty = true;
    }

    pub fn copy_from_slice_offset(&mut self, data: &[T], offset: usize) {
        assert!(
            data.len() < (self.len() - offset),
            "Data bigger in size ({}) than copy destination ({})",
            data.len(),
            self.len() - offset
        );
        for i in offset..(offset + data.len()) {
            self.host_buffer[i] = data[i - offset].clone();
        }
        self.dirty = true;
    }

    pub fn update(&mut self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder) {
        if self.dirty {
            let copy_size = self.bytes() as wgpu::BufferAddress;
            let staging_buffer = device.create_buffer_mapped(&wgpu::BufferDescriptor {
                usage: wgpu::BufferUsage::MAP_WRITE | wgpu::BufferUsage::COPY_SRC,
                label: Some("update-staging-buffer"),
                size: copy_size,
            });
            staging_buffer.data.copy_from_slice(self.as_bytes());
            self.staging_buffer = Some(staging_buffer.finish());

            device.create_buffer_with_data(self.as_bytes(), wgpu::BufferUsage::COPY_SRC);
            encoder.copy_buffer_to_buffer(
                self.staging_buffer.as_ref().unwrap(),
                0,
                &self.device_buffer,
                0,
                copy_size,
            );
        }

        self.dirty = false;
    }

    pub fn as_binding(&self, index: u32) -> wgpu::Binding {
        let binding = wgpu::Binding {
            binding: index,
            resource: wgpu::BindingResource::Buffer {
                buffer: &self.device_buffer,
                range: 0..(self.bytes() as wgpu::BufferAddress),
            },
        };

        binding
    }
}

impl<T: Sized + Default + Clone> Index<usize> for ManagedBuffer<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        if let Some(item) = self.get(index) {
            item
        } else {
            panic!("Index {} was out of bounds", index);
        }
    }
}

impl<T: Sized + Default + Clone> IndexMut<usize> for ManagedBuffer<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if let Some(item) = self.host_buffer.get_mut(index) {
            self.dirty = true;
            item
        } else {
            panic!("Index {} was out of bounds", index);
        }
    }
}
