use crate::buffer::*;
use crate::hal;
use hal::{command::CommandBuffer, device::Device, pso::DescriptorPool, *};
use rfw_scene::{Light, LightInfo, TrackedStorage};
use shared::BytesConversion;
use std::{fmt::Debug, mem::ManuallyDrop, sync::Arc};

pub struct Array<B: hal::Backend, T: Sized + Light + Clone + Debug + Default> {
    lights: TrackedStorage<T>,
    light_buffer: Buffer<B>,
    info: Vec<LightInfo>,
    shadow_maps: ShadowMapArray<B>,
}

#[allow(dead_code)]
pub struct ShadowMapArray<B: hal::Backend> {
    pub map: B::Image,
    pub view: B::ImageView,
    pub layer_views: Vec<B::ImageView>,

    filter_map: B::Image,
    filter_view: B::ImageView,
    filter_views: Vec<B::ImageView>,

    depth_map: B::Image,
    depth_view: B::ImageView,
    pub uniform_buffer: Buffer<B>,

    filter_pipeline: Arc<FilterPipeline<B>>,
    pipeline_layout: B::PipelineLayout,
    pipeline: B::GraphicsPipeline,
    anim_pipeline: B::GraphicsPipeline,
    light_infos: Vec<LightInfo>,
}

pub struct FilterPipeline<B: hal::Backend> {
    device: Arc<B::Device>,
    desc_pool: ManuallyDrop<B::DescriptorPool>,
    desc_layout: ManuallyDrop<B::DescriptorSetLayout>,
    pipeline_layout: ManuallyDrop<B::PipelineLayout>,
    pipeline: ManuallyDrop<B::ComputePipeline>,
}

#[derive(Debug, Copy, Clone)]
struct FilterPushConstant {
    direction: [f32; 2],
    layer: u32,
}

impl<'a> FilterPushConstant {
    pub fn as_bytes(&'a self) -> &'a [u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const FilterPushConstant as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }

    pub fn as_quad_bytes(&'a self) -> &'a [u32] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const FilterPushConstant as *const u32,
                std::mem::size_of::<Self>() / 4,
            )
        }
    }
}

