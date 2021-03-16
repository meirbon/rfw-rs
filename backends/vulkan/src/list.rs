use crate::memory::VkBuffer;
use crate::VkAllocator;
use ash::version::DeviceV1_0;
use ash::*;
use num_integer::Integer;
use std::collections::BTreeMap;
use std::fmt::Debug;

#[derive(Debug, Copy, Clone)]
pub struct RangeDescriptor<T: Sized, JW: Sized> {
    pub ptr: *const T,
    pub start: u32,
    pub count: u32,
    pub capacity: u32,
    pub jw_ptr: Option<*const JW>,
    pub jw_start: u32,
}

#[derive(Debug, Copy, Clone)]
pub struct DrawDescriptor {
    pub start: u32,
    pub end: u32,
    pub jw_start: u32,
    pub jw_end: u32,
}

#[derive(Debug)]
pub struct VertexList<T: Debug + Copy + Sized + Default, JW: Debug + Copy + Sized + Default = u32> {
    buffer: Option<VkBuffer<T>>,
    jw_buffer: Option<VkBuffer<JW>>,
    anim_buffer: Option<VkBuffer<T>>,
    host_buffer: Option<VkBuffer<T>>,
    jw_host_buffer: Option<VkBuffer<JW>>,
    pointers: BTreeMap<usize, RangeDescriptor<T, JW>>,
    draw_ranges: BTreeMap<usize, DrawDescriptor>,
    total_vertices: usize,
    total_jw: usize,
    recalculate_ranges: bool,
}

impl<T: Debug + Copy + Sized + Default, JW: Debug + Copy + Sized + Default> Default
    for VertexList<T, JW>
{
    fn default() -> Self {
        Self {
            buffer: None,
            jw_buffer: None,
            anim_buffer: None,
            host_buffer: None,
            jw_host_buffer: None,
            pointers: BTreeMap::new(),
            draw_ranges: BTreeMap::new(),
            total_vertices: 0,
            total_jw: 0,
            recalculate_ranges: true,
        }
    }
}

impl<T: Debug + Copy + Sized + Default, JW: Debug + Copy + Sized + Default> VertexList<T, JW> {
    pub fn new() -> Self {
        Self {
            buffer: None,
            jw_buffer: None,
            anim_buffer: None,
            host_buffer: None,
            jw_host_buffer: None,
            pointers: BTreeMap::new(),
            draw_ranges: BTreeMap::new(),
            total_vertices: 0,
            total_jw: 0,
            recalculate_ranges: true,
        }
    }

    pub fn add_pointer(
        &mut self,
        id: usize,
        pointer: *const T,
        joints_weights: Option<*const JW>,
        count: u32,
    ) {
        self.pointers.insert(
            id,
            RangeDescriptor {
                ptr: pointer,
                start: 0,
                capacity: count.next_multiple_of(&512),
                count,
                jw_ptr: joints_weights,
                jw_start: 0,
            },
        );

        self.draw_ranges.insert(
            id,
            DrawDescriptor {
                start: 0, // Will be filled in later
                end: count,
                jw_start: 0,
                jw_end: 0,
            },
        );

        self.recalculate_ranges = true;
    }

