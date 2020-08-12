use crate::buffer::Allocator;
use crate::hal;
use crate::light::map::{DepthType, FilterPipeline};
use hal::*;
use rfw_scene::*;
use std::sync::Arc;

pub mod map;

#[derive(Debug)]
pub struct LightList<B: hal::Backend> {
    // point: map::Array<B, PointLight>,
    area: map::Array<B, AreaLight>,
    spot: map::Array<B, SpotLight>,
    dir: map::Array<B, DirectionalLight>,
}

impl<B: hal::Backend> LightList<B> {
    pub fn new(
        device: Arc<B::Device>,
        allocator: Allocator<B>,
        instances_pipeline_layout: &B::PipelineLayout,
        capacity: usize,
    ) -> Self {
        let filter_pipeline = Arc::new(FilterPipeline::new(device.clone()));

        Self {
            // point: map::Array::new(device.clone(), allocator.clone(), capacity),
            area: map::Array::new(
                device.clone(),
                allocator.clone(),
                filter_pipeline.clone(),
                instances_pipeline_layout,
                DepthType::Perspective,
                capacity,
            ),
            spot: map::Array::new(
                device.clone(),
                allocator.clone(),
                filter_pipeline.clone(),
                instances_pipeline_layout,
                DepthType::Perspective,
                capacity,
            ),
            dir: map::Array::new(
                device,
                allocator,
                filter_pipeline,
                instances_pipeline_layout,
                DepthType::Linear,
                capacity,
            ),
        }
    }

    pub fn set_point_lights(&mut self, lights: &[PointLight]) {}
}
