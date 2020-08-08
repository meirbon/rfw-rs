#[cfg(all(not(feature = "dx12"), any(target_os = "windows", target_os = "unix")))]
pub use gfx_backend_vulkan as backend;

#[cfg(feature = "dx12")]
pub use gfx_backend_dx12 as backend;

#[cfg(target_os = "macos")]
pub use gfx_backend_metal as backend;

pub use gfx_hal as hal;

use buffer::Allocator;
use hal::prelude::*;
use hal::{
    adapter::PhysicalDevice,
    command::CommandBuffer,
    device::Device,
    pool::CommandPool,
    queue::{CommandQueue, QueueFamily},
    window::PresentationSurface,
    Instance,
};
use hal::{
    command, format as f,
    format::ChannelType,
    pool, pso,
    queue::{QueueGroup, Submission},
    window,
};
use instances::SceneList;
use rfw_scene::Renderer;
use std::{iter, mem::ManuallyDrop, ptr, sync::Arc};
use window::Extent2D;

mod buffer;
mod instances;
mod mesh;

#[derive(Debug, Copy, Clone)]
pub enum GfxError {
    UnsupportedBackend,
    SurfaceError,
    NoDevice,
}

impl std::fmt::Display for GfxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                GfxError::UnsupportedBackend => "Unsupported backend",
                GfxError::SurfaceError => "Could not initialize surface",
                GfxError::NoDevice => "Could not find a suitable device",
            }
        )
    }
}

impl std::error::Error for GfxError {}

pub type GfxBackend = GfxRenderer<backend::Backend>;

pub struct GfxRenderer<B: hal::Backend> {
    instance: B::Instance,
    queue_group: ManuallyDrop<QueueGroup<B>>,
    device: Arc<B::Device>,
    surface: ManuallyDrop<B::Surface>,
    adapter: hal::adapter::Adapter<B>,
    cmd_pools: Vec<B::CommandPool>,
    cmd_buffers: Vec<B::CommandBuffer>,
    submission_complete_semaphores: Vec<B::Semaphore>,
    submission_complete_fences: Vec<B::Fence>,
    format: hal::format::Format,
    viewport: pso::Viewport,
    frame: usize,
    frames_in_flight: usize,
    dimensions: Extent2D,

    scene_list: SceneList<B>,
    mesh_renderer: mesh::RenderPipeline<B>,
}

