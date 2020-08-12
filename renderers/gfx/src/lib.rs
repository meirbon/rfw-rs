pub use gfx_backend_vulkan as backend;

pub use gfx_hal as hal;

use buffer::Allocator;
use hal::prelude::*;
use hal::{
    adapter::PhysicalDevice,
    command,
    command::CommandBuffer,
    device::Device,
    format as f,
    format::ChannelType,
    pool,
    pool::CommandPool,
    pso,
    queue::{CommandQueue, QueueFamily},
    queue::{QueueGroup, Submission},
    window,
    window::PresentationSurface,
    Instance, *,
};
use instances::SceneList;
use rfw_scene::{Renderer, ChangedIterator, AnimatedMesh, Mesh};
use std::{iter, mem::ManuallyDrop, ptr, sync::Arc};
use window::Extent2D;

mod buffer;
mod instances;
mod light;
mod materials;
mod mesh;
mod skinning;

use crate::hal::device::OutOfMemory;
use crate::hal::window::{PresentError, Suboptimal, SwapImageIndex};
use std::borrow::Borrow;
use std::sync::Mutex;
use rfw_scene::graph::Skin;

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

#[derive(Debug)]
pub struct Queue<B: hal::Backend> {
    queue_group: ManuallyDrop<QueueGroup<B>>,
}

impl<B: hal::Backend> Queue<B> {
    pub fn new(group: QueueGroup<B>) -> Self {
        Self {
            queue_group: ManuallyDrop::new(group),
        }
    }

    pub fn submit<'a, T, Ic, S, Iw, Is>(
        &mut self,
        submission: Submission<Ic, Iw, Is>,
        fence: Option<&B::Fence>,
    ) where
        T: 'a + Borrow<B::CommandBuffer>,
        Ic: IntoIterator<Item=&'a T>,
        S: 'a + Borrow<B::Semaphore>,
        Iw: IntoIterator<Item=(&'a S, pso::PipelineStage)>,
        Is: IntoIterator<Item=&'a S>,
    {
        unsafe { self.queue_group.queues[0].submit(submission, fence) }
    }

    pub fn submit_without_semaphores<'a, T, Ic>(
        &mut self,
        command_buffers: Ic,
        fence: Option<&B::Fence>,
    ) where
        T: 'a + Borrow<B::CommandBuffer>,
        Ic: IntoIterator<Item=&'a T>,
    {
        let submission = Submission {
            command_buffers,
            wait_semaphores: iter::empty(),
            signal_semaphores: iter::empty(),
        };
        self.submit::<_, _, B::Semaphore, _, _>(submission, fence)
    }

    pub fn present<'a, W, Is, S, Iw>(
        &mut self,
        swapchains: Is,
        wait_semaphores: Iw,
    ) -> Result<Option<Suboptimal>, PresentError>
        where
            Self: Sized,
            W: 'a + Borrow<B::Swapchain>,
            Is: IntoIterator<Item=(&'a W, SwapImageIndex)>,
            S: 'a + Borrow<B::Semaphore>,
            Iw: IntoIterator<Item=&'a S>,
    {
        unsafe { self.queue_group.queues[0].present(swapchains, wait_semaphores) }
    }

    pub fn present_without_semaphores<'a, W, Is>(
        &mut self,
        swapchains: Is,
    ) -> Result<Option<Suboptimal>, PresentError>
        where
            Self: Sized,
            W: 'a + Borrow<B::Swapchain>,
            Is: IntoIterator<Item=(&'a W, SwapImageIndex)>,
    {
        unsafe { self.queue_group.queues[0].present_without_semaphores(swapchains) }
    }

    pub fn present_surface(
        &mut self,
        surface: &mut B::Surface,
        image: <B::Surface as PresentationSurface<B>>::SwapchainImage,
        wait_semaphore: Option<&B::Semaphore>,
    ) -> Result<Option<Suboptimal>, PresentError> {
        unsafe { self.queue_group.queues[0].present_surface(surface, image, wait_semaphore) }
    }

    pub fn wait_idle(&mut self) -> Result<(), OutOfMemory> {
        self.queue_group.queues[0].wait_idle()
    }
}

pub struct GfxRenderer<B: hal::Backend> {
    instance: B::Instance,
    queue: Arc<Mutex<Queue<B>>>,
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

        // Attempt to find a discrete GPU first (fastest option)
        for (i, adapter) in adapters.iter().enumerate() {
            if adapter.info.device_type == adapter::DeviceType::DiscreteGpu {
                adapter_index = Some(i);
            }
        }

        // If we did not find a GPU, attempt to find an integrated GPU
        if adapter_index.is_none() {
            for (i, adapter) in adapters.iter().enumerate() {
                if adapter.info.device_type == adapter::DeviceType::IntegratedGpu {
                    adapter_index = Some(i);
                }
            }
        }

        if adapter_index.is_none() {
            return Err(Box::new(GfxError::NoDevice));
        }

        // Retrieve the picked adapter
        let adapter = adapters.remove(adapter_index.unwrap());
        println!("Picked adapter: {}", adapter.info.name);

        // Build a new device and associated command queues
        let family = adapter
            .queue_families
            .iter()
            .find(|family| {
                surface.supports_queue_family(family)
                    && family.queue_type().supports_graphics()
                    && family.queue_type().supports_compute()
            })
            .unwrap();

