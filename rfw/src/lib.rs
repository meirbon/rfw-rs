mod ecs;
mod resources;
mod system;

pub use rfw_backend as backend;
pub use rfw_math as math;
pub use rfw_scene as scene;
pub use rfw_utils as utils;

pub use ecs::*;
pub use resources::*;
pub use system::*;

pub mod prelude {
    pub use crate::*;
    pub use rfw_backend::*;
    pub use rfw_math::*;
    pub use rfw_scene::bvh::*;
    pub use rfw_scene::*;
    pub use rfw_utils::collections::*;
    pub use rfw_utils::task::*;
    pub use rfw_utils::*;
}

use rfw_backend::{Backend, DirectionalLight, PointLight, RenderMode, SpotLight, TextureData};
use rfw_math::*;
use rfw_scene::{Camera3D, GraphHandle, Scene, SceneError};
use rfw_scene::{Flip, Texture};
use rfw_utils::BytesConversion;
use std::error::Error;
use std::path::Path;

pub struct PointLightRef(u32);
pub struct SpotLightRef(u32);
pub struct DirectionalLightRef(u32);

pub struct Instance<T: Sized + Backend> {
    pub resources: ResourceList,
    scheduler: ecs::Scheduler,
    renderer: Box<T>,
}

pub enum CameraRef3D<'a> {
    ID(usize),
    Ref(&'a Camera3D),
}

impl<'a> From<&'a Camera3D> for CameraRef3D<'a> {
    fn from(cam: &'a Camera3D) -> Self {
        Self::Ref(cam)
    }
}

impl From<usize> for CameraRef3D<'_> {
    fn from(id: usize) -> Self {
        Self::ID(id)
    }
}

pub enum CameraRef2D<'a> {
    Ref(&'a scene::Camera2D),
}

impl<'a> From<&'a scene::Camera2D> for CameraRef2D<'a> {
    fn from(cam: &'a scene::Camera2D) -> Self {
        Self::Ref(cam)
    }
}

impl<T: Sized + Backend> Instance<T> {
    pub fn new<B: rfw_backend::HasRawWindowHandle>(
        window: &B,
        window_size: (u32, u32),
        scale_factor: Option<f64>,
    ) -> Result<Self, Box<dyn Error>> {
        let renderer = T::init(window, window_size, scale_factor.unwrap_or(1.0))?;
        let mut resources = ResourceList::new();
        resources.add_resource(RenderSystem {
            width: window_size.0,
            height: window_size.1,
            scale_factor: scale_factor.unwrap_or(1.0),
        });
        resources.add_resource(Scene::new());

        Ok(Self {
            resources,
            scheduler: Default::default(),
            renderer,
        })
    }

    #[cfg(feature = "serde")]
    pub fn from_scene<B: rfw_backend::HasRawWindowHandle, P: AsRef<Path>>(
        scene: P,
        window: &B,
        window_size: (u32, u32),
        scale_factor: Option<f64>,
    ) -> Result<Self, Box<dyn Error>> {
        let renderer = T::init(window, window_size, render_size)?;

        resources.add_resource(System {
            width: window_size.0,
            height: window_size.1,
            scale_factor: scale_factor.unwrap_or(1.0),
        });
        resources.add_resource(Scene::deserialize(scene)?);

        Ok(Self {
            resources,
            scheduler: Default::default(),
            renderer,
        })
    }

