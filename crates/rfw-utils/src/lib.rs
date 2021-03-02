pub mod collections;
pub mod task;
pub mod input;
pub mod log;

pub use bitvec::prelude::*;

use std::time::{Duration, Instant};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timer {
    moment: Instant,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            moment: Instant::now(),
        }
    }

    pub fn reset(&mut self) {
        self.moment = Instant::now();
    }

    pub fn elapsed(&self) -> Duration {
        self.moment.elapsed()
    }

    pub fn elapsed_in_millis(&self) -> f32 {
        let elapsed = self.elapsed();
        let secs = elapsed.as_secs() as u32;
        let millis = elapsed.subsec_micros();
        (secs * 1_000) as f32 + (millis as f32 / 1000.0)
    }
}

#[derive(Debug, Clone)]
pub struct Averager<T: num::Float + num::FromPrimitive> {
    values: Vec<T>,
    capacity: usize,
    index: usize,
    has_looped: bool,
}

impl<T: num::Float + num::FromPrimitive> Default for Averager<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: num::Float + num::FromPrimitive> Averager<T> {
    pub fn new() -> Averager<T> {
        Self {
            values: vec![T::from_f32(0.0).unwrap(); 100],
            capacity: 100,
            index: 0,
            has_looped: false,
        }
    }

    pub fn with_capacity(capacity: usize) -> Averager<T> {
        Self {
            values: vec![T::from_f32(0.0).unwrap(); capacity],
            capacity,
            index: 0,
            has_looped: false,
        }
    }

    pub fn add_sample(&mut self, sample: T) {
        if self.has_looped {
            for i in 0..(self.capacity - 1) {
                self.values[i] = self.values[i + 1];
            }
            self.values[self.capacity - 1] = sample;
            return;
        }

        if self.index >= (self.capacity - 1) {
            self.has_looped = true;
        }

        self.values[self.index] = sample;
        self.index += 1;
    }

    pub fn get_average(&mut self) -> T {
        let range = if self.has_looped {
            self.capacity
        } else {
            self.index
        };
        let mut avg = T::from(0.0).unwrap();
        for i in 0..range {
            avg = avg + self.values[i];
        }
        avg * (T::from_f32(1.0).unwrap() / T::from_usize(range).unwrap())
    }

    pub fn data(&self) -> &[T] {
        &self.values[0..self.index.min(self.capacity)]
    }
}

pub trait BytesConversion {
    fn as_bytes(&self) -> &[u8];
    fn as_quad_bytes(&self) -> &[u32];
}

pub fn as_bytes<T: Sized>(data: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data as *const T as *const u8, std::mem::size_of::<T>()) }
}

pub fn as_quad_bytes<T: Sized>(data: &T) -> &[u32] {
    unsafe {
        std::slice::from_raw_parts(data as *const T as *const u32, std::mem::size_of::<T>() / 4)
    }
}

impl<T: Sized> BytesConversion for &[T] {
    fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.as_ptr() as *const u8,
                self.len() * std::mem::size_of::<T>(),
            )
        }
    }

    fn as_quad_bytes(&self) -> &[u32] {
        unsafe {
            std::slice::from_raw_parts(
                self.as_ptr() as *const u32,
                self.len() * std::mem::size_of::<T>() / 4,
            )
        }
    }
}

impl<T: Sized> BytesConversion for Vec<T> {
    fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self.as_ptr() as *const u8,
                self.len() * std::mem::size_of::<T>(),
            )
        }
    }

    fn as_quad_bytes(&self) -> &[u32] {
        unsafe {
            std::slice::from_raw_parts(
                self.as_ptr() as *const u32,
                self.len() * std::mem::size_of::<T>() / 4,
            )
        }
    }
}
