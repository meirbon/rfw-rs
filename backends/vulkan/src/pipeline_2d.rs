use ash::util::read_spv;
use ash::version::DeviceV1_0;
use ash::*;
use bytemuck::*;
use rfw::backend::{JointData, Vertex3D};
use std::ffi::CString;
use std::io::Cursor;

pub struct RenderPipeline2D {
    pipeline: vk::Pipeline,
    pipeline_layout: vk::PipelineLayout,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: Option<vk::DescriptorSet>,
    renderpass: vk::RenderPass,
}

impl RenderPipeline2D {
    pub fn new(device: &Device, width: u32, height: u32, surface_format: vk::Format) -> Self {
        let shader_entry_name = CString::new("main").unwrap();
        let mut vertex_spv_file = Cursor::new(&include_bytes!("../shaders/2d.vert.spv")[..]);
        let mut frag_spv_file = Cursor::new(&include_bytes!("../shaders/2d.frag.spv")[..]);

        let vertex_code = read_spv(&mut vertex_spv_file).expect("Failed to read vertex shader spv");
        let vertex_shader_info = vk::ShaderModuleCreateInfo::builder().code(vertex_code.as_slice());

        let frag_code = read_spv(&mut frag_spv_file).expect("Failed to read frag shader spv");
        let frag_shader_info = vk::ShaderModuleCreateInfo::builder().code(frag_code.as_slice());

        unsafe {
            let bindings = [
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(0)
                    .descriptor_count(1)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                    .build(),
                vk::DescriptorSetLayoutBinding::builder()
                    .binding(1)
                    .descriptor_count(1)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                    .build(),
                // vk::DescriptorSetLayoutBinding::builder()
                //     .binding(2)
                //     .descriptor_count(1)
                //     .descriptor_type(vk::DescriptorType::SAMPLER)
                //     .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                //     .build(),
                // vk::DescriptorSetLayoutBinding::builder()
                //     .binding(3)
                //     .descriptor_count(1024)
                //     .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE),
                // vk::DescriptorSetLayoutBinding::builder()
                //     .binding(4)
                //     .descriptor_count(1)
                //     .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                //     .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                //     .build(),
                // vk::DescriptorSetLayoutBinding::builder()
                //     .binding(5)
                //     .descriptor_count(1)
                //     .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                //     .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                //     .build(),
                // vk::DescriptorSetLayoutBinding::builder()
                //     .binding(6)
                //     .descriptor_count(1)
                //     .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                //     .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                //     .build(),
                // vk::DescriptorSetLayoutBinding::builder()
                //     .binding(7)
                //     .descriptor_count(1)
                //     .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                //     .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT)
                //     .build(),
            ];
            let descriptor_set_layout_create_info = vk::DescriptorSetLayoutCreateInfo::builder()
                .bindings(&bindings)
                .build();
            let descriptor_set_layout = device
                .create_descriptor_set_layout(&descriptor_set_layout_create_info, None)
                .unwrap();

            let pool_sizes = [
                vk::DescriptorPoolSize::builder()
                    .descriptor_count(2)
                    .ty(vk::DescriptorType::UNIFORM_BUFFER)
                    .build(),
                vk::DescriptorPoolSize::builder()
                    .descriptor_count(2)
                    .ty(vk::DescriptorType::STORAGE_BUFFER)
                    .build(),
            ];
            let descriptor_pool_create_info = vk::DescriptorPoolCreateInfo::builder()
                .flags(vk::DescriptorPoolCreateFlags::FREE_DESCRIPTOR_SET)
                .max_sets(2)
                .pool_sizes(&pool_sizes)
                .build();

            let descriptor_pool = device
                .create_descriptor_pool(&descriptor_pool_create_info, None)
                .unwrap();

            let layouts = [descriptor_set_layout];
            let layout_create_info = vk::PipelineLayoutCreateInfo::builder().set_layouts(&layouts);
            let pipeline_layout = device
                .create_pipeline_layout(&layout_create_info, None)
                .unwrap();

            let renderpass_attachments = [
                vk::AttachmentDescription {
                    format: surface_format,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    store_op: vk::AttachmentStoreOp::STORE,
                    final_layout: vk::ImageLayout::PRESENT_SRC_KHR,
                    ..Default::default()
                },
                vk::AttachmentDescription {
                    format: vk::Format::D32_SFLOAT,
                    samples: vk::SampleCountFlags::TYPE_1,
                    load_op: vk::AttachmentLoadOp::CLEAR,
                    initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    ..Default::default()
                },
            ];
            let color_attachment_refs = [vk::AttachmentReference {
                attachment: 0,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            }];
            let depth_attachment_ref = vk::AttachmentReference {
                attachment: 1,
                layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            };
            let dependencies = [vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ
                    | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                ..Default::default()
            }];

            let subpasses = [vk::SubpassDescription::builder()
                .color_attachments(&color_attachment_refs)
                .depth_stencil_attachment(&depth_attachment_ref)
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .build()];

            let renderpass_create_info = vk::RenderPassCreateInfo::builder()
                .attachments(&renderpass_attachments)
                .subpasses(&subpasses)
                .dependencies(&dependencies);

            let renderpass = device
                .create_render_pass(&renderpass_create_info, None)
                .unwrap();

            let vertex_module = device
                .create_shader_module(&vertex_shader_info, None)
                .unwrap();
            let frag_module = device
                .create_shader_module(&frag_shader_info, None)
                .unwrap();

            let shader_stage_create_infos = [
                vk::PipelineShaderStageCreateInfo {
                    module: vertex_module,
                    p_name: shader_entry_name.as_ptr(),
                    stage: vk::ShaderStageFlags::VERTEX,
                    ..Default::default()
                },
                vk::PipelineShaderStageCreateInfo {
                    module: frag_module,
                    p_name: shader_entry_name.as_ptr(),
                    stage: vk::ShaderStageFlags::FRAGMENT,
                    ..Default::default()
                },
            ];

            let vertex_input_binding_descriptions = [vk::VertexInputBindingDescription {
                binding: 0,
                stride: std::mem::size_of::<Vertex3D>() as u32,
                input_rate: vk::VertexInputRate::VERTEX,
            }];
            let vertex_input_attribute_descriptions = [
                vk::VertexInputAttributeDescription {
                    location: 0,
                    binding: 0,
                    format: vk::Format::R32G32B32A32_SFLOAT,
                    offset: 0,
                },
                vk::VertexInputAttributeDescription {
                    location: 1,
                    binding: 0,
                    format: vk::Format::R32G32B32_SFLOAT,
                    offset: 16,
                },
                vk::VertexInputAttributeDescription {
                    location: 2,
                    binding: 0,
                    format: vk::Format::R32_UINT,
                    offset: 28,
                },
                vk::VertexInputAttributeDescription {
                    location: 3,
                    binding: 0,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: 32,
                },
                vk::VertexInputAttributeDescription {
                    location: 4,
                    binding: 0,
                    format: vk::Format::R32G32B32A32_SFLOAT,
                    offset: 40,
                },
            ];

            let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo {
                vertex_attribute_description_count: vertex_input_attribute_descriptions.len()
                    as u32,
                p_vertex_attribute_descriptions: vertex_input_attribute_descriptions.as_ptr(),
                vertex_binding_description_count: vertex_input_binding_descriptions.len() as u32,
                p_vertex_binding_descriptions: vertex_input_binding_descriptions.as_ptr(),
                ..Default::default()
            };
            let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo {
                topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                ..Default::default()
            };
            let viewports = [vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: width as f32,
                height: height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            }];
            let scissors = [vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D { width, height },
            }];
            let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
                .scissors(&scissors)
                .viewports(&viewports);

            let rasterization_info = vk::PipelineRasterizationStateCreateInfo {
                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                line_width: 1.0,
                polygon_mode: vk::PolygonMode::FILL,
                cull_mode: vk::CullModeFlags::NONE,
                ..Default::default()
            };
            let multisample_state_info = vk::PipelineMultisampleStateCreateInfo {
                rasterization_samples: vk::SampleCountFlags::TYPE_1,
                ..Default::default()
            };
            let noop_stencil_state = vk::StencilOpState {
                fail_op: vk::StencilOp::KEEP,
                pass_op: vk::StencilOp::KEEP,
                depth_fail_op: vk::StencilOp::KEEP,
                compare_op: vk::CompareOp::ALWAYS,
                ..Default::default()
            };
            let depth_state_info = vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: 1,
                depth_write_enable: 1,
                depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
                front: noop_stencil_state,
                back: noop_stencil_state,
                max_depth_bounds: 1.0,
                ..Default::default()
            };
            let color_blend_attachment_states = [vk::PipelineColorBlendAttachmentState {
                blend_enable: 0,
                src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,
                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ZERO,
                dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                alpha_blend_op: vk::BlendOp::ADD,
                color_write_mask: vk::ColorComponentFlags::all(),
            }];
            let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
                .logic_op(vk::LogicOp::CLEAR)
                .attachments(&color_blend_attachment_states);

            let dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state_info =
                vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state);

            let graphic_pipeline_info = vk::GraphicsPipelineCreateInfo::builder()
                .stages(&shader_stage_create_infos)
                .vertex_input_state(&vertex_input_state_info)
                .input_assembly_state(&vertex_input_assembly_state_info)
                .viewport_state(&viewport_state_info)
                .rasterization_state(&rasterization_info)
                .multisample_state(&multisample_state_info)
                .depth_stencil_state(&depth_state_info)
                .color_blend_state(&color_blend_state)
                .dynamic_state(&dynamic_state_info)
                .layout(pipeline_layout)
                .render_pass(renderpass);

            let graphics_pipeline = device
                .create_graphics_pipelines(
                    vk::PipelineCache::null(),
                    &[graphic_pipeline_info.build()],
                    None,
                )
                .expect("Unable to create graphics pipeline")[0];

            device.destroy_shader_module(vertex_module, None);
            device.destroy_shader_module(frag_module, None);

            Self {
                pipeline: graphics_pipeline,
                pipeline_layout,
                descriptor_set_layout,
                descriptor_pool,
                descriptor_set: None,
                renderpass,
            }
        }
    }

    pub fn update_descriptor_set(
        &mut self,
        device: &Device,
        camera_buffer: vk::Buffer,
        instance_buffer: vk::Buffer,
    ) {
        let set = if let Some(set) = self.descriptor_set {
            set
        } else {
            let allocate_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(self.descriptor_pool)
                .set_layouts(&[self.descriptor_set_layout])
                .build();
            let set = unsafe { device.allocate_descriptor_sets(&allocate_info).unwrap()[0] };

            self.descriptor_set = Some(set);
            set
        };

        let descriptor_writes = [
            vk::WriteDescriptorSet::builder()
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .dst_binding(0)
                .dst_array_element(0)
                .dst_set(set)
                .buffer_info(&[vk::DescriptorBufferInfo::builder()
                    .buffer(camera_buffer)
                    .offset(0)
                    .range(vk::WHOLE_SIZE)
                    .build()])
                .build(),
            vk::WriteDescriptorSet::builder()
                .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                .dst_binding(1)
                .dst_array_element(0)
                .dst_set(set)
                .buffer_info(&[vk::DescriptorBufferInfo::builder()
                    .buffer(instance_buffer)
                    .offset(0)
                    .range(vk::WHOLE_SIZE)
                    .build()])
                .build(),
        ];

        unsafe {
            device.update_descriptor_sets(&descriptor_writes, &[]);
        }
    }

    pub fn render(
        &self,
        width: u32,
        height: u32,
        device: &Device,
        cmd_buffer: vk::CommandBuffer,
        framebuffer: vk::Framebuffer,
        vertices: &crate::VertexList<Vertex3D, JointData>,
        instances: &crate::InstanceList<crate::InstanceTransform>,
    ) {
        if self.descriptor_set.is_none() {
            eprintln!("desc_set is none");
            return;
        } else if vertices.get_vertex_buffer().is_none() {
            eprintln!("vertices is none");
            return;
        }

        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 1.0,
                    stencil: 0,
                },
            },
        ];

        let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
            .render_pass(self.renderpass)
            .framebuffer(framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D { width, height },
            })
            .clear_values(&clear_values);

        let viewports = [vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: width as f32,
            height: height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        }];
        let scissors = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D { width, height },
        }];
        unsafe {
            device.cmd_begin_render_pass(
                cmd_buffer,
                &render_pass_begin_info,
                vk::SubpassContents::INLINE,
            );

            device.cmd_bind_pipeline(cmd_buffer, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            device.cmd_set_viewport(cmd_buffer, 0, &viewports);
            device.cmd_set_scissor(cmd_buffer, 0, &scissors);
            device.cmd_bind_descriptor_sets(
                cmd_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout,
                0,
                &[self.descriptor_set.unwrap()],
                &[],
            );
            device.cmd_bind_vertex_buffers(
                cmd_buffer,
                0,
                &[vertices.get_vertex_buffer().unwrap()],
                &[0],
            );

            let vertex_ranges = vertices.get_ranges();
            let instance_ranges = instances.get_ranges();

            // device.cmd_draw(cmd_buffer, vertices.len() as u32, 1, 0, 0);
            for (id, instance) in instance_ranges.iter() {
                let vertex_range = vertex_ranges.get(id).unwrap();
                device.cmd_draw(
                    cmd_buffer,
                    vertex_range.end - vertex_range.start,
                    instance.count,
                    vertex_range.start,
                    instance.start,
                );
                // println!(
                //     "Drawing {} ({}..{}) instances for {} vertices {}..{}",
                //     instance.count,
                //     instance.start,
                //     instance.end,
                //     vertex_range.end - vertex_range.start,
                //     vertex_range.start,
                //     vertex_range.end
                // );
            }

            device.cmd_end_render_pass(cmd_buffer);
        }
    }

    pub unsafe fn destroy(&mut self, device: &Device) {
        device.destroy_pipeline(self.pipeline, None);
        device.destroy_pipeline_layout(self.pipeline_layout, None);
        device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        device.destroy_descriptor_pool(self.descriptor_pool, None);
        device.destroy_render_pass(self.renderpass, None);
    }
}
