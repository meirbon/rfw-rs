use crate::mem::ManagedBuffer;
use bitflags::bitflags;
use num_integer::Integer;
use std::fmt::Debug;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone)]
pub struct RangeDescriptor<T: Sized, JW: Sized> {
    pub ptr: Vec<T>,
    pub start: u32,
    pub count: u32,
    pub capacity: u32,
    pub jw_ptr: Vec<JW>,
    pub jw_start: u32,
    pub changed: u32,
}

#[derive(Debug, Copy, Clone)]
pub struct DrawDescriptor {
    pub start: u32,
    pub end: u32,
    pub jw_start: u32,
    pub jw_end: u32,
}

bitflags! {
    #[derive(Default)]
    pub struct VertexListFlags: u32 {
        const CALCULATE_RANGES = 1;
        const UPDATE_DATA = 2;
    }
}

#[derive(Debug)]
pub struct VertexList<T: Debug + Copy + Sized + Default, JW: Debug + Copy + Sized + Default = u32> {
    buffer: ManagedBuffer<T>,
    jw_buffer: ManagedBuffer<JW>,
    pointers: BTreeMap<usize, RangeDescriptor<T, JW>>,
    draw_ranges: BTreeMap<usize, DrawDescriptor>,
    total_vertices: usize,
    total_jw: usize,
    update_flags: VertexListFlags,
}

impl<T: Debug + Copy + Sized + Default, JW: Debug + Copy + Sized + Default> VertexList<T, JW> {
    pub fn new(device: &Arc<wgpu::Device>, queue: &Arc<wgpu::Queue>) -> Self {
        Self {
            buffer: ManagedBuffer::new(
                device.clone(),
                queue.clone(),
                wgpu::BufferUsage::STORAGE
                    | wgpu::BufferUsage::VERTEX
                    | wgpu::BufferUsage::COPY_DST,
                2048,
            ),
            jw_buffer: ManagedBuffer::new(
                device.clone(),
                queue.clone(),
                wgpu::BufferUsage::STORAGE
                    | wgpu::BufferUsage::VERTEX
                    | wgpu::BufferUsage::COPY_DST,
                2048,
            ),
            pointers: BTreeMap::new(),
            draw_ranges: BTreeMap::new(),
            total_vertices: 0,
            total_jw: 0,
            update_flags: Default::default(),
        }
    }