impl<B: hal::Backend> FilterPipeline<B> {
    pub fn new(device: Arc<B::Device>) -> Self {
        unsafe {
            // 4 Types of lights, thus max 4 sets
            let desc_pool = device
                .create_descriptor_pool(
                    8,
                    &[
                        pso::DescriptorRangeDesc {
                            count: 8,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: false },
                            },
                        },
                        pso::DescriptorRangeDesc {
                            count: 8,
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: true },
                            },
                        },
                    ],
                    pso::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET,
                )
                .unwrap();
            let desc_layout = device
                .create_descriptor_set_layout(
                    &[
                        pso::DescriptorSetLayoutBinding {
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: false },
                            },
                            binding: 0,
                            count: 1,
                            immutable_samplers: false,
                            stage_flags: pso::ShaderStageFlags::COMPUTE,
                        },
                        pso::DescriptorSetLayoutBinding {
                            ty: pso::DescriptorType::Image {
                                ty: pso::ImageDescriptorType::Storage { read_only: true },
                            },
                            binding: 1,
                            count: 1,
                            immutable_samplers: false,
                            stage_flags: pso::ShaderStageFlags::COMPUTE,
                        },
                    ],
                    &[],
                )
                .unwrap();

            let pipeline_layout = device
                .create_pipeline_layout(
                    std::iter::once(&desc_layout),
                    std::iter::once(&(pso::ShaderStageFlags::COMPUTE, 0..12)),
                )
                .unwrap();

            let spirv = include_bytes!("../../shaders/shadow_filter.comp.spv");
            let shader = device.create_shader_module(spirv.as_quad_bytes()).unwrap();

            let pipeline = device
                .create_compute_pipeline(
                    &pso::ComputePipelineDesc {
                        shader: pso::EntryPoint {
                            entry: "main",
                            module: &shader,
                            specialization: pso::Specialization::EMPTY,
                        },
                        layout: &pipeline_layout,
                        flags: pso::PipelineCreationFlags::empty(),
                        parent: pso::BasePipeline::None,
                    },
                    None,
                )
                .unwrap();

            Self {
                device,
                desc_pool: ManuallyDrop::new(desc_pool),
                desc_layout: ManuallyDrop::new(desc_layout),
                pipeline_layout: ManuallyDrop::new(pipeline_layout),
                pipeline: ManuallyDrop::new(pipeline),
            }
        }
    }

    pub fn launch(&self, cmd_buffer: &mut B::CommandBuffer, set: &FilterDescSet<B>, layer: u32) {
        unsafe {
            // Filter in x direction
            let push_constants = FilterPushConstant {
                direction: [1.0, 0.0],
                layer,
            };
            cmd_buffer.bind_compute_pipeline(&*self.pipeline);
            cmd_buffer.bind_compute_descriptor_sets(
                &self.pipeline_layout,
                0,
                std::iter::once(&set.set1),
                &[],
            );
            cmd_buffer.push_compute_constants(
                &self.pipeline_layout,
                0,
                push_constants.as_quad_bytes(),
            );
            cmd_buffer.dispatch([
                (set.width as f32 / 16.0).ceil() as u32,
                (set.height as f32 / 16.0_f32).ceil() as u32,
                1,
            ]);

            // Filter in y direction
            let push_constants = FilterPushConstant {
                direction: [0.0, 1.0],
                layer,
            };
            cmd_buffer.bind_compute_descriptor_sets(
                &self.pipeline_layout,
                0,
                std::iter::once(&set.set2),
                &[],
            );
            cmd_buffer.push_compute_constants(
                &self.pipeline_layout,
                0,
                push_constants.as_quad_bytes(),
            );
            cmd_buffer.dispatch([
                (set.width as f32 / 16.0).ceil() as u32,
                (set.height as f32 / 16.0_f32).ceil() as u32,
                1,
            ]);
        }
    }

    pub fn allocate_set(
        &mut self,
        width: u32,
        height: u32,
        source_output: &B::ImageView,
        filter_view: &B::ImageView,
    ) -> FilterDescSet<B> {
        unsafe {
            let set1 = self.desc_pool.allocate_set(&self.desc_layout).unwrap();
            let set2 = self.desc_pool.allocate_set(&self.desc_layout).unwrap();

            let writes = vec![
                pso::DescriptorSetWrite {
                    binding: 0,
                    array_offset: 0,
                    descriptors: vec![
                        pso::Descriptor::Image(filter_view, image::Layout::General),
                        pso::Descriptor::Image(source_output, image::Layout::General),
                    ],
                    set: &set1,
                },
                pso::DescriptorSetWrite {
                    binding: 0,
                    array_offset: 0,
                    descriptors: vec![
                        pso::Descriptor::Image(source_output, image::Layout::General),
                        pso::Descriptor::Image(filter_view, image::Layout::General),
                    ],
                    set: &set2,
                },
            ];

            self.device.write_descriptor_sets(writes);

            FilterDescSet {
                width,
                height,
                set1,
                set2,
            }
        }
    }

    pub fn update_set(
        &mut self,
        source_output: &B::ImageView,
        filter_view: &B::ImageView,
        set: &FilterDescSet<B>,
    ) {
        let writes = vec![
            pso::DescriptorSetWrite {
                binding: 0,
                array_offset: 0,
                descriptors: vec![
                    pso::Descriptor::Image(filter_view, image::Layout::General),
                    pso::Descriptor::Image(source_output, image::Layout::General),
                ],
                set: &set.set1,
            },
            pso::DescriptorSetWrite {
                binding: 0,
                array_offset: 0,
                descriptors: vec![
                    pso::Descriptor::Image(source_output, image::Layout::General),
                    pso::Descriptor::Image(filter_view, image::Layout::General),
                ],
                set: &set.set2,
            },
        ];

        unsafe { self.device.write_descriptor_sets(writes) };
    }

    pub fn free_set(&mut self, set: FilterDescSet<B>) {
        let sets = vec![set.set1, set.set2];

        unsafe {
            self.desc_pool.free_sets(sets);
        }
    }
}

pub struct FilterDescSet<B: hal::Backend> {
    width: u32,
    height: u32,
    set1: B::DescriptorSet,
    set2: B::DescriptorSet,
}
