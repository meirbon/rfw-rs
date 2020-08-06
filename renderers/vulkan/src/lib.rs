use std::error::Error;
use std::{
    ffi::{CStr, CString},
    fmt::Display,
};

use ash::extensions::{
    ext::DebugUtils,
    khr::{Surface, Swapchain},
};
use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::{vk, Entry};

use ash::util::read_spv;
use helpers::*;
use rfw_scene::{
    graph::Skin,
    raw_window_handle::HasRawWindowHandle,
    renderers::{RenderMode, Renderer, Setting},
    AreaLight, BitVec, Camera, DeviceMaterial, DirectionalLight, Instance, Local, Material, Mesh,
    PointLight, SpotLight, Texture,
};
use std::io::Cursor;

mod helpers;

#[derive(Debug, Copy, Clone)]
pub enum RendererError {
    Unknown,
}

impl Error for RendererError {}

impl Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "")
    }
}

pub struct VkRenderer {
    pub entry: ash::Entry,
    pub instance: ash::Instance,
    pub device: ash::Device,
    pub surface_loader: Surface,
    pub swapchain_loader: Swapchain,
    pub debug_utils_loader: DebugUtils,
    pub debug_call_back: vk::DebugUtilsMessengerEXT,
    pub pdevice: vk::PhysicalDevice,
    pub device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    pub queue_family_index: u32,
    pub present_queue: vk::Queue,

    pub surface: vk::SurfaceKHR,
    pub surface_format: vk::SurfaceFormatKHR,
    pub surface_resolution: vk::Extent2D,

    pub swapchain: vk::SwapchainKHR,
    pub swapchain_create_info: vk::SwapchainCreateInfoKHR,
    pub present_images: Vec<vk::Image>,
    pub present_image_views: Vec<vk::ImageView>,

    pub pool: vk::CommandPool,
    pub draw_command_buffer: vk::CommandBuffer,
    pub setup_command_buffer: vk::CommandBuffer,

    pub depth_image: vk::Image,
    pub depth_image_view: vk::ImageView,
    pub depth_image_memory: vk::DeviceMemory,

    pub present_complete_semaphore: vk::Semaphore,
    pub rendering_complete_semaphore: vk::Semaphore,

    pub pipeline_cache: vk::PipelineCache,
    pub debug_pipeline_layout: vk::PipelineLayout,
    pub debug_pipeline: vk::Pipeline,
    pub render_pass: vk::RenderPass,
    pub scissor: vk::Rect2D,
    pub framebuffers: Vec<vk::Framebuffer>,
}

impl VkRenderer {
    pub const DEPTH_FORMAT: vk::Format = vk::Format::D16_UNORM;
}

impl Renderer for VkRenderer {
    fn init<T: HasRawWindowHandle>(
        window: &T,
        width: usize,
        height: usize,
    ) -> Result<Box<Self>, Box<dyn Error>> {
        unsafe {
            let entry = Entry::new().unwrap();
            let app_name = CString::new("Vulkan Renderer").unwrap();
            let engine_name = CString::new("rfw-rs").unwrap();

            let layer_names = [CString::new("VK_LAYER_KHRONOS_validation")?];
            let layers_names_raw: Vec<*const i8> = layer_names
                .iter()
                .map(|raw_name| raw_name.as_ptr())
                .collect();

            let mut extension_names: Vec<&CStr> =
                ash_window::enumerate_required_extensions(window).unwrap();
            extension_names.push(ash::extensions::ext::DebugUtils::name());

            let extension_names_raw: Vec<*const i8> =
                extension_names.iter().map(|s| s.as_ptr()).collect();

            let app_info = vk::ApplicationInfo::builder()
                .application_name(&app_name)
                .application_version(0)
                .engine_name(&engine_name)
                .engine_version(0)
                .api_version(vk::make_version(1, 1, 0))
                .build();

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names_raw);
            let instance: ash::Instance = entry.create_instance(&create_info, None).unwrap();

            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING,
                )
                .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
                .pfn_user_callback(Some(vulkan_debug_callback));