    pub fn len(&self) -> usize {
        self.buffer
            .as_ref()
            .and_then(|b| Some(b.len()))
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn has(&self, id: usize) -> bool {
        self.draw_ranges.get(&id).is_some()
    }

    pub fn update_pointer(
        &mut self,
        id: usize,
        pointer: *const T,
        joints_weights: Option<*const JW>,
        count: u32,
    ) {
        let reference = self.pointers.get_mut(&id).unwrap();
        let draw_range = self.draw_ranges.get_mut(&id).unwrap();

        if count as u32 > reference.capacity {
            // if we're out of capacity, we need to recalculate the range of each mesh
            self.recalculate_ranges = true;
            reference.capacity = count.next_multiple_of(&512);
        }

        reference.ptr = pointer;
        reference.jw_ptr = joints_weights;
        reference.count = count;
        draw_range.end = draw_range.start + count;
    }

    pub fn remove_pointer(&mut self, id: usize) -> bool {
        self.pointers.remove(&id).is_some() && self.draw_ranges.remove(&id).is_some()
        // no need to recalculate ranges
    }

    pub fn update_ranges(&mut self) {
        if !self.recalculate_ranges {
            return;
        }

        let mut current_offset = 0;
        let mut current_offset_jw = 0;

        for (id, desc) in self.pointers.iter_mut() {
            desc.start = current_offset;
            if let Some(range) = self.draw_ranges.get_mut(id) {
                range.start = current_offset;
                range.end = current_offset + desc.count;

                if desc.jw_ptr.is_some() {
                    desc.jw_start = current_offset_jw;
                    range.jw_start = current_offset_jw;
                    range.jw_end = current_offset_jw + desc.count;

                    current_offset_jw += desc.capacity;
                } else {
                    desc.jw_start = 0;
                    range.jw_start = 0;
                    range.jw_end = 0;
                }
            }

            current_offset += desc.capacity;
        }

        self.total_vertices = current_offset as usize;
        self.total_jw = current_offset_jw as usize;
        self.recalculate_ranges = false;
    }

    pub fn update_data(
        &mut self,
        allocator: &VkAllocator,
        device: &Device,
        cmd_buffer: vk::CommandBuffer,
    ) {
        if self.total_vertices == 0 {
            return;
        }

        let total = self.total_vertices;
        let host_buffer = if let Some(host_buffer) = self.host_buffer.as_mut() {
            if host_buffer.len() < total {
                *host_buffer = allocator
                    .create_buffer(
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::NONE,
                        total.next_multiple_of(&2048),
                    )
                    .unwrap();
            }
            host_buffer
        } else {
            self.host_buffer = Some(
                allocator
                    .create_buffer(
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::NONE,
                        total.next_multiple_of(&2048),
                    )
                    .unwrap(),
            );
            self.host_buffer.as_mut().unwrap()
        };

        {
            let mut data = host_buffer.map_memory().unwrap();
            for (_, desc) in self.pointers.iter() {
                let offset = desc.start as usize;
                let offset_plus_count = offset + desc.count as usize;

                data.as_slice()[offset..offset_plus_count].copy_from_slice(unsafe {
                    std::slice::from_raw_parts(desc.ptr, desc.count as usize)
                });
            }
        }

        let buffer = if let Some(buffer) = self.buffer.as_mut() {
            if buffer.len() < total {
                *buffer = allocator
                    .create_buffer(
                        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                        vk_mem::MemoryUsage::GpuOnly,
                        vk_mem::AllocationCreateFlags::NONE,
                        total.next_multiple_of(&2048),
                    )
                    .unwrap()
            }

            buffer
        } else {
            let buffer = allocator
                .create_buffer(
                    vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                    vk_mem::MemoryUsage::GpuOnly,
                    vk_mem::AllocationCreateFlags::NONE,
                    total.next_multiple_of(&2048),
                )
                .unwrap();

            self.buffer = Some(buffer);
            self.buffer.as_mut().unwrap()
        };

        unsafe {
            device.cmd_copy_buffer(
                cmd_buffer,
                host_buffer.buffer,
                buffer.buffer,
                &[vk::BufferCopy {
                    size: (total * std::mem::size_of::<T>()) as _,
                    dst_offset: 0 as _,
                    src_offset: 0 as _,
                }],
            );
        }

        let total = self.total_jw;
        if total == 0 {
            return;
        }

        let host_buffer = if let Some(host_buffer) = self.jw_host_buffer.as_mut() {
            if host_buffer.len() < total {
                *host_buffer = allocator
                    .create_buffer(
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::NONE,
                        total.next_multiple_of(&2048),
                    )
                    .unwrap();
            }
            host_buffer
        } else {
            self.jw_host_buffer = Some(
                allocator
                    .create_buffer(
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::NONE,
                        total.next_multiple_of(&2048),
                    )
                    .unwrap(),
            );
            self.jw_host_buffer.as_mut().unwrap()
        };

        {
            let mut data = host_buffer.map_memory().unwrap();
            for (_, desc) in self.pointers.iter() {
                if let Some(ptr) = desc.jw_ptr {
                    let offset = desc.jw_start as usize;
                    let offset_plus_count = offset + desc.count as usize;

                    data.as_slice()[offset..offset_plus_count].copy_from_slice(unsafe {
                        std::slice::from_raw_parts(ptr, desc.count as usize)
                    });
                }
            }
        }

        let buffer = if let Some(buffer) = self.jw_buffer.as_mut() {
            if buffer.len() < total {
                *buffer = allocator
                    .create_buffer(
                        vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                        vk_mem::MemoryUsage::GpuOnly,
                        vk_mem::AllocationCreateFlags::NONE,
                        total.next_multiple_of(&2048),
                    )
                    .unwrap();

                self.anim_buffer = Some(
                    allocator
                        .create_buffer(
                            vk::BufferUsageFlags::TRANSFER_SRC,
                            vk_mem::MemoryUsage::CpuToGpu,
                            vk_mem::AllocationCreateFlags::NONE,
                            total.next_multiple_of(&2048),
                        )
                        .unwrap(),
                );
            }

            buffer
        } else {
            let buffer = allocator
                .create_buffer(
                    vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                    vk_mem::MemoryUsage::GpuOnly,
                    vk_mem::AllocationCreateFlags::NONE,
                    total.next_multiple_of(&2048),
                )
                .unwrap();

            self.jw_buffer = Some(buffer);

            self.anim_buffer = Some(
                allocator
                    .create_buffer(
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::NONE,
                        total.next_multiple_of(&2048),
                    )
                    .unwrap(),
            );

            self.jw_buffer.as_mut().unwrap()
        };

        unsafe {
            device.cmd_copy_buffer(
                cmd_buffer,
                host_buffer.buffer,
                buffer.buffer,
                &[vk::BufferCopy {
                    size: (total * std::mem::size_of::<JW>()) as _,
                    dst_offset: 0 as _,
                    src_offset: 0 as _,
                }],
            );
        }
    }

    pub fn free_buffers(&mut self) {
        if let Some(buffer) = self.buffer.take() {
            drop(buffer);
        }

        if let Some(buffer) = self.host_buffer.take() {
            drop(buffer);
        }

        if let Some(buffer) = self.jw_buffer.take() {
            drop(buffer);
        }

        if let Some(buffer) = self.jw_host_buffer.take() {
            drop(buffer);
        }

        if let Some(buffer) = self.anim_buffer.take() {
            drop(buffer);
        }
    }

    pub fn get_ranges(&self) -> &BTreeMap<usize, DrawDescriptor> {
        &self.draw_ranges
    }

    pub fn get_vertex_buffer(&self) -> Option<vk::Buffer> {
        self.buffer.as_ref().and_then(|b| Some(b.buffer))
    }

    pub fn get_jw_buffer(&self) -> Option<vk::Buffer> {
        self.jw_buffer.as_ref().and_then(|b| Some(b.buffer))
    }

    pub fn get_anim_vertex_buffer(&self) -> Option<vk::Buffer> {
        self.anim_buffer.as_ref().and_then(|b| Some(b.buffer))
    }
}

impl<T: Debug + Copy + Sized + Default, JW: Debug + Copy + Sized + Default> Drop
    for VertexList<T, JW>
{
    fn drop(&mut self) {
        self.free_buffers();
    }
}

#[derive(Debug, Copy, Clone)]
pub struct InstanceRange<T: Debug + Copy + Sized + Default> {
    ptr: *const T,
    pub start: u32,
    pub end: u32,
    pub count: u32,
    pub capacity: u32,
}

#[derive(Debug)]
pub struct InstanceList<T: Debug + Copy + Sized + Default> {
    host_buffer: Option<VkBuffer<T>>,
    device_buffer: Option<VkBuffer<T>>,
    lists: BTreeMap<usize, InstanceRange<T>>,
    total: usize,
    recalculate_ranges: bool,
}

impl<T: Debug + Copy + Sized + Default> Default for InstanceList<T> {
    fn default() -> Self {
        Self {
            host_buffer: None,
            device_buffer: None,
            lists: Default::default(),
            total: 0,
            recalculate_ranges: true,
        }
    }
}

impl<T: Debug + Copy + Sized + Default> InstanceList<T> {
    pub fn free_buffers(&mut self) {
        if let Some(buffer) = self.host_buffer.take() {
            drop(buffer);
        }

        if let Some(buffer) = self.device_buffer.take() {
            drop(buffer);
        }
    }

