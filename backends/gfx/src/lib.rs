pub use list::*;
use rendy::{factory::{BasicDevicesConfigure, BasicHeapsConfigure, Config, ImageState, ImageStateOrLayout, OneGraphicsQueue}, hal::{
        self,
        device::Device,
        format::{Aspects, Format},
        image::{self, Usage},
        window::{Extent2D, PresentMode},
    }, init::Rendy, memory, resource::{Escape, Handle, Image, ImageInfo, SubresourceLayers, ViewCapabilities}};
use rfw::prelude::*;
use std::{collections::HashMap, mem::ManuallyDrop};

mod pipeline;
pub use pipeline::*;
// mod list;

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

pub type GfxBackend = GfxBackendInternal<rendy::vulkan::Backend>;

#[allow(dead_code)]
pub struct GfxBackendInternal<B: hal::Backend> {
    rendy: Rendy<B>,
    target: ManuallyDrop<rendy::wsi::Target<B>>,
    present_semaphore: ManuallyDrop<B::Semaphore>,
    textures: HashMap<usize, Handle<Image<B>>>,
    settings: GfxSettings,
}

#[derive(Debug, Copy, Clone)]
pub struct GfxSettings {
    image_count: u32,
    format: Format,
    present_mode: PresentMode,
}

impl<B: hal::Backend> rfw::prelude::Backend for GfxBackendInternal<B> {
    type Settings = GfxSettings;

    fn init<T: HasRawWindowHandle>(
        window: &T,
        window_size: (u32, u32),
        _scale_factor: f64,
    ) -> Result<Box<Self>, Box<dyn std::error::Error>> {
        let config: Config<BasicDevicesConfigure, BasicHeapsConfigure, OneGraphicsQueue> =
            Config::default();

        let mut rendy = Rendy::<B>::init(&config)?;

        let surface = rendy.factory.create_surface(window)?;
        let format = rendy.factory.get_surface_format(&surface);
        let capabilities = rendy.factory.get_surface_capabilities(&surface);

        let image_count = 3_u32.min(*capabilities.image_count.end());
        let present_mode = PresentMode::FIFO;

        let target = rendy
            .factory
            .create_target(
                surface,
                Extent2D {
                    width: window_size.0,
                    height: window_size.1,
                },
                image_count,
                present_mode,
                Usage::COLOR_ATTACHMENT,
            )
            .expect("Could not create swapchain.");

        let present_semaphore = rendy
            .factory
            .create_semaphore()
            .expect("Could not create present semaphore.");

        Ok(Box::new(Self {
            rendy,
            target: ManuallyDrop::new(target),
            present_semaphore: ManuallyDrop::new(present_semaphore),
            textures: Default::default(),
            settings: GfxSettings {
                image_count,
                format,
                present_mode,
            },
        }))
    }