            let debug_utils_loader = DebugUtils::new(&entry, &instance);
            let debug_call_back = debug_utils_loader
                .create_debug_utils_messenger(&debug_info, None)
                .unwrap();

            let surface: vk::SurfaceKHR =
                ash_window::create_surface(&entry, &instance, window, None).unwrap();
            let pdevices = instance
                .enumerate_physical_devices()
                .expect("Could not retrieve physical devices");
            let surface_loader = Surface::new(&entry, &instance);
            let (pdevice, queue_family_index) = pdevices
                .iter()
                .map(|pdevice| {
                    instance
                        .get_physical_device_queue_family_properties(*pdevice)
                        .iter()
                        .enumerate()
                        .filter_map(|(index, ref info)| {
                            let supports_graphic_and_surface =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                                    && surface_loader
                                        .get_physical_device_surface_support(
                                            *pdevice,
                                            index as u32,
                                            surface,
                                        )
                                        .unwrap();
                            if supports_graphic_and_surface {
                                Some((*pdevice, index))
                            } else {
                                None
                            }
                        })
                        .next()
                })
                .filter_map(|v| v)
                .next()
                .expect("Could not find suitable device");

            let queue_family_index = queue_family_index as u32;
            let device_extension_names_raw = [Swapchain::name().as_ptr()];
            let features = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                ..Default::default()
            };
            let priorities = [1.0];

            let queue_info = [vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(queue_family_index)
                .queue_priorities(&priorities)
                .build()];

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&device_extension_names_raw)
                .enabled_features(&features);

            let props = instance.get_physical_device_properties(pdevice);
            let device_name = CStr::from_ptr(props.device_name.as_ptr());
            println!(
                "Picked device: {}",
                device_name.to_str().unwrap().to_string()
            );

            let device = instance
                .create_device(pdevice, &device_create_info, None)
                .unwrap();

            let present_queue = device.get_device_queue(queue_family_index as u32, 0);

            let surface_formats = surface_loader
                .get_physical_device_surface_formats(pdevice, surface)
                .unwrap();

            let surface_format = surface_formats
                .iter()
                .map(|sfmt| match sfmt.format {
                    vk::Format::UNDEFINED => vk::SurfaceFormatKHR {
                        format: vk::Format::B8G8R8_UNORM,
                        color_space: sfmt.color_space,
                    },
                    _ => *sfmt,
                })
                .next()
                .expect("Could not find suitable surface format");

            let surface_capabilities = surface_loader
                .get_physical_device_surface_capabilities(pdevice, surface)
                .unwrap();
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }
            let surface_resolution = match surface_capabilities.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: width as u32,
                    height: height as u32,
                },
                _ => surface_capabilities.current_extent,
            };
            let pre_transform = if surface_capabilities
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capabilities.current_transform
            };
            let present_modes = surface_loader
                .get_physical_device_surface_present_modes(pdevice, surface)
                .unwrap();
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::FIFO)
                .unwrap_or(vk::PresentModeKHR::FIFO);
            let swapchain_loader = Swapchain::new(&instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(desired_image_count)
                .image_color_space(surface_format.color_space)
                .image_format(surface_format.format)
                .image_extent(surface_resolution)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1)
                .build();

            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            let pool_create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(queue_family_index);

            let pool = device.create_command_pool(&pool_create_info, None).unwrap();

            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_buffer_count(2)
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY);

            let command_buffers = device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap();
            let setup_command_buffer = command_buffers[0];
            let draw_command_buffer = command_buffers[1];

            let present_images = swapchain_loader.get_swapchain_images(swapchain).unwrap();
            let present_image_views: Vec<vk::ImageView> = present_images
                .iter()
                .map(|&image| {
                    let create_view_info = vk::ImageViewCreateInfo::builder()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(
                            vk::ImageSubresourceRange::builder()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .base_mip_level(0)
                                .base_array_layer(0)
                                .level_count(1)
                                .layer_count(1)
                                .build(),
                        )
                        .image(image);
                    device.create_image_view(&create_view_info, None).unwrap()
                })
                .collect();

            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);
            let depth_image_create_info = vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(Self::DEPTH_FORMAT)
                .extent(vk::Extent3D {
                    width: surface_resolution.width,
                    height: surface_resolution.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            let depth_image = device.create_image(&depth_image_create_info, None).unwrap();
            let depth_image_memory_req = device.get_image_memory_requirements(depth_image);
            let depth_image_memory_index = find_memorytype_index(
                &depth_image_memory_req,
                &device_memory_properties,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .expect("Unable to find suitable memory index for depth image.");

            let depth_image_allocate_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(depth_image_memory_req.size)
                .memory_type_index(depth_image_memory_index)
                .build();

            let depth_image_memory = device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap();

            device
                .bind_image_memory(depth_image, depth_image_memory, 0)
                .unwrap();

            let depth_image_view_info = vk::ImageViewCreateInfo::builder()
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1)
                        .build(),
                )
                .image(depth_image)
                .format(depth_image_create_info.format)
                .view_type(vk::ImageViewType::TYPE_2D);

            let depth_image_view = device
                .create_image_view(&depth_image_view_info, None)
                .unwrap();

            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            let present_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            let rendering_complete_semaphore = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();

            let pipeline_layout_create_info = vk::PipelineLayoutCreateInfo::default();
            let debug_pipeline_layout = device
                .create_pipeline_layout(&pipeline_layout_create_info, None)
                .unwrap();

            let mut vert_module = Cursor::new(&include_bytes!("../shaders/quad.vert.spv")[..]);
            let mut frag_module = Cursor::new(&include_bytes!("../shaders/quad.frag.spv")[..]);

            let vertex_code = read_spv(&mut vert_module).expect("Vertex shader module error");
            let frag_code = read_spv(&mut frag_module).expect("Fragment shader module error");

            let pipeline_cache = device
                .create_pipeline_cache(
                    &vk::PipelineCacheCreateInfo::builder()
                        .initial_data(&[])
                        .build(),
                    None,
                )
                .unwrap();

            let vert_create_info = vk::ShaderModuleCreateInfo::builder()
                .code(&vertex_code)
                .build();
            let vert_module = device
                .create_shader_module(&vert_create_info, None)
                .unwrap();

            let frag_create_info = vk::ShaderModuleCreateInfo::builder()
                .code(&frag_code)
                .build();
            let frag_module = device
                .create_shader_module(&frag_create_info, None)
                .unwrap();

            let attachment_info = [
                vk::AttachmentDescription::builder()
                    .format(surface_format.format)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                    .build(),
                vk::AttachmentDescription::builder()
                    .format(Self::DEPTH_FORMAT)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                    .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .initial_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                    .build(),
            ];

            let dependencies = [vk::SubpassDependency {
                src_subpass: vk::SUBPASS_EXTERNAL,
                src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ
                    | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                ..Default::default()
            }];

            let color_attachments = [vk::AttachmentReference::builder()
                .attachment(0)
                .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .build()];

            let depth_attachment = vk::AttachmentReference::builder()
                .attachment(1)
                .layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                .build();

            let sub_pass = vk::SubpassDescription::builder()
                .color_attachments(&color_attachments)
                .depth_stencil_attachment(&depth_attachment)
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
                .build();

            let subpasses = [sub_pass];
            let render_pass = vk::RenderPassCreateInfo::builder()
                .attachments(&attachment_info)
                .subpasses(&subpasses)
                .dependencies(&dependencies)
                .build();

            let render_pass = device.create_render_pass(&render_pass, None).unwrap();

            let entry_name = CString::new("main").unwrap();
            let scissor = vk::Rect2D::builder()
                .extent(vk::Extent2D {
                    width: surface_resolution.width as u32,
                    height: surface_resolution.height as u32,
                })
                .offset(vk::Offset2D { x: 0, y: 0 })
                .build();
            let dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];

            let stages = [
                vk::PipelineShaderStageCreateInfo::builder()
                    .module(vert_module)
                    .name(entry_name.as_c_str())
                    .stage(vk::ShaderStageFlags::VERTEX)
                    .build(),
                vk::PipelineShaderStageCreateInfo::builder()
                    .module(frag_module)
                    .name(entry_name.as_c_str())
                    .stage(vk::ShaderStageFlags::FRAGMENT)
                    .build(),
            ];

            let scissors = [scissor];

            let viewports = [vk::Viewport::builder()
                .x(0.0)
                .y(0.0)
                .width(surface_resolution.width as f32)
                .height(surface_resolution.height as f32)
                .min_depth(0.0)
                .max_depth(1.0)
                .build()];
            let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
                .scissors(&scissors)
                .viewports(&viewports);
            let rasterization_state = vk::PipelineRasterizationStateCreateInfo::builder()
                .polygon_mode(vk::PolygonMode::FILL)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                .cull_mode(vk::CullModeFlags::NONE)
                .line_width(1.0)
                .build();
            let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1)
                .build();
            let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo {
                depth_test_enable: 1,
                depth_write_enable: 1,
                depth_compare_op: vk::CompareOp::LESS,
                front: vk::StencilOpState {
                    fail_op: vk::StencilOp::KEEP,
                    pass_op: vk::StencilOp::KEEP,
                    depth_fail_op: vk::StencilOp::KEEP,
                    compare_op: vk::CompareOp::ALWAYS,
                    ..Default::default()
                },
                back: vk::StencilOpState {
                    fail_op: vk::StencilOp::KEEP,
                    pass_op: vk::StencilOp::KEEP,
                    depth_fail_op: vk::StencilOp::KEEP,
                    compare_op: vk::CompareOp::ALWAYS,
                    ..Default::default()
                },
                min_depth_bounds: 0.0,
                max_depth_bounds: 1.0,
                ..Default::default()
            };
            let attachments = [vk::PipelineColorBlendAttachmentState {
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
                .attachments(&attachments);

            let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo::builder()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
                .build();

            let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::builder()
                .vertex_attribute_descriptions(&[])
                .vertex_binding_descriptions(&[])
                .build();

            let dynamic_state_info =
                vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state);

            let debug_create_info = vk::GraphicsPipelineCreateInfo::builder()
                .stages(&stages)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterization_state)
                .multisample_state(&multisample_state)
                .depth_stencil_state(&depth_stencil_state)
                .color_blend_state(&color_blend_state)
                .render_pass(render_pass)
                .layout(debug_pipeline_layout)
                .input_assembly_state(&input_assembly_state)
                .vertex_input_state(&vertex_input_state)
                .dynamic_state(&dynamic_state_info)
                .build();

            let debug_create_infos = [debug_create_info];
            let debug_pipeline = device
                .create_graphics_pipelines(pipeline_cache, &debug_create_infos, None)
                .unwrap()[0];

            record_submit_commandbuffer(
                &device,
                setup_command_buffer,
                present_queue,
                &[],
                &[],
                &[],
                |device, command_buffer| {
                    let layout_transition_barriers = vk::ImageMemoryBarrier::builder()
                        .image(depth_image)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .subresource_range(
                            vk::ImageSubresourceRange::builder()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .layer_count(1)
                                .level_count(1)
                                .build(),
                        );

                    device.cmd_pipeline_barrier(
                        command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barriers.build()],
                    );
                },
            );

            let framebuffers: Vec<vk::Framebuffer> = present_image_views
                .iter()
                .map(|&present_image_view| {
                    let framebuffer_attachments = [present_image_view, depth_image_view];
                    let frame_buffer_create_info = vk::FramebufferCreateInfo::builder()
                        .render_pass(render_pass)
                        .attachments(&framebuffer_attachments)
                        .width(surface_resolution.width)
                        .height(surface_resolution.height)
                        .layers(1);

                    device
                        .create_framebuffer(&frame_buffer_create_info, None)
                        .unwrap()
                })
                .collect();

            Ok(Box::new(Self {
                entry,
                instance,
                device,
                surface_loader,
                swapchain_loader,
                debug_utils_loader,
                debug_call_back,
                pdevice,
                device_memory_properties,
                queue_family_index,
                present_queue,
                surface,
                surface_format,
                surface_resolution,
                swapchain,
                swapchain_create_info,
                present_images,
                present_image_views,
                pool,
                draw_command_buffer,
                setup_command_buffer,
                depth_image,
                depth_image_view,
                depth_image_memory,
                present_complete_semaphore,
                rendering_complete_semaphore,
                pipeline_cache,
                debug_pipeline_layout,
                debug_pipeline,
                render_pass,
                scissor,
                framebuffers,
            }))
        }
    }

    fn set_mesh(&mut self, _id: usize, _mesh: &Mesh) {}

    fn set_instance(&mut self, _id: usize, _instance: &Instance) {}

    fn set_materials(&mut self, _materials: &[Material], _device_materials: &[DeviceMaterial]) {}

    fn set_textures(&mut self, _textures: &[Texture]) {}

    fn synchronize(&mut self) {}

    fn render(&mut self, _camera: &Camera, _mode: RenderMode) {
        unsafe {
            let (present_index, _) = self
                .swapchain_loader
                .acquire_next_image(
                    self.swapchain,
                    std::u64::MAX,
                    self.present_complete_semaphore,
                    vk::Fence::null(),
                )
                .unwrap();

            record_submit_commandbuffer(
                &self.device,
                self.draw_command_buffer,
                self.present_queue,
                &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
                &[self.present_complete_semaphore],
                &[self.rendering_complete_semaphore],
                |device, command_buffer| {
                    let render_pass_info = vk::RenderPassBeginInfo::builder()
                        .clear_values(&[
                            vk::ClearValue {
                                color: vk::ClearColorValue {
                                    float32: [0.0, 0.0, 0.0, 0.0],
                                },
                            },
                            vk::ClearValue {
                                depth_stencil: vk::ClearDepthStencilValue {
                                    depth: 1.0,
                                    stencil: 0,
                                },
                            },
                        ])
                        .framebuffer(self.framebuffers[present_index as usize])
                        .render_pass(self.render_pass)
                        .render_area(vk::Rect2D {
                            offset: vk::Offset2D { x: 0, y: 0 },
                            extent: self.surface_resolution,
                        })
                        .build();

                    device.cmd_begin_render_pass(
                        command_buffer,
                        &render_pass_info,
                        vk::SubpassContents::INLINE,
                    );

                    device.cmd_bind_pipeline(
                        command_buffer,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.debug_pipeline,
                    );

                    let viewports = [vk::Viewport {
                        x: 0.0,
                        y: 0.0,
                        width: self.surface_resolution.width as f32,
                        height: self.surface_resolution.height as f32,
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }];

                    let scissors = [vk::Rect2D {
                        offset: vk::Offset2D { x: 0, y: 0 },
                        extent: self.surface_resolution,
                    }];

                    device.cmd_set_viewport(command_buffer, 0, &viewports);
                    device.cmd_set_scissor(command_buffer, 0, &scissors);

                    device.cmd_draw(command_buffer, 3, 1, 0, 0);
                    device.cmd_end_render_pass(command_buffer);
                },
            );

            let wait_semaphores = [self.rendering_complete_semaphore];
            let swapchains = [self.swapchain];
            let image_indices = [present_index];
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&wait_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            match self
                .swapchain_loader
                .queue_present(self.present_queue, &present_info)
            {
                _ => {}
            }
        }
    }

    fn resize<T: HasRawWindowHandle>(&mut self, _window: &T, width: usize, height: usize) {
        let semaphore_create_info = vk::SemaphoreCreateInfo::default();
        unsafe {
            self.present_complete_semaphore = self
                .device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            self.rendering_complete_semaphore = self
                .device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();

            self.surface_resolution.width = width as u32;
            self.surface_resolution.height = height as u32;

            self.swapchain_create_info.image_extent.width = width as u32;
            self.swapchain_create_info.image_extent.height = height as u32;
            self.swapchain_create_info.old_swapchain = self.swapchain;
            self.swapchain = self
                .swapchain_loader
                .create_swapchain(&self.swapchain_create_info, None)
                .unwrap();

            self.present_images = self
                .swapchain_loader
                .get_swapchain_images(self.swapchain)
                .unwrap();
            self.present_image_views = self
                .present_images
                .iter()
                .map(|&image| {
                    let create_view_info = vk::ImageViewCreateInfo::builder()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(self.surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .image(image);
                    self.device
                        .create_image_view(&create_view_info, None)
                        .unwrap()
                })
                .collect();

            let depth_image_create_info = vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(Self::DEPTH_FORMAT)
                .extent(vk::Extent3D {
                    width: self.surface_resolution.width,
                    height: self.surface_resolution.height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);

            self.depth_image = self
                .device
                .create_image(&depth_image_create_info, None)
                .unwrap();
            let depth_image_memory_req =
                self.device.get_image_memory_requirements(self.depth_image);
            let depth_image_memory_index = find_memorytype_index(
                &depth_image_memory_req,
                &self.device_memory_properties,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )
            .expect("Unable to find suitable memory index for depth image.");

            let depth_image_allocate_info = vk::MemoryAllocateInfo::builder()
                .allocation_size(depth_image_memory_req.size)
                .memory_type_index(depth_image_memory_index)
                .build();

            self.depth_image_memory = self
                .device
                .allocate_memory(&depth_image_allocate_info, None)
                .unwrap();

            self.device
                .bind_image_memory(self.depth_image, self.depth_image_memory, 0)
                .unwrap();

            let depth_image_view_info = vk::ImageViewCreateInfo::builder()
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1)
                        .build(),
                )
                .image(self.depth_image)
                .format(depth_image_create_info.format)
                .view_type(vk::ImageViewType::TYPE_2D);

            self.depth_image_view = self
                .device
                .create_image_view(&depth_image_view_info, None)
                .unwrap();

            self.framebuffers = self
                .present_image_views
                .iter()
                .map(|&present_image_view| {
                    let framebuffer_attachments = [present_image_view, self.depth_image_view];
                    let frame_buffer_create_info = vk::FramebufferCreateInfo::builder()
                        .render_pass(self.render_pass)
                        .attachments(&framebuffer_attachments)
                        .width(self.surface_resolution.width)
                        .height(self.surface_resolution.height)
                        .layers(1);

                    self.device
                        .create_framebuffer(&frame_buffer_create_info, None)
                        .unwrap()
                })
                .collect();

            record_submit_commandbuffer(
                &self.device,
                self.setup_command_buffer,
                self.present_queue,
                &[],
                &[],
                &[],
                |device, command_buffer| {
                    let layout_transition_barriers = vk::ImageMemoryBarrier::builder()
                        .image(self.depth_image)
                        .dst_access_mask(
                            vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                        )
                        .new_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .subresource_range(
                            vk::ImageSubresourceRange::builder()
                                .aspect_mask(vk::ImageAspectFlags::DEPTH)
                                .layer_count(1)
                                .level_count(1)
                                .build(),
                        );

                    device.cmd_pipeline_barrier(
                        command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barriers.build()],
                    );
                },
            );
        }
    }

    fn set_point_lights(&mut self, _changed: &BitVec, _lights: &[PointLight]) {}

    fn set_spot_lights(&mut self, _changed: &BitVec, _lights: &[SpotLight]) {}

    fn set_area_lights(&mut self, _changed: &BitVec, _lights: &[AreaLight]) {}

    fn set_directional_lights(
        &mut self,
        _changed: &BitVec<Local, usize>,
        _lights: &[DirectionalLight],
    ) {
    }

    fn set_skybox(&mut self, _skybox: Texture) {
        unimplemented!()
    }

    fn get_settings(&self) -> Vec<Setting> {
        Vec::new()
    }

    fn set_setting(&mut self, _setting: Setting) {}
    fn set_animated_mesh(&mut self, _id: usize, _mesh: &rfw_scene::AnimatedMesh) {
        todo!()
    }

    fn set_skin(&mut self, _id: usize, _skin: &Skin) {
        unimplemented!()
    }
}
