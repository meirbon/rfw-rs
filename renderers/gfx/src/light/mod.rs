use crate::hal;
use hal::*;
use rfw_scene::*;
use crate::buffer::Allocator;
use std::sync::Arc;

pub mod map;

#[derive(Debug)]
pub struct LightList<B: hal::Backend> {
    point: map::Array<B, PointLight>,
    area: map::Array<B, AreaLight>,
    spot: map::Array<B, SpotLight>,
    dir: map::Array<B, DirectionalLight>,
}

impl<B: hal::Backend> LightList<B> {
    pub fn new(device: Arc<B::Device>, allocator: Allocator<B>, capacity: usize) -> Self {
        Self {
            point: map::Array::new(device.clone(), allocator.clone(), capacity),
            area: map::Array::new(device.clone(), allocator.clone(), capacity),
            spot: map::Array::new(device.clone(), allocator.clone(), capacity),
            dir: map::Array::new(device, allocator, capacity),
        }
    }
}