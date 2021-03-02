pub use gfx_backend_vulkan as backend;

pub use gfx_hal as hal;

use hal::prelude::*;
use hal::{
    adapter::PhysicalDevice, command, command::CommandBuffer, device::Device, format as f,
    format::ChannelType, pool, pool::CommandPool, pso, queue::QueueFamily, queue::Submission,
    window, window::PresentationSurface, Instance, *,
};
use instances::SceneList;
use mem::Allocator;
use rfw::prelude::*;
use std::{iter, mem::ManuallyDrop, ptr, unimplemented};
use window::Extent2D;

mod cmd;
mod instances;
#[allow(dead_code)]
mod light;
mod mem;
mod mesh;
mod skinning;
mod utils;

use crate::skinning::SkinList;

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

pub use cmd::*;
use rfw::prelude::HasRawWindowHandle;

#[allow(dead_code)]
pub struct GfxRenderer<B: hal::Backend> {
    instance: B::Instance,
    queue: Queue<B>,
    device: DeviceHandle<B>,
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

    window_size: Extent2D,
    render_size: Extent2D,

    scene_list: SceneList<B>,
    mesh_renderer: mesh::RenderPipeline<B>,
    skins: SkinList<B>,

    point_lights: light::LightList<B>,
    spot_lights: light::LightList<B>,
    settings: GfxSettings,
}

#[derive(Debug, Copy, Clone)]
pub struct GfxSettings {
    scale_factor: f64,
}

impl Default for GfxSettings {
    fn default() -> Self {
        Self { scale_factor: 1.0 }
    }
}

impl<B: hal::Backend> rfw::prelude::Backend for GfxRenderer<B> {
    type Settings = GfxSettings;

    fn init<T: HasRawWindowHandle>(
        window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    ) -> Result<Box<Self>, Box<dyn std::error::Error>> {
        let instance: B::Instance = match hal::Instance::create("RFW", 1) {
            Ok(instance) => instance,
            Err(_) => return Err(Box::new(GfxError::UnsupportedBackend)),
        };

        let (render_width, render_height) = (
            (window_size.0 as f64 * scale_factor) as u32,
            (window_size.1 as f64 * scale_factor) as u32,
        );

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
            let queue = Queue::new(queue_group);
            let transfer_queue = Queue::new(transfer_queue_group);
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
            let queue = Queue::new(queue_group);
            let transfer_queue = queue.clone();
            (queue, transfer_queue, device)
        };

