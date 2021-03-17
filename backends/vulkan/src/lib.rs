use ash::extensions::ext::DebugUtils;
use ash::extensions::khr::{Surface, Swapchain};
use ash::version::{DeviceV1_0, EntryV1_0, InstanceV1_0};
use ash::*;
use rfw::backend::{
    AreaLight, BitSlice, CameraView2D, CameraView3D, DeviceMaterial, DirectionalLight,
    HasRawWindowHandle, InstancesData2D, InstancesData3D, JointData, Lsb0, MeshData2D, MeshData3D,
    PointLight, RenderMode, SkinData, SkinID, SpotLight, TextureData, Vertex2D, Vertex3D,
};
use rfw::math::*;
use std::ffi::CString;
use std::os::raw::c_char;
use std::{error::Error, fmt::Display};

mod list;
mod memory;
mod pipeline;
mod structs;
mod util;

pub use list::*;
pub use memory::*;
pub use pipeline::*;
pub use structs::*;
use util::*;

#[derive(Debug)]
pub enum VkError {
    NoSupportedDevice,
}

impl Display for VkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "VkError({})",
            match self {
                Self::NoSupportedDevice => String::from("NoSupportedDevice"),
            }
        )
    }
}

impl Error for VkError {}

pub struct VkBackend {
    entry: Entry,
    instance: Instance,
    device: Device,
    surface_loader: Surface,
    swapchain_loader: Swapchain,
    debug_utils: Option<(DebugUtils, vk::DebugUtilsMessengerEXT)>,

    pdevice: vk::PhysicalDevice,
    device_memory_properties: vk::PhysicalDeviceMemoryProperties,
    queue_family_index: u32,
    present_queue: vk::Queue,

    surface: vk::SurfaceKHR,
    surface_format: vk::SurfaceFormatKHR,
    surface_resolution: vk::Extent2D,

    swapchain: vk::SwapchainKHR,
    present_images: Vec<vk::Image>,
    present_image_views: Vec<vk::ImageView>,

    pool: vk::CommandPool,
    draw_command_buffer: vk::CommandBuffer,
    setup_command_buffer: vk::CommandBuffer,

    depth_image: Option<VkImage>,
    depth_image_view: vk::ImageView,

    present_complete_semaphore: vk::Semaphore,
    rendering_complete_semaphore: vk::Semaphore,

    draw_commands_reuse_fence: vk::Fence,
    setup_commands_reuse_fence: vk::Fence,

    renderpass: vk::RenderPass,
    framebuffers: Vec<vk::Framebuffer>,

    allocator: Option<VkAllocator>,
    uniform_camera: Option<VkBuffer<VkCamera>>,

    vertex_list_2d: VertexList<Vertex2D>,
    instance_matrices_2d: InstanceList<Mat4>,

    vertex_list_3d: VertexList<Vertex3D, JointData>,
    instance_matrices_storage: Vec<Vec<InstanceTransform>>,
    instance_matrices_3d: InstanceList<InstanceTransform>,
    skin_ids: Vec<SkinID>,

    area_lights: Option<VkBuffer<AreaLight>>,
    point_lights: Option<VkBuffer<PointLight>>,
    spot_lights: Option<VkBuffer<SpotLight>>,
    directional_lights: Option<VkBuffer<DirectionalLight>>,

    pipeline: RenderPipeline,
    settings: VkSettings,
    // TODO: use bitflags for this
    update_set: bool,
    update_2d: bool,
    update_2d_instances: bool,
    update_3d: bool,
    update_3d_instances: bool,
}

#[derive(Debug, Default)]
pub struct VkSettings {}

impl rfw::backend::Backend for VkBackend {
    type Settings = VkSettings;

