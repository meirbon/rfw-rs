use std::rc::Rc;

use ash::{
    version::DeviceV1_0,
    vk::{CommandBuffer, Handle},
    *,
};
use rfw::prelude::*;

use crate::memory::{VkAllocator, VkBuffer, VkImage};

pub struct TextureList {
    textures: Vec<Option<Rc<VkImage>>>,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    descriptor_set_layout: vk::DescriptorSetLayout,
    staging_buffer: Option<VkBuffer<u8>>,
}

impl TextureList {
    pub fn new(device: &Device, allocator: &VkAllocator) -> Self {
        // let create_info = vk::ImageCreateInfo::builder()
        //     .image_type(vk::ImageType::TYPE_2D)
        //     .format(vk::Format::D32_SFLOAT)
        //     .extent(vk::Extent3D {
        //         width: 1024,
        //         height: 1024,
        //         depth: 1,
        //     })
        //     .mip_levels(1)
        //     .array_layers(1)
        //     .samples(vk::SampleCountFlags::TYPE_1)
        //     .tiling(vk::ImageTiling::OPTIMAL)
        //     .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
        //     .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let pool_sizes = [
            vk::DescriptorPoolSize::builder()
                .descriptor_count(1024)
                .ty(vk::DescriptorType::SAMPLED_IMAGE)
                .build(),
            vk::DescriptorPoolSize::builder()
                .descriptor_count(1)
                .ty(vk::DescriptorType::SAMPLER)
                .build(),
        ];

        let descriptor_pool_create_info = vk::DescriptorPoolCreateInfo::builder()
            .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
            .max_sets(1)
            .pool_sizes(&pool_sizes)
            .build();

        let descriptor_pool =
            unsafe { device.create_descriptor_pool(&descriptor_pool_create_info, None) }.unwrap();

        let bindings = [
            vk::DescriptorSetLayoutBinding::builder()
                .binding(0)
                .descriptor_count(1)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                .build(),
            vk::DescriptorSetLayoutBinding::builder()
                .binding(1)
                .descriptor_count(1024)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                .build(),
        ];
        let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
            .bindings(&bindings)
            .build();
        let descriptor_set_layout = unsafe {
            device.create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
        }
        .unwrap();