    pub fn get_scene(&self) -> LockedValue<'_, Box<dyn ResourceStorage>, Scene> {
        self.resources.get_resource::<Scene>().unwrap()
    }

    pub fn get_scene_mut(&mut self) -> LockedValueMut<'_, Box<dyn ResourceStorage>, Scene> {
        self.resources.get_resource_mut::<Scene>().unwrap()
    }

    pub fn with_resource<P: resources::Resource>(mut self, resource: P) -> Self {
        self.resources.add_resource(resource);
        self
    }

    pub fn with_plugin<P: ecs::Plugin + Resource>(mut self, mut plugin: P) -> Self {
        plugin.init(&mut self.resources, &mut self.scheduler);
        self.resources.add_resource(plugin);
        self
    }

    pub fn with_system<S: ecs::System>(mut self, system: S) -> Self {
        self.scheduler.add_system(system);
        self
    }

    pub fn resize<B: rfw_backend::HasRawWindowHandle>(
        &mut self,
        window: &B,
        window_size: (u32, u32),
        scale_factor: Option<f64>,
    ) {
        let mut system = self.resources.get_resource_mut::<RenderSystem>().unwrap();
        let scale_factor = scale_factor.unwrap_or(system.scale_factor);
        self.renderer.resize(window, window_size, scale_factor);
        system.width = window_size.0;
        system.height = window_size.1;
        system.scale_factor = scale_factor;
    }

    pub fn render<'a, C2: Into<CameraRef2D<'a>>, C3: Into<CameraRef3D<'a>>>(
        &mut self,
        camera_2d: C2,
        camera_3d: C3,
        mode: RenderMode,
    ) -> Result<(), SceneError> {
        self.scheduler.run(&self.resources);

        let mut scene = self.resources.get_resource_mut::<Scene>().unwrap();
        let mut system = self.resources.get_resource_mut::<RenderSystem>().unwrap();
        system.synchronize(&mut scene, &mut *self.renderer);

        let view_3d = match camera_3d.into() {
            CameraRef3D::ID(id) => {
                if let Some(c) = scene.cameras.get(id) {
                    c
                } else {
                    return Err(SceneError::InvalidCameraID(id));
                }
            }
            CameraRef3D::Ref(reference) => reference,
        }
        .get_view(system.width, system.height);

        let view_2d = match camera_2d.into() {
            CameraRef2D::Ref(reference) => reference,
        }
        .get_view();

        self.renderer.render(view_2d, view_3d, mode);
        Ok(())
    }

    /// Will return a reference to the point light if the scene is not locked
    pub fn add_point_light<B: Into<[f32; 3]>>(
        &mut self,
        position: B,
        radiance: B,
    ) -> PointLightRef {
        let mut scene = self.resources.get_resource_mut::<Scene>().unwrap();

        let position: Vec3 = Vec3::from(position.into());
        let radiance: Vec3 = Vec3::from(radiance.into());

        let light = PointLight::new(position, radiance);
        let id = scene.lights.point_lights.push(light);

        PointLightRef(id as u32)
    }

    /// Will return a reference to the spot light if the scene is not locked
    pub fn add_spot_light<B: Into<[f32; 3]>>(
        &mut self,
        position: B,
        direction: B,
        radiance: B,
        inner_degrees: f32,
        outer_degrees: f32,
    ) -> SpotLightRef {
        let mut scene = self.resources.get_resource_mut::<Scene>().unwrap();

        let position = Vec3::from(position.into());
        let direction = Vec3::from(direction.into());
        let radiance = Vec3::from(radiance.into());

        let light = SpotLight::new(position, direction, inner_degrees, outer_degrees, radiance);

        let id = scene.lights.spot_lights.push(light);
        SpotLightRef(id as u32)
    }

    /// Will return a reference to the directional light if the scene is not locked
    pub fn add_directional_light<B: Into<[f32; 3]>>(
        &mut self,
        direction: B,
        radiance: B,
    ) -> DirectionalLightRef {
        let mut scene = self.resources.get_resource_mut::<Scene>().unwrap();
        let light =
            DirectionalLight::new(Vec3::from(direction.into()), Vec3::from(radiance.into()));
        let id = scene.lights.directional_lights.push(light);
        DirectionalLightRef(id as u32)
    }

    pub fn set_animation_time(&mut self, handle: &GraphHandle, time: f32) {
        let mut scene = self.resources.get_resource_mut::<Scene>().unwrap();
        scene.objects.graph.set_animation(handle, time);
    }

    pub fn set_animations_time(&mut self, time: f32) {
        let mut scene = self.resources.get_resource_mut::<Scene>().unwrap();
        scene.objects.graph.set_animations(time);
    }

    pub fn get_settings(&mut self) -> &mut T::Settings {
        self.renderer.settings()
    }

    pub fn set_skybox<B: AsRef<Path>>(&mut self, path: B) -> Result<(), SceneError> {
        if let Ok(texture) = Texture::load(path.as_ref(), Flip::FlipV) {
            self.renderer.set_skybox(TextureData {
                width: texture.width,
                height: texture.height,
                mip_levels: texture.mip_levels,
                bytes: texture.data.as_bytes(),
                format: rfw_backend::DataFormat::BGRA8,
            });
            Ok(())
        } else {
            Err(SceneError::LoadError(path.as_ref().to_path_buf()))
        }
    }

    #[cfg(feature = "serde")]
    pub fn save_scene<B: AsRef<Path>>(&self, path: B) -> Result<(), ()> {
        match self.scene.serialize(path) {
            Ok(_) => Ok(()),
            _ => Err(()),
        }
    }
}