    pub fn has(&self, id: usize) -> bool {
        self.lists.get(&id).is_some()
    }

    pub fn add_instances_list(&mut self, id: usize, ptr: *const T, count: u32) {
        self.lists.insert(
            id,
            InstanceRange {
                ptr,
                start: 0,
                end: 0,
                count,
                capacity: count.next_multiple_of(&128),
            },
        );
    }

    pub fn update_instances_list(&mut self, id: usize, ptr: *const T, count: u32) {
        let list = self.lists.get_mut(&id).unwrap();

        if count > list.capacity {
            self.recalculate_ranges = true;
        }
        list.ptr = ptr;
        list.count = count;
        list.capacity = count.next_multiple_of(&128);
    }

    pub fn remove_instances_list(&mut self, id: usize) -> bool {
        self.lists.remove(&id).is_some()
    }

    pub fn get_ranges(&self) -> &BTreeMap<usize, InstanceRange<T>> {
        &self.lists
    }

    pub fn get_buffer(&self) -> Option<vk::Buffer> {
        self.device_buffer.as_ref().and_then(|b| Some(b.buffer))
    }

    pub fn update_ranges(&mut self) {
        if !self.recalculate_ranges {
            return;
        }

        let mut current_offset = 0;
        for (_, desc) in self.lists.iter_mut() {
            desc.start = current_offset;
            desc.end = desc.start + desc.count;
            current_offset += desc.capacity;
        }

        self.total = current_offset as usize;
        self.recalculate_ranges = false;
    }