    pub fn add_pointer(&mut self, id: usize, data: Vec<T>, joints_weights: Vec<JW>) {
        let count = data.len() as u32;
        let capacity = data.len().next_multiple_of(&512) as u32;
        self.pointers.insert(
            id,
            RangeDescriptor {
                ptr: data,
                start: 0,
                capacity,
                count,
                jw_ptr: joints_weights,
                jw_start: 0,
                changed: 1,
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

        self.update_flags = VertexListFlags::CALCULATE_RANGES | VertexListFlags::UPDATE_DATA;
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn has(&self, id: usize) -> bool {
        self.draw_ranges.get(&id).is_some()
    }

    pub fn update_pointer(&mut self, id: usize, data: Vec<T>, joints_weights: Vec<JW>) {
        let reference = self.pointers.get_mut(&id).unwrap();
        let draw_range = self.draw_ranges.get_mut(&id).unwrap();

        if data.len() as u32 > reference.capacity {
            // if we're out of capacity, we need to recalculate the range of each mesh
            self.update_flags = VertexListFlags::CALCULATE_RANGES;
            reference.capacity = (data.len() as u32).next_multiple_of(&512);
        }

        reference.count = data.len() as _;
        reference.ptr = data;
        reference.jw_ptr = joints_weights;
        reference.changed = 1;
        draw_range.end = draw_range.start + reference.count;

        self.update_flags = VertexListFlags::UPDATE_DATA;
    }

    pub fn remove_pointer(&mut self, id: usize) -> bool {
        self.pointers.remove(&id).is_some() && self.draw_ranges.remove(&id).is_some()
        // no need to recalculate ranges
    }

    pub fn update(&mut self) {
        if self
            .update_flags
            .contains(VertexListFlags::CALCULATE_RANGES)
        {
            self.update_ranges();
        }

        if self.update_flags.contains(VertexListFlags::UPDATE_DATA) {
            self.update_data();
        }

        self.update_flags = Default::default();
    }

    fn update_ranges(&mut self) {
        let mut current_offset = 0;
        let mut current_offset_jw = 0;

        for (id, desc) in self.pointers.iter_mut() {
            desc.start = current_offset;
            desc.changed = 1;

            if let Some(range) = self.draw_ranges.get_mut(id) {
                range.start = current_offset;
                range.end = current_offset + desc.count;

                if !desc.jw_ptr.is_empty() {
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
        self.update_flags.remove(VertexListFlags::CALCULATE_RANGES);
    }

    fn update_data(&mut self) {
        if self.total_vertices == 0 {
            return;
        }

        if self.buffer.len() < self.total_vertices {
            self.buffer
                .resize(self.total_vertices.next_multiple_of(&2048));
        }

        if self.jw_buffer.len() < self.total_jw {
            self.jw_buffer.resize(self.total_jw.next_multiple_of(&2048));
        }

        for (i, desc) in self.pointers.iter_mut() {
            let range = self.draw_ranges.get(i).unwrap();
            if desc.changed != 1 {
                continue;
            }

            let offset = range.start as usize;
            let offset_plus_count = range.end as usize;
            self.buffer.as_mut_slice()[offset..offset_plus_count]
                .copy_from_slice(desc.ptr.as_slice());

            self.buffer.copy_to_device_ranged(offset, offset_plus_count);

            if !desc.jw_ptr.is_empty() {
                let offset = range.jw_start as usize;
                let offset_plus_count = range.jw_end as usize;

                self.jw_buffer.as_mut_slice()[offset..offset_plus_count]
                    .copy_from_slice(desc.jw_ptr.as_slice());

                self.jw_buffer
                    .copy_to_device_ranged(offset, offset_plus_count);
            }

            desc.changed = 0
        }
    }

    pub fn requires_update(&self) -> bool {
        !self.update_flags.is_empty()
    }

    pub fn get_ranges(&self) -> &BTreeMap<usize, DrawDescriptor> {
        &self.draw_ranges
    }

    pub(crate) fn get_vertex_buffer(&self) -> &ManagedBuffer<T> {
        &self.buffer
    }

    pub(crate) fn get_jw_buffer(&self) -> &ManagedBuffer<JW> {
        &self.jw_buffer
    }
}

#[derive(Debug, Clone)]
pub struct InstanceRange<
    T: Debug + Copy + Sized + Default,
    Ex: Debug + Clone + Sized + Default = (),
> {
    ptr: *const T,
    pub start: u32,
    pub end: u32,
    pub count: u32,
    pub capacity: u32,
    pub extra: Ex,
}

#[derive(Debug)]
pub struct InstanceList<T: Debug + Copy + Sized + Default, Ex: Debug + Clone + Sized + Default = ()>
{
    device_buffer: ManagedBuffer<T>,
    lists: BTreeMap<usize, InstanceRange<T, Ex>>,
    total: usize,
    recalculate_ranges: bool,
}

impl<T: Debug + Copy + Sized + Default, Ex: Debug + Clone + Sized + Default> InstanceList<T, Ex> {
    pub fn new(device: &Arc<wgpu::Device>, queue: &Arc<wgpu::Queue>) -> Self {
        Self {
            device_buffer: ManagedBuffer::new(
                device.clone(),
                queue.clone(),
                wgpu::BufferUsage::STORAGE | wgpu::BufferUsage::COPY_DST,
                512,
            ),
            lists: Default::default(),
            total: 0,
            recalculate_ranges: true,
        }
    }

    pub fn has(&self, id: usize) -> bool {
        self.lists.get(&id).is_some()
    }

    pub fn add_instances_list(&mut self, id: usize, ptr: *const T, count: u32, extra: Ex) {
        self.lists.insert(
            id,
            InstanceRange {
                ptr,
                start: 0,
                end: 0,
                count,
                capacity: count.next_multiple_of(&4),
                extra,
            },
        );
    }

    pub fn update_instances_list(&mut self, id: usize, ptr: *const T, count: u32, extra: Ex) {
        let list = self.lists.get_mut(&id).unwrap();

        if count > list.capacity {
            self.recalculate_ranges = true;
        }
        list.ptr = ptr;
        list.count = count;
        list.capacity = count.next_multiple_of(&4);
        list.extra = extra;
    }

    pub fn remove_instances_list(&mut self, id: usize) -> bool {
        self.lists.remove(&id).is_some()
    }

    pub fn get_ranges(&self) -> &BTreeMap<usize, InstanceRange<T, Ex>> {
        &self.lists
    }

    pub fn get_buffer(&self) -> &ManagedBuffer<T> {
        &self.device_buffer
    }

    pub fn update(&mut self) {
        if self.recalculate_ranges {
            self.update_ranges();
        }

        self.update_data();
    }

    fn update_ranges(&mut self) {
        let mut current_offset = 0;
        for (_, desc) in self.lists.iter_mut() {
            desc.start = current_offset;
            desc.end = desc.start + desc.count;
            current_offset += desc.capacity;
        }

        self.total = current_offset as usize;
        self.recalculate_ranges = false;
    }

    fn update_data(&mut self) {
        if self.total == 0 {
            return;
        }

        if self.device_buffer.len() < self.total {
            self.device_buffer.resize(self.total.next_multiple_of(&512));
        }

        {
            for (_, desc) in self.lists.iter() {
                let offset = desc.start as usize;
                let offset_plus_count = offset + desc.count as usize;

                self.device_buffer.as_mut_slice()[offset..offset_plus_count].copy_from_slice(
                    unsafe { std::slice::from_raw_parts(desc.ptr, desc.count as usize) },
                );
            }
        }

        self.device_buffer.copy_to_device();
    }
}