        let allocate_info = vk::DescriptorSetAllocateInfo::builder()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&[descriptor_set_layout])
            .build();
        let descriptor_set = unsafe { device.allocate_descriptor_sets(&allocate_info).unwrap()[0] };

        Self {
            textures: Vec::new(),
            descriptor_pool,
            descriptor_set,
            descriptor_set_layout,
            staging_buffer: None,
        }
    }

    pub fn update(
        &mut self,
        device: &Device,
        cmd_buffer: CommandBuffer,
        allocator: &VkAllocator,
        textures: &[TextureData],
        changed: &BitSlice<Lsb0, usize>,
    ) {
        self.textures.resize(textures.len(), Default::default());

        let mut start_end = Vec::new();
        let mut data = Vec::new();

        for (i, t) in textures.iter().enumerate().filter(|(i, _)| changed[*i]) {
            let image = if let Some(tex) = self.textures[i].as_mut() {
                let extent = tex.extent();
                if extent.width != t.width
                    || extent.height != t.height
                    || tex.mip_levels() != t.mip_levels
                {
                    let create_info = vk::ImageCreateInfo::builder()
                        .image_type(vk::ImageType::TYPE_2D)
                        .format(vk::Format::B8G8R8A8_UNORM)
                        .extent(vk::Extent3D {
                            width: t.width,
                            height: t.height,
                            depth: 1,
                        })
                        .mip_levels(t.mip_levels)
                        .array_layers(1)
                        .samples(vk::SampleCountFlags::TYPE_1)
                        .tiling(vk::ImageTiling::OPTIMAL)
                        .usage(vk::ImageUsageFlags::SAMPLED)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE);

                    *tex = Rc::new(
                        allocator
                            .create_image(
                                &create_info,
                                vk_mem::MemoryUsage::CpuToGpu, // It is likely this texture will be updated often, switch to mappable memory
                                vk_mem::AllocationCreateFlags::NONE,
                            )
                            .unwrap(),
                    );
                }

                tex
            } else {
                let create_info = vk::ImageCreateInfo::builder()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::B8G8R8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: t.width,
                        height: t.height,
                        depth: 1,
                    })
                    .initial_layout(vk::ImageLayout::UNDEFINED)
                    .mip_levels(t.mip_levels)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .tiling(vk::ImageTiling::OPTIMAL)
                    .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE);

                self.textures[i] = Some(Rc::new(
                    allocator
                        .create_image(
                            &create_info,
                            vk_mem::MemoryUsage::GpuOnly, // Start with GPU only memory for performance
                            vk_mem::AllocationCreateFlags::NONE,
                        )
                        .unwrap(),
                ));

                self.textures[i].as_mut().unwrap()
            };

            let bytes = t.bytes;

            // use a staging buffer
            let start = data.len();
            data.reserve(bytes.len());
            for b in bytes {
                data.push(*b);
            }
            start_end.push((i, start));
        }

        if !data.is_empty() {
            let staging_buffer = if let Some(b) = self.staging_buffer.as_mut() {
                if b.len() < data.len() {
                    *b = allocator
                        .create_buffer(
                            vk::BufferUsageFlags::TRANSFER_SRC,
                            vk_mem::MemoryUsage::CpuOnly,
                            vk_mem::AllocationCreateFlags::NONE,
                            data.len().next_power_of_two(),
                        )
                        .unwrap();
                }

                b
            } else {
                self.staging_buffer = Some(
                    allocator
                        .create_buffer(
                            vk::BufferUsageFlags::TRANSFER_SRC,
                            vk_mem::MemoryUsage::CpuOnly,
                            vk_mem::AllocationCreateFlags::NONE,
                            data.len().next_power_of_two(),
                        )
                        .unwrap(),
                );

                self.staging_buffer.as_mut().unwrap()
            };

            let mut mapping = unsafe { staging_buffer.map_memory() }.unwrap();
            mapping[0..data.len()].copy_from_slice(data.as_slice());

            let mut regions = Vec::with_capacity(5);

            for (i, start) in start_end {
                let image = self.textures[i].as_ref().unwrap().image;

                let t = &textures[i];
                regions.clear();

                let image_barriers = [vk::ImageMemoryBarrier::builder()
                    .old_layout(vk::ImageLayout::UNDEFINED)
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::empty())
                    .subresource_range(
                        vk::ImageSubresourceRange::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_mip_level(0)
                            .level_count(t.mip_levels)
                            .base_array_layer(0)
                            .layer_count(1)
                            .build(),
                    )
                    .image(image)
                    .build()];

                unsafe {
                    device.cmd_pipeline_barrier(
                        cmd_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::DependencyFlags::BY_REGION,
                        &[],
                        &[],
                        &image_barriers[..],
                    );
                }

                let mut offset = 0;
                for i in 0..t.mip_levels {
                    let (w, h) = t.mip_level_width_height(i as usize);

                    regions.push(vk::BufferImageCopy {
                        buffer_offset: (start + offset) as vk::DeviceSize,
                        buffer_row_length: w as _,
                        buffer_image_height: h as _,
                        image_subresource: vk::ImageSubresourceLayers::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_array_layer(0)
                            .layer_count(1)
                            .mip_level(i)
                            .build(),
                        image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
                        image_extent: vk::Extent3D {
                            width: w as _,
                            height: h as _,
                            depth: 1,
                        },
                    });

                    offset += w * h;
                }

                unsafe {
                    device.cmd_copy_buffer_to_image(
                        cmd_buffer,
                        staging_buffer.buffer,
                        image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        regions.as_slice(),
                    );
                }

                let image_barriers = [vk::ImageMemoryBarrier::builder()
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
                    .src_access_mask(vk::AccessFlags::empty())
                    .dst_access_mask(vk::AccessFlags::empty())
                    .subresource_range(
                        vk::ImageSubresourceRange::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_mip_level(0)
                            .level_count(t.mip_levels)
                            .base_array_layer(0)
                            .layer_count(1)
                            .build(),
                    )
                    .image(image)
                    .build()];

                unsafe {
                    device.cmd_pipeline_barrier(
                        cmd_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::DependencyFlags::BY_REGION,
                        &[],
                        &[],
                        &image_barriers[..],
                    );
                }
            }
        }
    }
}
