use crate::buffer::Allocator;
use crate::hal;
use crate::hal::device::Device;
use crate::hal::pso::DescriptorPool;
use crate::instances::SceneList;
use crate::light::map::{DepthType, FilterPipeline};
use hal::*;
use rfw_scene::*;
use std::mem::ManuallyDrop;
use std::sync::Arc;

pub mod map;

#[derive(Debug)]
pub struct LightList<B: hal::Backend> {
    device: Arc<B::Device>,
    // point: map::Array<B, PointLight>,
    area: Option<map::Array<B, AreaLight>>,
    spot: Option<map::Array<B, SpotLight>>,
    dir: Option<map::Array<B, DirectionalLight>>,
    pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    uniform_desc_pool: ManuallyDrop<B::DescriptorPool>,
    uniform_desc_layout: ManuallyDrop<B::DescriptorSetLayout>,
}

impl<B: hal::Backend> LightList<B> {
    pub fn new(
        device: Arc<B::Device>,
        allocator: Allocator<B>,
        instances_desc_layout: &B::DescriptorSetLayout,
        skins_desc_layout: &B::DescriptorSetLayout,
        capacity: usize,
    ) -> Self {
        let filter_pipeline = Arc::new(FilterPipeline::new(device.clone()));

        let uniform_desc_layout = unsafe {
            device
                .create_descriptor_set_layout(
                    &[pso::DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Uniform,
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: true,
                            },
                        },
                        count: 1,
                        stage_flags: pso::ShaderStageFlags::VERTEX
                            | pso::ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    }],
                    &[],
                )
                .unwrap()
        };

        let uniform_desc_pool = unsafe {
            device
                .create_descriptor_pool(
                    4,
                    &[pso::DescriptorRangeDesc {
                        ty: pso::DescriptorType::Buffer {
                            ty: pso::BufferDescriptorType::Uniform,
                            format: pso::BufferDescriptorFormat::Structured {
                                dynamic_offset: true,
                            },
                        },
                        count: 4,
                    }],
                    pso::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET,
                )
                .unwrap()
        };

        let pipeline_layout = unsafe {
            device
                .create_pipeline_layout(
                    vec![
                        instances_desc_layout,
                        skins_desc_layout,
                        &uniform_desc_layout,
                    ],
                    &[],
                )
                .unwrap()
        };

        Self {
            device: device.clone(),
            area: Some(map::Array::new(
                device.clone(),
                allocator.clone(),
                filter_pipeline.clone(),
                &pipeline_layout,
                DepthType::Perspective,
                capacity,
            )),
            spot: Some(map::Array::new(
                device.clone(),
                allocator.clone(),
                filter_pipeline.clone(),
                &pipeline_layout,
                DepthType::Perspective,
                capacity,
            )),
            dir: Some(map::Array::new(
                device,
                allocator,
                filter_pipeline,
                &pipeline_layout,
                DepthType::Linear,
                capacity,
            )),
            pipeline_layout: ManuallyDrop::new(pipeline_layout),
            uniform_desc_layout: ManuallyDrop::new(uniform_desc_layout),
            uniform_desc_pool: ManuallyDrop::new(uniform_desc_pool),
        }
    }

    pub fn render(&self, cmd_buffer: &mut B::CommandBuffer, scene: &SceneList<B>) {}
}

impl<B: hal::Backend> Drop for LightList<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();

        self.area = None;
        self.spot = None;
        self.dir = None;

        unsafe {
            self.device
                .destroy_pipeline_layout(ManuallyDrop::into_inner(std::ptr::read(
                    &self.pipeline_layout,
                )));
            self.uniform_desc_pool.reset();
            self.device
                .destroy_descriptor_pool(ManuallyDrop::into_inner(std::ptr::read(
                    &self.uniform_desc_pool,
                )));
            self.device
                .destroy_descriptor_set_layout(ManuallyDrop::into_inner(std::ptr::read(
                    &self.uniform_desc_layout,
                )));
        }
    }
}