    fn init<T: HasRawWindowHandle>(
        window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    ) -> Result<Box<Self>, Box<dyn Error>> {
        unsafe {
            let entry = Entry::new()?;
            let app_name = CString::new("RFW")?;
            let mut layer_names_raw: Vec<*const c_char> = Vec::new();
            let layer_names: Vec<CString> = vec![
                #[cfg(feature = "validation")]
                CString::new("VK_LAYER_KHRONOS_validation")?,
            ];

            for ln in layer_names.iter() {
                layer_names_raw.push(ln.as_ptr());
            }

            let surface_extensions = ash_window::enumerate_required_extensions(window)?;
            let mut extension_names_raw: Vec<*const c_char> =
                surface_extensions.iter().map(|ext| ext.as_ptr()).collect();

            #[cfg(feature = "validation")]
            extension_names_raw.push(DebugUtils::name().as_ptr());

            let app_info = vk::ApplicationInfo::builder()
                .application_name(app_name.as_ref())
                .application_version(0)
                .engine_name(app_name.as_ref())
                .engine_version(0)
                .api_version(vk::make_version(1, 2, 0));

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_layer_names(&layer_names_raw)
                .enabled_extension_names(&extension_names_raw);

            let instance: Instance = entry.create_instance(&create_info, None)?;

            let debug_utils = if cfg!(feature = "validation") {
                let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                    .message_severity(
                        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                            | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                            | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                    )
                    .message_type(vk::DebugUtilsMessageTypeFlagsEXT::all())
                    .pfn_user_callback(Some(util::vulkan_debug_callback));

                let debug_utils_loader = DebugUtils::new(&entry, &instance);
                let debug_call_back =
                    debug_utils_loader.create_debug_utils_messenger(&debug_info, None)?;
                Some((debug_utils_loader, debug_call_back))
            } else {
                None
            };

            let surface = ash_window::create_surface(&entry, &instance, window, None)?;
            let pdevices = instance.enumerate_physical_devices()?;
            let surface_loader = Surface::new(&entry, &instance);

            let mut pdevice = None;
            for device in pdevices.iter() {
                let results = instance
                    .get_physical_device_queue_family_properties(*device)
                    .iter()
                    .enumerate()
                    .filter_map(|(index, info)| {
                        let supports_graphics_and_surface = info
                            .queue_flags
                            .contains(vk::QueueFlags::GRAPHICS)
                            && surface_loader
                                .get_physical_device_surface_support(*device, index as u32, surface)
                                .unwrap();
                        if supports_graphics_and_surface {
                            Some((*device, index))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                let target = vk::PhysicalDeviceType::DISCRETE_GPU;
                for (device, index) in results.iter() {
                    let properties = instance.get_physical_device_properties(*device);
                    if properties.device_type == target {
                        pdevice = Some((*device, *index));
                        break;
                    }
                }

                if pdevice.is_none() {
                    let target = vk::PhysicalDeviceType::INTEGRATED_GPU;
                    for (device, index) in results.iter() {
                        let properties = instance.get_physical_device_properties(*device);
                        if properties.device_type == target {
                            pdevice = Some((*device, *index));
                            break;
                        }
                    }
                }
            }

            let (pdevice, queue_family_index) = if let Some((pdevice, queue_family_index)) = pdevice
            {
                (pdevice, queue_family_index)
            } else {
                return Err(Box::new(VkError::NoSupportedDevice));
            };

            let queue_family_index = queue_family_index as u32;
            let device_extension_names_raw = [
                Swapchain::name().as_ptr(),
                #[cfg(target_os = "macos")]
                vk::KhrPortabilitySubsetFn::name().as_ptr(),
            ];

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

            let device: Device = instance.create_device(pdevice, &device_create_info, None)?;

            let present_queue = device.get_device_queue(queue_family_index, 0);

            let surface_format =
                surface_loader.get_physical_device_surface_formats(pdevice, surface)?[0];

            let surface_capabilities =
                surface_loader.get_physical_device_surface_capabilities(pdevice, surface)?;

            let surface_capabilities =
                surface_loader.get_physical_device_surface_capabilities(pdevice, surface)?;
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }
            let surface_resolution = match surface_capabilities.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: window_size.0,
                    height: window_size.1,
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
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
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
                .image_array_layers(1);

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
                            r: vk::ComponentSwizzle::IDENTITY,
                            g: vk::ComponentSwizzle::IDENTITY,
                            b: vk::ComponentSwizzle::IDENTITY,
                            a: vk::ComponentSwizzle::IDENTITY,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .image(image);
                    device.create_image_view(&create_view_info, None).unwrap()
                })
                .collect();
            let device_memory_properties = instance.get_physical_device_memory_properties(pdevice);
            let depth_image_create_info = vk::ImageCreateInfo::builder()
                .image_type(vk::ImageType::TYPE_2D)
                .format(vk::Format::D32_SFLOAT)
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

            let allocator_create_info = vk_mem::AllocatorCreateInfo {
                device: device.clone(),
                instance: instance.clone(),
                flags: vk_mem::AllocatorCreateFlags::default(),
                frame_in_use_count: desired_image_count,
                heap_size_limits: None,
                preferred_large_heap_block_size: 0,
                physical_device: pdevice,
            };

            let allocator = VkAllocator::new(&allocator_create_info)?;
            let depth_image = allocator.create_image(
                &depth_image_create_info,
                vk_mem::MemoryUsage::GpuOnly,
                vk_mem::AllocationCreateFlags::NONE,
            )?;

            let fence_create_info =
                vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

            let draw_commands_reuse_fence = device
                .create_fence(&fence_create_info, None)
                .expect("Create fence failed.");
            let setup_commands_reuse_fence = device
                .create_fence(&fence_create_info, None)
                .expect("Create fence failed.");

            let depth_image_view_info = vk::ImageViewCreateInfo::builder()
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1)
                        .build(),
                )
                .image(*depth_image)
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