impl<B: hal::Backend> Renderer for GfxRenderer<B> {
    fn init<T: rfw_scene::raw_window_handle::HasRawWindowHandle>(
        window: &T,
        width: usize,
        height: usize,
    ) -> Result<Box<Self>, Box<dyn std::error::Error>> {
        let instance: B::Instance = match hal::Instance::create("RFW", 1) {
            Ok(instance) => instance,
            Err(_) => return Err(Box::new(GfxError::UnsupportedBackend)),
        };

        let mut surface: B::Surface = match unsafe { instance.create_surface(window) } {
            Ok(surface) => surface,
            Err(_) => return Err(Box::new(GfxError::SurfaceError)),
        };

        let mut adapters = instance.enumerate_adapters();
        let mut adapter_index: Option<usize> = None;
        for (i, adapter) in adapters.iter().enumerate() {
            if adapter.info.device_type == hal::adapter::DeviceType::DiscreteGpu {
                adapter_index = Some(i);
            }
        }

        if adapter_index.is_none() {
            for (i, adapter) in adapters.iter().enumerate() {
                if adapter.info.device_type == hal::adapter::DeviceType::IntegratedGpu {
                    adapter_index = Some(i);
                }
            }
        }

        if adapter_index.is_none() {
            return Err(Box::new(GfxError::SurfaceError));
        }

        let adapter = adapters.remove(adapter_index.unwrap());
        println!("Picked adapter: {}", adapter.info.name);

        // Build a new device and associated command queues
        let family = adapter
            .queue_families
            .iter()
            .find(|family| {
                surface.supports_queue_family(family) && family.queue_type().supports_graphics()
            })
            .unwrap();
        let mut gpu = unsafe {
            adapter
                .physical_device
                .open(&[(family, &[1.0])], hal::Features::NDC_Y_UP)
                .unwrap()
        };
        let queue_group = gpu.queue_groups.pop().unwrap();
        let device = gpu.device;

        let command_pool = unsafe {
            device.create_command_pool(queue_group.family, pool::CommandPoolCreateFlags::empty())
        }
        .expect("Can't create command pool");

        let frames_in_flight = 3;

        let mut submission_complete_semaphores = Vec::with_capacity(frames_in_flight);
        let mut submission_complete_fences = Vec::with_capacity(frames_in_flight);

        let mut cmd_pools = Vec::with_capacity(frames_in_flight);
        let mut cmd_buffers = Vec::with_capacity(frames_in_flight);

        cmd_pools.push(command_pool);
        for _ in 1..frames_in_flight {
            unsafe {
                cmd_pools.push(
                    device
                        .create_command_pool(
                            queue_group.family,
                            pool::CommandPoolCreateFlags::empty(),
                        )
                        .expect("Can't create command pool"),
                );
            }
        }

        for i in 0..frames_in_flight {
            submission_complete_semaphores.push(
                device
                    .create_semaphore()
                    .expect("Could not create semaphore"),
            );
            submission_complete_fences
                .push(device.create_fence(true).expect("Could not create fence"));
            cmd_buffers.push(unsafe { cmd_pools[i].allocate_one(command::Level::Primary) });
        }

        let caps = surface.capabilities(&adapter.physical_device);
        let formats = surface.supported_formats(&adapter.physical_device);

        let format = formats.map_or(f::Format::Rgba8Srgb, |formats| {
            formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .map(|format| *format)
                .unwrap_or(formats[0])
        });

        let swap_config = window::SwapchainConfig::from_caps(
            &caps,
            format,
            Extent2D {
                width: width as u32,
                height: height as u32,
            },
        );

        let extent = swap_config.extent;
        unsafe {
            surface
                .configure_swapchain(&device, swap_config)
                .expect("Can't configure swapchain");
        };

        let viewport = pso::Viewport {
            rect: pso::Rect {
                x: 0,
                y: 0,
                w: extent.width as i16,
                h: extent.height as i16,
            },
            depth: 0.0..1.0,
        };

        let device = Arc::new(device);
        let allocator = Allocator::new(device.clone(), &adapter);

        let scene_list = SceneList::new(device.clone(), allocator.clone());
        let mesh_renderer = mesh::RenderPipeline::new(
            device.clone(),
            allocator,
            format,
            width as u32,
            height as u32,
            &scene_list,
        );

        Ok(Box::new(Self {
            instance,
            queue_group: ManuallyDrop::new(queue_group),
            device,
            surface: ManuallyDrop::new(surface),
            adapter,
            cmd_pools,
            cmd_buffers,
            submission_complete_semaphores,
            submission_complete_fences,
            format,
            viewport,
            frame: 0,
            frames_in_flight,
            dimensions: Extent2D {
                width: width as u32,
                height: height as u32,
            },
            scene_list,
            mesh_renderer,
        }))
    }

    fn set_mesh(&mut self, id: usize, mesh: &rfw_scene::Mesh) {
        self.scene_list.set_mesh(id, mesh);
    }

    fn set_animated_mesh(&mut self, _id: usize, _mesh: &rfw_scene::AnimatedMesh) {}

    fn set_instance(&mut self, id: usize, instance: &rfw_scene::Instance) {
        self.scene_list.set_instance(id, instance);
    }

    fn set_materials(
        &mut self,
        _materials: &[rfw_scene::Material],
        _device_materials: &[rfw_scene::DeviceMaterial],
    ) {
    }

    fn set_textures(&mut self, _textures: &[rfw_scene::Texture]) {}

    fn synchronize(&mut self) {
        self.scene_list.synchronize();
    }