    pub fn update_data(
        &mut self,
        allocator: &VkAllocator,
        device: &Device,
        cmd_buffer: vk::CommandBuffer,
    ) {
        if self.total == 0 {
            return;
        }

        let buffer = if let Some(buffer) = self.device_buffer.as_mut() {
            if buffer.len() < self.total {
                *buffer = allocator
                    .create_buffer(
                        vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::NONE,
                        self.total.next_multiple_of(&512),
                    )
                    .unwrap();
            }

            buffer
        } else {
            let buffer = allocator
                .create_buffer(
                    vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                    vk_mem::MemoryUsage::CpuToGpu,
                    vk_mem::AllocationCreateFlags::NONE,
                    self.total.next_multiple_of(&512),
                )
                .unwrap();

            self.device_buffer = Some(buffer);
            self.device_buffer.as_mut().unwrap()
        };

        let host_buffer = if let Some(host_buffer) = self.host_buffer.as_mut() {
            if host_buffer.len() < self.total {
                *host_buffer = allocator
                    .create_buffer(
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::NONE,
                        self.total.next_multiple_of(&512),
                    )
                    .unwrap();
            }
            host_buffer
        } else {
            self.host_buffer = Some(
                allocator
                    .create_buffer(
                        vk::BufferUsageFlags::TRANSFER_SRC,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::NONE,
                        self.total.next_multiple_of(&512),
                    )
                    .unwrap(),
            );
            self.host_buffer.as_mut().unwrap()
        };

        {
            let mut data = host_buffer.map_memory().unwrap();
            for (_, desc) in self.lists.iter() {
                let offset = desc.start as usize;
                let offset_plus_count = offset + desc.count as usize;

                data.as_slice()[offset..offset_plus_count].copy_from_slice(unsafe {
                    std::slice::from_raw_parts(desc.ptr, desc.count as usize)
                });
            }
        }

        unsafe {
            device.cmd_copy_buffer(
                cmd_buffer,
                host_buffer.buffer,
                buffer.buffer,
                &[vk::BufferCopy {
                    size: (self.total * std::mem::size_of::<T>()) as _,
                    dst_offset: 0_u64,
                    src_offset: 0_u64,
                }],
            );
        }
    }
}

impl<T: Debug + Copy + Sized + Default> Drop for InstanceList<T> {
    fn drop(&mut self) {
        self.free_buffers();
    }
}