        let transfer_family = adapter
            .queue_families
            .iter()
            .find(|family| family.queue_type().supports_transfer() && family.id() != family.id());
        let (queue, transfer_queue, device) = if let Some(transfer_family) = transfer_family {
            let mut gpu = unsafe {
                adapter
                    .physical_device
                    .open(
                        &[(family, &[1.0]), (&transfer_family, &[0.8])],
                        hal::Features::NDC_Y_UP | hal::Features::SAMPLER_ANISOTROPY,
                    )
                    .unwrap()
            };
            let queue_group = gpu.queue_groups.pop().unwrap();
            let transfer_queue_group = gpu.queue_groups.pop().unwrap();
            let device = gpu.device;
            let queue = Arc::new(Mutex::new(Queue {
                queue_group: ManuallyDrop::new(queue_group),
            }));
            let transfer_queue = Arc::new(Mutex::new(Queue {
                queue_group: ManuallyDrop::new(transfer_queue_group),
            }));
            (queue, transfer_queue, device)
        } else {
            let mut gpu = unsafe {
                adapter
                    .physical_device
                    .open(
                        &[(family, &[1.0])],
                        hal::Features::NDC_Y_UP | hal::Features::SAMPLER_ANISOTROPY,
                    )
                    .unwrap()
            };
            let queue_group = gpu.queue_groups.pop().unwrap();
            let device = gpu.device;
            let queue = Arc::new(Mutex::new(Queue {
                queue_group: ManuallyDrop::new(queue_group),
            }));
            let transfer_queue = queue.clone();
            (queue, transfer_queue, device)
        };

        let command_pool = unsafe {
            device.create_command_pool(
                queue.lock().unwrap().queue_group.family,
                pool::CommandPoolCreateFlags::empty(),
            )
        }
            .expect("Can't create command pool");

        let frames_in_flight = 3;

        let mut submission_complete_semaphores = Vec::with_capacity(frames_in_flight);
        let mut submission_complete_fences = Vec::with_capacity(frames_in_flight);

        let mut cmd_pools = Vec::with_capacity(frames_in_flight);
        let mut cmd_buffers = Vec::with_capacity(frames_in_flight);

        cmd_pools.push(command_pool);
        for _ in 0..frames_in_flight {
            unsafe {
                cmd_pools.push(
                    device
                        .create_command_pool(
                            queue.lock().unwrap().queue_group.family,
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

        let scene_list = SceneList::new(device.clone(), allocator.clone(), transfer_queue.clone());

        let mesh_renderer = mesh::RenderPipeline::new(
            device.clone(),
            allocator,
            transfer_queue.clone(),
            format,
            width as u32,
            height as u32,
            &scene_list,
        );

        Ok(Box::new(Self {
            instance,
            queue,
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

    fn set_meshes(&mut self, meshes: ChangedIterator<'_, Mesh>) {
        for (i, mesh) in meshes {
            self.scene_list.set_mesh(i, mesh);
        }
    }

    fn set_animated_meshes(&mut self, meshes: ChangedIterator<'_, AnimatedMesh>) {
        for (i, mesh) in meshes {
            self.scene_list.set_anim_mesh(i, mesh);
        }
    }

    fn set_instances(&mut self, instances: ChangedIterator<'_, rfw_scene::Instance>) {
        for (i, instance) in instances {
            self.scene_list.set_instance(i, instance);
        }
    }

    fn set_materials(
        &mut self,
        materials: ChangedIterator<'_, rfw_scene::DeviceMaterial>,
    ) {
        self.mesh_renderer.set_materials(materials.as_slice());
    }

    fn set_textures(&mut self, textures: ChangedIterator<'_, rfw_scene::Texture>) {
        self.mesh_renderer.set_textures(textures.as_slice());
    }

    fn synchronize(&mut self) {
        let scene_list = &mut self.scene_list;
        let mesh_renderer = &mut self.mesh_renderer;

        scene_list.synchronize();
        mesh_renderer.synchronize();
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

            self.mesh_renderer.draw(
                cmd_buffer,
                &framebuffer,
                &self.viewport,
                &self.scene_list,
                &camera.calculate_frustrum(),
            );
            cmd_buffer.finish();

            let submission = Submission {
                command_buffers: iter::once(&*cmd_buffer),
                wait_semaphores: None,
                signal_semaphores: iter::once(&self.submission_complete_semaphores[frame_idx]),
            };

            let result = if let Ok(mut queue) = self.queue.lock() {
                queue.submit(
                    submission,
                    Some(&self.submission_complete_fences[frame_idx]),
                );

                // present frame
                Some(queue.queue_group.queues[0].present_surface(
                    &mut self.surface,
                    surface_image,
                    Some(&self.submission_complete_semaphores[frame_idx]),
                ))
            } else {
                None
            };

            self.device.destroy_framebuffer(framebuffer);

            match result {
                Some(result) => {
                    if result.is_err() {
                        self.recreate_swapchain();
                    }
                }
                _ => {}
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

    fn set_point_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::PointLight>) {}

    fn set_spot_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::SpotLight>) {}

    fn set_area_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::AreaLight>) {}

    fn set_directional_lights(&mut self, lights: ChangedIterator<'_, rfw_scene::DirectionalLight>) {}

    fn set_skybox(&mut self, _skybox: rfw_scene::Texture) {}

    fn set_skins(&mut self, skins: ChangedIterator<'_, Skin>) {
        for (i, skin) in skins {
            self.mesh_renderer.set_skin(i, skin);
        }
    }

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