    fn render(&mut self, camera: &rfw_scene::Camera, _mode: rfw_scene::RenderMode) {
        self.mesh_renderer.update_camera(camera);

        let surface_image = unsafe {
            match self.surface.acquire_image(!0) {
                Ok((image, _)) => image,
                Err(_) => {
                    self.recreate_swapchain();
                    return;
                }
            }
        };

        let framebuffer = unsafe {
            self.mesh_renderer
                .create_frame_buffer(&surface_image, self.dimensions)
        };

        // Compute index into our resource ring buffers based on the frame number
        // and number of frames in flight. Pay close attention to where this index is needed
        // versus when the swapchain image index we got from acquire_image is needed.
        let frame_idx = self.frame as usize % self.frames_in_flight;

        // Wait for the fence of the previous submission of this frame and reset it; ensures we are
        // submitting only up to maximum number of frames_in_flight if we are submitting faster than
        // the gpu can keep up with. This would also guarantee that any resources which need to be
        // updated with a CPU->GPU data copy are not in use by the GPU, so we can perform those updates.
        // In this case there are none to be done, however.
        unsafe {
            let fence = &self.submission_complete_fences[frame_idx];
            self.device
                .wait_for_fence(fence, !0)
                .expect("Failed to wait for fence");
            self.device
                .reset_fence(fence)
                .expect("Failed to reset fence");
            self.cmd_pools[frame_idx].reset(false);
        }

        // // Rendering
        let cmd_buffer = &mut self.cmd_buffers[frame_idx];
        unsafe {
            cmd_buffer.begin_primary(command::CommandBufferFlags::ONE_TIME_SUBMIT);

            cmd_buffer.set_viewports(0, &[self.viewport.clone()]);
            cmd_buffer.set_scissors(0, &[self.viewport.rect]);

            self.mesh_renderer
                .draw(cmd_buffer, &framebuffer, &self.viewport, &self.scene_list);
            cmd_buffer.finish();

            let submission = Submission {
                command_buffers: iter::once(&*cmd_buffer),
                wait_semaphores: None,
                signal_semaphores: iter::once(&self.submission_complete_semaphores[frame_idx]),
            };

            self.queue_group.queues[0].submit(
                submission,
                Some(&self.submission_complete_fences[frame_idx]),
            );

            // present frame
            let result = self.queue_group.queues[0].present_surface(
                &mut self.surface,
                surface_image,
                Some(&self.submission_complete_semaphores[frame_idx]),
            );

            self.device.destroy_framebuffer(framebuffer);

            if result.is_err() {
                self.recreate_swapchain();
            }
        }

        // Increment our frame
        self.frame += 1;
    }

    fn resize<T: rfw_scene::raw_window_handle::HasRawWindowHandle>(
        &mut self,
        _window: &T,
        width: usize,
        height: usize,
    ) {
        self.device.wait_idle().unwrap();

        self.dimensions.width = width as u32;
        self.dimensions.height = height as u32;
        self.recreate_swapchain();
        self.mesh_renderer.resize(width as u32, height as u32);
    }

    fn set_point_lights(
        &mut self,
        _changed: &rfw_scene::BitVec,
        _lights: &[rfw_scene::PointLight],
    ) {
    }

    fn set_spot_lights(&mut self, _changed: &rfw_scene::BitVec, _lights: &[rfw_scene::SpotLight]) {}

    fn set_area_lights(&mut self, _changed: &rfw_scene::BitVec, _lights: &[rfw_scene::AreaLight]) {}

    fn set_directional_lights(
        &mut self,
        _changed: &rfw_scene::BitVec,
        _lights: &[rfw_scene::DirectionalLight],
    ) {
    }

    fn set_skybox(&mut self, _skybox: rfw_scene::Texture) {}

    fn set_skin(&mut self, _id: usize, _skin: &rfw_scene::graph::Skin) {}

    fn get_settings(&self) -> Vec<rfw_scene::Setting> {
        Vec::new()
    }

    fn set_setting(&mut self, _setting: rfw_scene::Setting) {}
}

impl<B: hal::Backend> GfxRenderer<B> {
    fn recreate_swapchain(&mut self) {
        let caps = self.surface.capabilities(&self.adapter.physical_device);
        let swap_config = window::SwapchainConfig::from_caps(&caps, self.format, self.dimensions);

        let extent = swap_config.extent.to_extent();

        unsafe {
            self.surface
                .configure_swapchain(&self.device, swap_config)
                .expect("Can't recreate swapchain");
        }

        self.viewport.rect.w = extent.width as _;
        self.viewport.rect.h = extent.height as _;
    }
}

impl<B: hal::Backend> Drop for GfxRenderer<B> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();

        unsafe {
            for p in self.cmd_pools.drain(..) {
                self.device.destroy_command_pool(p);
            }
            for s in self.submission_complete_semaphores.drain(..) {
                self.device.destroy_semaphore(s);
            }
            for f in self.submission_complete_fences.drain(..) {
                self.device.destroy_fence(f);
            }

            // TODO: When ManuallyDrop::take (soon to be renamed to ManuallyDrop::read) is stabilized we should use that instead.
            self.surface.unconfigure_swapchain(&self.device);
            let surface = ManuallyDrop::into_inner(ptr::read(&self.surface));
            self.instance.destroy_surface(surface);
        }
    }
}