        let command_pool = unsafe {
            device.create_command_pool(queue.family, pool::CommandPoolCreateFlags::empty())
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
                        .create_command_pool(queue.family, pool::CommandPoolCreateFlags::empty())
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
                width: window_size.0 as u32,
                height: window_size.1 as u32,
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

        let device: DeviceHandle<B> = DeviceHandle::new(device);
        let allocator = Allocator::new(device.clone(), &adapter);

        let task_pool = TaskPool::default();

        let scene_list = SceneList::new(
            device.clone(),
            allocator.clone(),
            transfer_queue.clone(),
            &task_pool,
        );
        let skins = SkinList::new(
            device.clone(),
            allocator.clone(),
            transfer_queue.clone(),
            &task_pool,
        );

        let mesh_renderer = mesh::RenderPipeline::new(
            device.clone(),
            allocator.clone(),
            transfer_queue.clone(),
            format,
            render_width as u32,
            render_height as u32,
            &scene_list,
            &skins,
        );

        // device: Arc<B::Device>,
        // allocator: Allocator<B>,
        // instances_desc_layout: &B::DescriptorSetLayout,
        // skins_desc_layout: &B::DescriptorSetLayout,
        // capacity: usize,

        let point_lights = light::LightList::new(
            device.clone(),
            allocator.clone(),
            &*scene_list.set_layout,
            &*skins.desc_layout,
            32,
        );

        let spot_lights = light::LightList::new(
            device.clone(),
            allocator.clone(),
            &*scene_list.set_layout,
            &*skins.desc_layout,
            32,
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
            window_size: Extent2D {
                width: window_size.0 as u32,
                height: window_size.1 as u32,
            },
            render_size: Extent2D {
                width: render_width,
                height: render_height,
            },
            scene_list,
            mesh_renderer,
            skins,
            point_lights,
            spot_lights,
            settings: GfxSettings { scale_factor },
        }))
    }

    fn set_2d_mesh(&mut self, _id: usize, _mesh: MeshData2D) {
        // unimplemented!()
    }

    fn set_2d_instances(&mut self, _instances: InstancesData2D<'_>) {
        unimplemented!()
    }

    fn set_3d_mesh(&mut self, id: usize, mesh: MeshData3D) {
        self.scene_list.set_mesh(id, mesh);
    }

    fn unload_3d_meshes(&mut self, ids: Vec<usize>) {
        for id in ids {
            self.scene_list.remove_mesh(id);
        }
    }

    fn set_3d_instances(&mut self, _instances: InstancesData3D<'_>) {
        unimplemented!()
        // for (i, instance) in instances {
        //     self.scene_list.set_instance(i, instance);
        // }
    }

    fn unload_3d_instances(&mut self, ids: Vec<usize>) {
        let instance = rfw::prelude::Instance3D::default();
        for id in ids {
            self.scene_list.set_instance(id, &instance);
        }
    }

    fn set_materials(&mut self, materials: &[DeviceMaterial], _changed: &BitSlice) {
        self.mesh_renderer.set_materials(materials);
    }

    fn set_textures(&mut self, textures: &[TextureData<'_>], changed: &BitSlice) {
        self.mesh_renderer.set_textures(textures, changed);
    }

    fn synchronize(&mut self) {
        self.skins.synchronize();
        self.scene_list.synchronize();
    }

    fn render(&mut self, camera: CameraView3D, _mode: rfw::prelude::RenderMode) {
        self.mesh_renderer.update_camera(&camera);

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
            self.mesh_renderer.create_frame_buffer(
                &surface_image,
                Extent2D {
                    width: self.window_size.width as _,
                    height: self.window_size.height as _,
                },
            )
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

            self.mesh_renderer.draw(
                cmd_buffer,
                &framebuffer,
                &self.viewport,
                &self.scene_list,
                &self.skins,
                &FrustrumG::from(&camera),
            );

            cmd_buffer.finish();

            let submission = Submission {
                command_buffers: iter::once(&*cmd_buffer),
                wait_semaphores: None,
                signal_semaphores: iter::once(&self.submission_complete_semaphores[frame_idx]),
            };

            self.queue.submit(
                submission,
                Some(&self.submission_complete_fences[frame_idx]),
            );

            // present frame
            let result = self.queue.present(
                &mut self.surface,
                surface_image,
                Some(&self.submission_complete_semaphores[frame_idx]),
            );

            if result.is_err() {
                self.recreate_swapchain();
            }

            self.device.destroy_framebuffer(framebuffer);
        }

        // Increment our frame
        self.frame += 1;
    }

    fn resize<T: HasRawWindowHandle>(
        &mut self,
        _window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    ) {
        self.settings.scale_factor = scale_factor;
        self.device.wait_idle().unwrap();
        self.window_size = Extent2D {
            width: window_size.0 as u32,
            height: window_size.1 as u32,
        };
        self.render_size = Extent2D {
            width: (window_size.0 as f64 * scale_factor) as u32,
            height: (window_size.1 as f64 * scale_factor) as u32,
        };

        self.recreate_swapchain();
        self.mesh_renderer
            .resize(self.render_size.width, self.render_size.height);
    }

    fn set_point_lights(&mut self, _lights: &[PointLight], _changed: &BitSlice) {}
    fn set_spot_lights(&mut self, _lights: &[SpotLight], _changed: &BitSlice) {}
    fn set_area_lights(&mut self, _lights: &[AreaLight], _changed: &BitSlice) {}
    fn set_directional_lights(&mut self, _lights: &[DirectionalLight], _changed: &BitSlice) {}

    fn set_skybox(&mut self, _skybox: TextureData) {}

    fn set_skins(&mut self, skins: &[SkinData], changed: &BitSlice) {
        for i in 0..skins.len() {
            if !changed[i] {
                continue;
            }

            self.skins.set_skin(i, skins[i]);
        }
    }

    fn settings(&mut self) -> &mut Self::Settings {
        &mut self.settings
    }
}

impl<B: hal::Backend> GfxRenderer<B> {
    fn recreate_swapchain(&mut self) {
        let caps = self.surface.capabilities(&self.adapter.physical_device);
        let swap_config = window::SwapchainConfig::from_caps(&caps, self.format, self.window_size);
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

            self.surface.unconfigure_swapchain(&self.device);
            let surface = ManuallyDrop::into_inner(ptr::read(&self.surface));
            self.instance.destroy_surface(surface);
        }
    }
}