            record_submit_commandbuffer(
                &device,
                setup_command_buffer,
                setup_commands_reuse_fence,
                present_queue,
                &[],
                &[],
                &[],
                |device, setup_command_buffer| {
                    let layout_transition_barriers = vk::ImageMemoryBarrier::builder()
                        .image(*depth_image)
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
                        setup_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barriers.build()],
                    );
                },
            );

            let renderpass_attachments = [
                vk::AttachmentDescription {
                    format: surface_format.format,
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

            let framebuffers: Vec<vk::Framebuffer> = present_image_views
                .iter()
                .map(|&present_image_view| {
                    let framebuffer_attachments = [present_image_view, depth_image_view];
                    let frame_buffer_create_info = vk::FramebufferCreateInfo::builder()
                        .render_pass(renderpass)
                        .attachments(&framebuffer_attachments)
                        .width(surface_resolution.width)
                        .height(surface_resolution.height)
                        .layers(1);

                    device
                        .create_framebuffer(&frame_buffer_create_info, None)
                        .unwrap()
                })
                .collect();

            let uniform_camera = allocator.create_buffer(
                vk::BufferUsageFlags::UNIFORM_BUFFER,
                vk_mem::MemoryUsage::CpuToGpu,
                vk_mem::AllocationCreateFlags::MAPPED,
                1,
            )?;

            let pipeline =
                RenderPipeline::new(&device, window_size.0, window_size.1, surface_format.format);

            Ok(Box::new(Self {
                entry,
                instance,
                device,
                debug_utils,
                queue_family_index,
                pdevice,
                device_memory_properties,
                surface_loader,
                surface_format,
                present_queue,
                surface_resolution,
                swapchain_loader,
                swapchain,
                present_images,
                present_image_views,
                pool,
                draw_command_buffer,
                setup_command_buffer,
                depth_image: Some(depth_image),
                depth_image_view,
                present_complete_semaphore,
                rendering_complete_semaphore,
                draw_commands_reuse_fence,
                setup_commands_reuse_fence,
                surface,
                renderpass,
                framebuffers,
                allocator: Some(allocator),
                uniform_camera: Some(uniform_camera),

                vertex_list_2d: Default::default(),
                instance_matrices_2d: Default::default(),

                vertex_list_3d: Default::default(),
                instance_matrices_storage: Default::default(),
                instance_matrices_3d: Default::default(),
                skin_ids: Default::default(),

                area_lights: None,
                point_lights: None,
                spot_lights: None,
                directional_lights: None,

                pipeline,
                settings: VkSettings::default(),
                update_set: false,
                update_2d: false,
                update_2d_instances: false,
                update_3d: false,
                update_3d_instances: false,
            }))
        }
    }

    fn set_2d_mesh(&mut self, id: usize, data: MeshData2D<'_>) {
        // We store pointers here. Although this is unsafe, rfw ensures pointers do not get updated till synchronize is called.
        // Thus, we can safely store pointers and efficiently copy data even though this is technically unsafe.
        if self.vertex_list_2d.has(id) {
            self.vertex_list_2d.update_pointer(
                id,
                data.vertices.as_ptr(),
                None,
                data.vertices.len() as u32,
            );
        } else {
            self.vertex_list_2d.add_pointer(
                id,
                data.vertices.as_ptr(),
                None,
                data.vertices.len() as u32,
            );
        }

        self.update_2d = true;
    }

    fn set_2d_instances(&mut self, mesh: usize, instances: InstancesData2D<'_>) {
        if self.instance_matrices_2d.has(mesh) {
            self.instance_matrices_2d.update_instances_list(
                mesh,
                instances.matrices.as_ptr(),
                instances.len() as u32,
            );
        } else {
            self.instance_matrices_2d.add_instances_list(
                mesh,
                instances.matrices.as_ptr(),
                instances.len() as u32,
            );
        }

        self.update_2d_instances = true;
    }

    fn set_3d_mesh(&mut self, id: usize, data: MeshData3D<'_>) {
        // We store pointers here. Although this is unsafe, rfw ensures pointers do not get updated till synchronize is called.
        // Thus, we can safely store pointers and efficiently copy data even though this is technically unsafe.
        if self.vertex_list_3d.has(id) {
            self.vertex_list_3d.update_pointer(
                id,
                data.vertices.as_ptr(),
                if data.skin_data.is_empty() {
                    None
                } else {
                    Some(data.skin_data.as_ptr())
                },
                data.vertices.len() as u32,
            );
        } else {
            self.vertex_list_3d.add_pointer(
                id,
                data.vertices.as_ptr(),
                if data.skin_data.is_empty() {
                    None
                } else {
                    Some(data.skin_data.as_ptr())
                },
                data.vertices.len() as u32,
            );
        }

        self.update_3d = true;
    }

    fn unload_3d_meshes(&mut self, ids: Vec<usize>) {
        for id in ids {
            self.vertex_list_3d.remove_pointer(id);
            self.instance_matrices_3d.remove_instances_list(id);
        }
    }

    fn set_3d_instances(&mut self, mesh: usize, instances: InstancesData3D<'_>) {
        if mesh >= self.instance_matrices_storage.len() {
            self.instance_matrices_storage
                .resize(mesh + 1, Default::default());
        }

        self.instance_matrices_storage[mesh] = instances
            .matrices
            .iter()
            .map(|m| InstanceTransform::new(*m))
            .collect();

        if self.instance_matrices_3d.has(mesh) {
            self.instance_matrices_3d.update_instances_list(
                mesh,
                self.instance_matrices_storage[mesh].as_ptr(),
                instances.len() as u32,
            );
        } else {
            self.instance_matrices_3d.add_instances_list(
                mesh,
                self.instance_matrices_storage[mesh].as_ptr(),
                instances.len() as u32,
            );
        }

        self.skin_ids.resize(instances.len(), SkinID::default());
        self.skin_ids.copy_from_slice(instances.skin_ids);

        self.update_3d_instances = true;
    }

    fn set_materials(&mut self, materials: &[DeviceMaterial], changed: &BitSlice<Lsb0, usize>) {}

    fn set_textures(&mut self, textures: &[TextureData<'_>], changed: &BitSlice<Lsb0, usize>) {}

    fn synchronize(&mut self) {
        let update_2d = self.update_2d;
        self.vertex_list_2d.update_ranges();
        let update_3d = self.update_3d;
        self.vertex_list_3d.update_ranges();

        let update_inst_2d = self.update_2d_instances;
        self.instance_matrices_2d.update_ranges();
        let update_inst_3d = self.update_3d_instances;
        self.instance_matrices_3d.update_ranges();

        let allocator = self.allocator.as_ref().unwrap();
        let vertex_list_2d = &mut self.vertex_list_2d;
        let instance_list_2d = &mut self.instance_matrices_2d;
        let vertex_list_3d = &mut self.vertex_list_3d;
        let instance_list_3d = &mut self.instance_matrices_3d;

        if update_2d || update_3d || update_inst_2d || update_inst_3d {
            record_submit_commandbuffer(
                &self.device,
                self.setup_command_buffer,
                self.setup_commands_reuse_fence,
                self.present_queue,
                &[],
                &[],
                &[],
                |device, cmd_buffer| {
                    if update_2d {
                        vertex_list_2d.update_data(allocator, device, cmd_buffer);
                    }
                    if update_3d {
                        vertex_list_3d.update_data(allocator, device, cmd_buffer);
                    }
                    if update_inst_2d {
                        instance_list_2d.update_data(allocator, device, cmd_buffer);
                    }
                    if update_inst_3d {
                        instance_list_3d.update_data(allocator, device, cmd_buffer);
                    }
                },
            );
        }

        self.update_2d = false;
        self.update_3d = false;
        self.update_2d_instances = false;
        self.update_3d_instances = false;
    }

    fn render(&mut self, view_2d: CameraView2D, view_3d: CameraView3D, _mode: RenderMode) {
        if let Some(mut mapping) = self.uniform_camera.as_mut().and_then(|c| c.map_memory()) {
            mapping[0].view_2d = view_2d;
            mapping[0].view_3d = view_3d;
            mapping[0].view =
                Mat4::from_scale(Vec3::new(1.0, -1.0, 1.0)) * view_3d.get_rh_view_matrix();
            mapping[0].projection = view_3d.get_rh_projection();
            mapping[0].view_projection = mapping[0].view * mapping[0].projection;
            mapping[0].light_count = UVec4::new(
                self.area_lights.as_ref().map(|l| l.len()).unwrap_or(0) as u32,
                self.spot_lights.as_ref().map(|l| l.len()).unwrap_or(0) as u32,
                self.area_lights.as_ref().map(|l| l.len()).unwrap_or(0) as u32,
                self.directional_lights
                    .as_ref()
                    .map(|l| l.len())
                    .unwrap_or(0) as u32,
            );
        }

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

            let uniform_camera = &self.uniform_camera;
            let instance_matrices_3d = &self.instance_matrices_3d;
            let vertex_list_3d = &self.vertex_list_3d;
            let framebuffer = self.framebuffers[present_index as usize];
            let pipeline = &mut self.pipeline;

            let (width, height) = (
                self.surface_resolution.width,
                self.surface_resolution.height,
            );

            record_submit_commandbuffer(
                &self.device,
                self.setup_command_buffer,
                self.setup_commands_reuse_fence,
                self.present_queue,
                &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
                &[self.present_complete_semaphore],
                &[self.rendering_complete_semaphore],
                |device, cmd_buffer| {
                    // Descriptor set might still be in use in last frame, only update it once we know the previous frame finished rendering.
                    if let (Some(c), Some(i)) =
                        (uniform_camera.as_ref(), instance_matrices_3d.get_buffer())
                    {
                        pipeline.update_descriptor_set(device, c.buffer, i)
                    }

                    pipeline.render(
                        width,
                        height,
                        device,
                        cmd_buffer,
                        framebuffer,
                        vertex_list_3d,
                        instance_matrices_3d,
                    );
                },
            );

            let wait_semaphores = [self.rendering_complete_semaphore];
            let swapchains = [self.swapchain];
            let image_indices = [present_index];
            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&wait_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            self.swapchain_loader
                .queue_present(self.present_queue, &present_info)
                .unwrap();
        }
    }

    fn resize<T: HasRawWindowHandle>(
        &mut self,
        _window: &T,
        window_size: (u32, u32),
        _scale_factor: f64,
    ) {
        unsafe {
            self.surface_resolution.width = window_size.0;
            self.surface_resolution.height = window_size.1;

            self.device.device_wait_idle().unwrap();
            for framebuffer in self.framebuffers.drain(0..self.framebuffers.len()) {
                self.device.destroy_framebuffer(framebuffer, None);
            }

            if let Some(image) = self.depth_image.take() {
                drop(image);
                self.device.destroy_image_view(self.depth_image_view, None);
            }

            for &image_view in self.present_image_views.iter() {
                self.device.destroy_image_view(image_view, None);
            }
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            // re-initialize
            let surface_format = self
                .surface_loader
                .get_physical_device_surface_formats(self.pdevice, self.surface)
                .unwrap()[0];

            let surface_capabilities = self
                .surface_loader
                .get_physical_device_surface_capabilities(self.pdevice, self.surface)
                .unwrap();
            let mut desired_image_count = surface_capabilities.min_image_count + 1;
            if surface_capabilities.max_image_count > 0
                && desired_image_count > surface_capabilities.max_image_count
            {
                desired_image_count = surface_capabilities.max_image_count;
            }
            let surface_resolution = match surface_capabilities.current_extent.width {
                std::u32::MAX => vk::Extent2D {
                    width: window_size.0,
                    height: window_size.1,
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

            let present_modes = self
                .surface_loader
                .get_physical_device_surface_present_modes(self.pdevice, self.surface)
                .unwrap();
            let present_mode = present_modes
                .iter()
                .cloned()
                .find(|&mode| mode == vk::PresentModeKHR::MAILBOX)
                .unwrap_or(vk::PresentModeKHR::FIFO);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(self.surface)
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
                .image_array_layers(1);

            self.swapchain = self
                .swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
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
                .format(vk::Format::D32_SFLOAT)
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

            self.depth_image = Some(
                self.allocator
                    .as_ref()
                    .unwrap()
                    .create_image(
                        &depth_image_create_info,
                        vk_mem::MemoryUsage::GpuOnly,
                        vk_mem::AllocationCreateFlags::NONE,
                    )
                    .unwrap(),
            );

            let depth_image_view_info = vk::ImageViewCreateInfo::builder()
                .subresource_range(
                    vk::ImageSubresourceRange::builder()
                        .aspect_mask(vk::ImageAspectFlags::DEPTH)
                        .level_count(1)
                        .layer_count(1)
                        .build(),
                )
                .image(**self.depth_image.as_ref().unwrap())
                .format(depth_image_create_info.format)
                .view_type(vk::ImageViewType::TYPE_2D);

            self.depth_image_view = self
                .device
                .create_image_view(&depth_image_view_info, None)
                .unwrap();

            record_submit_commandbuffer(
                &self.device,
                self.setup_command_buffer,
                self.setup_commands_reuse_fence,
                self.present_queue,
                &[],
                &[],
                &[],
                |device, setup_command_buffer| {
                    let layout_transition_barriers = vk::ImageMemoryBarrier::builder()
                        .image(**self.depth_image.as_ref().unwrap())
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

                    self.device.cmd_pipeline_barrier(
                        self.setup_command_buffer,
                        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                        vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                        vk::DependencyFlags::empty(),
                        &[],
                        &[],
                        &[layout_transition_barriers.build()],
                    );
                },
            );

            self.framebuffers = self
                .present_image_views
                .iter()
                .map(|&present_image_view| {
                    let framebuffer_attachments = [present_image_view, self.depth_image_view];
                    let frame_buffer_create_info = vk::FramebufferCreateInfo::builder()
                        .render_pass(self.renderpass)
                        .attachments(&framebuffer_attachments)
                        .width(self.surface_resolution.width)
                        .height(self.surface_resolution.height)
                        .layers(1);

                    self.device
                        .create_framebuffer(&frame_buffer_create_info, None)
                        .unwrap()
                })
                .collect();
        }
    }

    fn set_point_lights(&mut self, lights: &[PointLight], changed: &BitSlice<Lsb0, usize>) {
        let l = if let Some(l) = &mut self.point_lights {
            if l.len() < lights.len() {
                *l = self
                    .allocator
                    .as_ref()
                    .unwrap()
                    .create_buffer(
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::MAPPED,
                        lights.len().next_power_of_two(),
                    )
                    .unwrap();
            }
            l
        } else {
            self.point_lights = Some(
                self.allocator
                    .as_ref()
                    .unwrap()
                    .create_buffer(
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::MAPPED,
                        lights.len().next_power_of_two(),
                    )
                    .unwrap(),
            );
            self.point_lights.as_mut().unwrap()
        };

        if let Some(mut mapping) = l.map_memory() {
            mapping[0..lights.len()].copy_from_slice(lights);
        }
    }

    fn set_spot_lights(&mut self, lights: &[SpotLight], changed: &BitSlice<Lsb0, usize>) {
        let l = if let Some(l) = &mut self.spot_lights {
            if l.len() < lights.len() {
                *l = self
                    .allocator
                    .as_ref()
                    .unwrap()
                    .create_buffer(
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::MAPPED,
                        lights.len().next_power_of_two(),
                    )
                    .unwrap();
            }
            l
        } else {
            self.spot_lights = Some(
                self.allocator
                    .as_ref()
                    .unwrap()
                    .create_buffer(
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::MAPPED,
                        lights.len().next_power_of_two(),
                    )
                    .unwrap(),
            );
            self.spot_lights.as_mut().unwrap()
        };

        if let Some(mut mapping) = l.map_memory() {
            mapping[0..lights.len()].copy_from_slice(lights);
        }
    }

    fn set_area_lights(&mut self, lights: &[AreaLight], changed: &BitSlice<Lsb0, usize>) {
        let l = if let Some(l) = &mut self.area_lights {
            if l.len() < lights.len() {
                *l = self
                    .allocator
                    .as_ref()
                    .unwrap()
                    .create_buffer(
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::MAPPED,
                        lights.len().next_power_of_two(),
                    )
                    .unwrap();
            }
            l
        } else {
            self.area_lights = Some(
                self.allocator
                    .as_ref()
                    .unwrap()
                    .create_buffer(
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::MAPPED,
                        lights.len().next_power_of_two(),
                    )
                    .unwrap(),
            );
            self.area_lights.as_mut().unwrap()
        };

        if let Some(mut mapping) = l.map_memory() {
            mapping[0..lights.len()].copy_from_slice(lights);
        }
    }

    fn set_directional_lights(
        &mut self,
        lights: &[DirectionalLight],
        changed: &BitSlice<Lsb0, usize>,
    ) {
        let l = if let Some(l) = &mut self.directional_lights {
            if l.len() < lights.len() {
                *l = self
                    .allocator
                    .as_ref()
                    .unwrap()
                    .create_buffer(
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::MAPPED,
                        lights.len().next_power_of_two(),
                    )
                    .unwrap();
            }
            l
        } else {
            self.directional_lights = Some(
                self.allocator
                    .as_ref()
                    .unwrap()
                    .create_buffer(
                        vk::BufferUsageFlags::UNIFORM_BUFFER,
                        vk_mem::MemoryUsage::CpuToGpu,
                        vk_mem::AllocationCreateFlags::MAPPED,
                        lights.len().next_power_of_two(),
                    )
                    .unwrap(),
            );
            self.directional_lights.as_mut().unwrap()
        };

        if let Some(mut mapping) = l.map_memory() {
            mapping[0..lights.len()].copy_from_slice(lights);
        }
    }

    fn set_skybox(&mut self, skybox: TextureData<'_>) {}

    fn set_skins(&mut self, skins: &[SkinData<'_>], changed: &BitSlice<Lsb0, usize>) {}

    fn settings(&mut self) -> &mut Self::Settings {
        &mut self.settings
    }
}

impl Drop for VkBackend {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();

            for framebuffer in self.framebuffers.drain(0..self.framebuffers.len()) {
                self.device.destroy_framebuffer(framebuffer, None);
            }
            self.device.destroy_render_pass(self.renderpass, None);

            self.device
                .destroy_semaphore(self.present_complete_semaphore, None);
            self.device
                .destroy_semaphore(self.rendering_complete_semaphore, None);
            self.device
                .destroy_fence(self.draw_commands_reuse_fence, None);
            self.device
                .destroy_fence(self.setup_commands_reuse_fence, None);

            if let Some(image) = self.depth_image.take() {
                drop(image);
                self.device.destroy_image_view(self.depth_image_view, None);
            }

            for &image_view in self.present_image_views.iter() {
                self.device.destroy_image_view(image_view, None);
            }
            self.device.destroy_command_pool(self.pool, None);
            self.swapchain_loader
                .destroy_swapchain(self.swapchain, None);

            if let Some(uniform_buffer) = self.uniform_camera.take() {
                drop(uniform_buffer);
            }

            if let Some(lights) = self.area_lights.take() {
                drop(lights);
            }
            if let Some(lights) = self.point_lights.take() {
                drop(lights);
            }
            if let Some(lights) = self.spot_lights.take() {
                drop(lights);
            }
            if let Some(lights) = self.directional_lights.take() {
                drop(lights);
            }

            self.vertex_list_2d.free_buffers();
            self.instance_matrices_2d.free_buffers();
            self.vertex_list_3d.free_buffers();
            self.instance_matrices_3d.free_buffers();

            self.pipeline.destroy(&self.device);

            if let Some(allocator) = self.allocator.take() {
                allocator.destroy();
            }

            self.device.destroy_device(None);
            self.surface_loader.destroy_surface(self.surface, None);
            if let Some((debug_utils, debug_callback)) = self.debug_utils.take() {
                debug_utils.destroy_debug_utils_messenger(debug_callback, None);
            }
            self.instance.destroy_instance(None);
        }
    }
}
