use crate::hal;
use hal::{adapter::PhysicalDevice, buffer, device::Device, memory};
use rfw_scene::{Mesh, VertexData};
use std::iter;
use std::mem::{self, ManuallyDrop};
use std::{ptr, sync::Arc};

#[derive(Debug)]
pub struct GfxAnimMesh<B: hal::Backend> {
    pub device: Option<Arc<B::Device>>,
    pub memory: Option<ManuallyDrop<B::Memory>>,
    pub buffer: Option<ManuallyDrop<B::Buffer>>,
    buffer_len: usize,
    vertices: usize,
}

impl<B: hal::Backend> Default for GfxAnimMesh<B> {
    fn default() -> Self {
        Self {
            device: None,
            memory: None,
            buffer: None,
            buffer_len: 0,
            vertices: 0,
        }
    }
}

#[allow(dead_code)]
impl<B: hal::Backend> GfxAnimMesh<B> {
    pub fn new(device: Arc<B::Device>, adapter: &hal::adapter::Adapter<B>, mesh: &Mesh) -> Self {
        let mut m = Self::default();
        m.init(device);
        m.set_data(adapter, mesh);
        m
    }

    pub fn init(&mut self, device: Arc<B::Device>) {
        self.device = Some(device);
    }

    pub fn set_data(&mut self, adapter: &hal::adapter::Adapter<B>, mesh: &Mesh) {
        let device = match self.device.as_ref() {
            Some(device) => device,
            None => panic!("This mesh was not initialized"),
        };

        if mesh.vertices.is_empty() {
            self.buffer = None;
            return;
        }

        let memory_types = adapter.physical_device.memory_properties().memory_types;
        let limits = adapter.physical_device.limits();
        let non_coherent_alignment = limits.non_coherent_atom_size as u64;

        let buffer_len = (mesh.vertices.len() * mem::size_of::<VertexData>()) as u64;
        assert_ne!(buffer_len, 0);
        let padded_buffer_len = ((buffer_len + non_coherent_alignment - 1)
            / non_coherent_alignment)
            * non_coherent_alignment;

        let mut buffer = ManuallyDrop::new(
            unsafe { device.create_buffer(padded_buffer_len, buffer::Usage::STORAGE) }.unwrap(),
        );

        let buffer_req = unsafe { device.get_buffer_requirements(&buffer) };

        let upload_type = memory_types
            .iter()
            .enumerate()
            .position(|(id, mem_type)| {
                // type_mask is a bit field where each bit represents a memory type. If the bit is set
                // to 1 it means we can use that type for our buffer. So this code finds the first
                // memory type that has a `1` (or, is allowed), and is visible to the CPU.
                buffer_req.type_mask & (1 << id) != 0
                    && mem_type
                        .properties
                        .contains(memory::Properties::CPU_VISIBLE)
            })
            .unwrap()
            .into();

        let buffer_memory = unsafe {
            let memory = device
                .allocate_memory(upload_type, buffer_req.size)
                .unwrap();
            device.bind_buffer_memory(&memory, 0, &mut buffer).unwrap();
            let mapping = device.map_memory(&memory, memory::Segment::ALL).unwrap();
            ptr::copy_nonoverlapping(
                mesh.vertices.as_ptr() as *const u8,
                mapping,
                buffer_len as usize,
            );
            device
                .flush_mapped_memory_ranges(iter::once((&memory, memory::Segment::ALL)))
                .unwrap();
            device.unmap_memory(&memory);
            ManuallyDrop::new(memory)
        };

        self.memory = Some(buffer_memory);
        self.buffer = Some(buffer);
        self.buffer_len = buffer_len as usize;
    }

    pub fn len(&self) -> usize {
        self.vertices
    }

    pub fn valid(&self) -> bool {
        self.device.is_some() && self.buffer.is_some() && self.memory.is_some()
    }
}

impl<B: hal::Backend> Drop for GfxAnimMesh<B> {
    fn drop(&mut self) {
        if let Some(device) = self.device.as_ref() {
            let mut buf = None;
            std::mem::swap(&mut buf, &mut self.buffer);
            if let Some(buffer) = buf {
                unsafe {
                    device.destroy_buffer(ManuallyDrop::into_inner(buffer));
                }
            }

            let mut mem = None;
            std::mem::swap(&mut mem, &mut self.memory);
            if let Some(memory) = mem {
                unsafe { device.free_memory(ManuallyDrop::into_inner(memory)) }
            }
        }
    }
}