    fn set_2d_mesh(&mut self, id: usize, data: MeshData2D<'_>) {}

    fn set_2d_instances(&mut self, mesh: usize, instances: InstancesData2D<'_>) {}

    fn set_3d_mesh(&mut self, id: usize, mesh: MeshData3D) {}

    fn unload_3d_meshes(&mut self, ids: Vec<usize>) {}

    fn set_3d_instances(&mut self, mesh: usize, instances: InstancesData3D<'_>) {}

    fn set_materials(&mut self, materials: &[DeviceMaterial], _changed: &BitSlice) {}

    fn set_textures(&mut self, textures: &[TextureData<'_>], changed: &BitSlice) {
        for (i, (t, changed)) in textures.iter().zip(changed.iter()).enumerate() {
            if changed == false {
                continue;
            }

            let image = if let Some(image) = self.textures.get_mut(&i) {
                let info = image.info();
                if match info.kind {
                    rendy::resource::Kind::D1(_, _) => true,
                    rendy::resource::Kind::D2(w, h, m, _) => {
                        w != t.width || h != t.height || m != (t.mip_levels as u16)
                    }
                    rendy::resource::Kind::D3(_, _, _) => true,
                } {
                    *image = self
                        .rendy
                        .factory
                        .create_image(
                            ImageInfo {
                                kind: image::Kind::D2(t.width, t.height, t.mip_levels as _, 1),
                                levels: 1,
                                format: Format::Bgr8Unorm,
                                tiling: rendy::resource::Tiling::Optimal,
                                view_caps: ViewCapabilities::empty(),
                                usage: Usage::SAMPLED | Usage::TRANSFER_DST,
                            },
                            memory::Data,
                        )
                        .unwrap()
                        .into();
                }
                image
            } else {
                self.textures.insert(
                    i,
                    self.rendy
                        .factory
                        .create_image(
                            ImageInfo {
                                kind: image::Kind::D2(t.width, t.height, t.mip_levels as _, 1),
                                levels: 1,
                                format: Format::Bgr8Unorm,
                                tiling: rendy::resource::Tiling::Optimal,
                                view_caps: ViewCapabilities::empty(),
                                usage: Usage::SAMPLED | Usage::TRANSFER_DST,
                            },
                            memory::Data,
                        )
                        .unwrap()
                        .into(),
                );
                self.textures.get_mut(&i).unwrap()
            };

            unsafe {
                let x_offset = 0;
                let y_offset = 0;
                for i in 0..t.mip_levels {
                    let (w, h) = t.mip_level_width_height(i as usize);
                    self.rendy.factory.upload_image(
                        image.clone(),
                        t.width,
                        t.height,
                        SubresourceLayers {
                            aspects: Aspects::COLOR,
                            layers: 0..1,
                            level: i as _,
                        },
                        image::Offset {
                            x: x_offset as _,
                            y: y_offset as _,
                            z: 0,
                        },
                        image::Extent {
                            width: w as _,
                            height: h as _,
                            depth: 1,
                        },
                        t.bytes,
                        ImageStateOrLayout::undefined(),
                        self.rendy.factory
                        ImageState {
                            access: image::Access::SHADER_READ,
                            layout: image::Layout::ShaderReadOnlyOptimal,
                            queue: self.rendy.families.
                        },
                    );
                    x_offset += w;
                    y_offset += h;
                }
            }
        }
    }

    fn synchronize(&mut self) {}

    fn render(&mut self, _view_2d: CameraView2D, view_3d: CameraView3D, _mode: RenderMode) {
        self.rendy.factory.maintain(&mut self.rendy.families);
        let next_image = match unsafe { self.target.next_image(&self.present_semaphore) } {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Acquire image error: {}", e);
                return;
            }
        };
    }

    fn resize<T: HasRawWindowHandle>(
        &mut self,
        _window: &T,
        window_size: (u32, u32),
        scale_factor: f64,
    ) {
        unsafe {
            let surface = self
                .rendy
                .factory
                .destroy_target(ManuallyDrop::take(&mut self.target));
            self.target = ManuallyDrop::new(
                self.rendy
                    .factory
                    .create_target(
                        surface,
                        Extent2D {
                            width: window_size.0,
                            height: window_size.1,
                        },
                        self.settings.image_count,
                        self.settings.present_mode,
                        Usage::COLOR_ATTACHMENT,
                    )
                    .expect("Could not recreate target."),
            );
        }
    }

    fn set_point_lights(&mut self, _lights: &[PointLight], _changed: &BitSlice) {}
    fn set_spot_lights(&mut self, _lights: &[SpotLight], _changed: &BitSlice) {}
    fn set_area_lights(&mut self, _lights: &[AreaLight], _changed: &BitSlice) {}
    fn set_directional_lights(&mut self, _lights: &[DirectionalLight], _changed: &BitSlice) {}

    fn set_skybox(&mut self, _skybox: TextureData) {}

    fn set_skins(&mut self, skins: &[SkinData], changed: &BitSlice) {}

    fn settings(&mut self) -> &mut Self::Settings {
        &mut self.settings
    }
}

impl<B: hal::Backend> Drop for GfxBackendInternal<B> {
    fn drop(&mut self) {
        self.rendy.factory.wait_idle().unwrap();
        unsafe {
            self.rendy
                .factory
                .destroy_semaphore(ManuallyDrop::take(&mut self.present_semaphore));
            self.rendy.factory.cleanup(&self.rendy.families);
            let surface = self
                .rendy
                .factory
                .destroy_target(ManuallyDrop::take(&mut self.target));
            self.rendy.factory.destroy_surface(surface);
        }
    }
}
