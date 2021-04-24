pub mod ecs;
pub mod resources;
pub mod system;

pub use rfw_backend as backend;
pub use rfw_math as math;
pub use rfw_scene as scene;
pub use rfw_utils as utils;

pub mod prelude {
    pub use crate::ecs::*;
    pub use crate::resources::*;
    pub use crate::system::*;
    pub use crate::Instance;

    pub use rfw_backend::*;
    pub use rfw_math::*;
    pub use rfw_scene::bvh::*;
    pub use rfw_scene::*;
    pub use rfw_utils::collections::*;
    pub use rfw_utils::task::*;
    pub use rfw_utils::*;
}

use crate::ecs::component::Component;
use crate::ecs::schedule::SystemDescriptor;
use crate::resources::Resource;
use ecs::*;
use rfw_backend::{Backend, DirectionalLight, PointLight, RenderMode, SpotLight};
use rfw_math::*;
use rfw_scene::{Camera2D, Camera3D, GraphHandle, Scene, SceneError};
use rfw_scene::{Flip, Texture};
use rfw_utils::track::Tracked;
use std::error::Error;
use system::RenderSystem;

pub struct PointLightRef(u32);

pub struct SpotLightRef(u32);

pub struct DirectionalLightRef(u32);

pub struct Instance {
    scheduler: ecs::Scheduler,
    world: ecs::World,
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

impl Instance {
    pub fn new<T: 'static + Sized + Backend>(
        renderer: T,
        window_size: (u32, u32),
        scale_factor: Option<f64>,
    ) -> Result<Self, Box<dyn Error>> {
        rfw_utils::log::SimpleLogger::new().init().unwrap();
        rfw_utils::log::info!("initialized renderer: {}", std::any::type_name::<T>());

        let mut world = World::new();
        let mut scheduler = Scheduler::default();

        let mut system = RenderSystem {
            width: window_size.0,
            height: window_size.1,
            scale_factor: scale_factor.unwrap_or(1.0),
            renderer: Box::new(renderer),
            mode: RenderMode::Default,
        };

        system.init(&mut world, &mut scheduler);
        world.insert_resource(system);
        world.insert_resource(Scene::new());
        world.insert_resource(
            Camera3D::new().with_aspect_ratio(window_size.0 as f32 / window_size.1 as f32),
        );
        world.insert_resource(Camera2D::from_width_height(
            window_size.0,
            window_size.1,
            scale_factor,
        ));
        scheduler.add_system(ecs::CoreStage::PostUpdate, render_system.system());

        Ok(Self { scheduler, world })
    }

    #[cfg(feature = "serde")]
    pub fn from_scene<T: 'static + Sized + Backend, P: AsRef<Path>>(
        scene: P,
        renderer: T,
        window_size: (u32, u32),
        scale_factor: Option<f64>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut result = Self::new(renderer, window_size, scale_factor)?;
        *result.world.get_resource_mut::<Scene>().unwrap() = Scene::deserialize(scene)?;
        Ok(result)
    }

    pub fn spawn(&mut self) -> ecs::world::EntityMut {
        self.world.spawn()
    }

    pub fn get_resource<T: Component>(&self) -> Option<&T> {
        self.world.get_resource()
    }

    pub fn get_resource_mut<T: Component>(&mut self) -> Option<Mut<T>> {
        self.world.get_resource_mut()
    }

    pub fn get_scene(&self) -> &Scene {
        self.world.get_resource::<Scene>().unwrap()
    }

    pub fn get_scene_mut(&mut self) -> ecs::Mut<Scene> {
        self.world.get_resource_mut().unwrap()
    }

    pub fn get_camera_2d(&mut self) -> ecs::Mut<Camera2D> {
        self.world.get_resource_mut().unwrap()
    }

    pub fn get_camera_3d(&mut self) -> ecs::Mut<Camera3D> {
        self.world.get_resource_mut().unwrap()
    }

    pub fn with_resource<P: ecs::component::Component>(mut self, resource: P) -> Self {
        self.world.insert_resource(resource);
        self
    }

    pub fn with_plugin<P: ecs::Plugin + Resource>(mut self, mut plugin: P) -> Self {
        plugin.init(&mut self.world, &mut self.scheduler);
        self.world.insert_resource(plugin);
        self
    }

    pub fn with_system(mut self, system: impl Into<SystemDescriptor>) -> Self {
        self.scheduler.add_system(CoreStage::Update, system);
        self
    }

    pub fn with_system_at_stage(
        mut self,
        stage: impl StageLabel,
        system: impl Into<SystemDescriptor>,
    ) -> Self {
        self.scheduler.add_system(stage, system);
        self
    }

    pub fn with_system_set(mut self, system_set: SystemSet) -> Self {
        self.scheduler.add_system_set(CoreStage::Update, system_set);
        self
    }

    pub fn with_system_set_at_stage(
        mut self,
        stage: impl StageLabel,
        system_set: SystemSet,
    ) -> Self {
        self.scheduler.add_system_set(stage, system_set);
        self
    }

    pub fn resize(&mut self, window_size: (u32, u32), scale_factor: Option<f64>) {
        let mut system = self.world.get_resource_mut::<RenderSystem>().unwrap();
        let scale_factor = scale_factor.unwrap_or(system.scale_factor);
        system.renderer.resize(window_size, scale_factor);
        system.width = window_size.0;
        system.height = window_size.1;
        system.scale_factor = scale_factor;
    }

    pub fn render(&mut self) {
        self.scheduler.run(&mut self.world);
    }

    /// Will return a reference to the point light if the scene is not locked
    pub fn add_point_light<B: Into<[f32; 3]>>(
        &mut self,
        position: B,
        radiance: B,
    ) -> PointLightRef {
        let mut scene = self.world.get_resource_mut::<Scene>().unwrap();

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
        let mut scene = self.world.get_resource_mut::<Scene>().unwrap();

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
        let mut scene = self.world.get_resource_mut::<Scene>().unwrap();
        let light =
            DirectionalLight::new(Vec3::from(direction.into()), Vec3::from(radiance.into()));
        let id = scene.lights.directional_lights.push(light);
        DirectionalLightRef(id as u32)
    }

    pub fn set_animation_time(&mut self, handle: &GraphHandle, time: f32) {
        let mut scene = self.world.get_resource_mut::<Scene>().unwrap();
        scene.objects.graph.set_animation(handle, time);
    }

    pub fn set_animations_time(&mut self, time: f32) {
        let mut scene = self.world.get_resource_mut::<Scene>().unwrap();
        scene.objects.graph.set_animations(time);
    }

    // pub fn set_skybox<B: AsRef<Path>>(&mut self, path: B) -> Result<(), SceneError> {
    //     if let Ok(texture) = Texture::load(path.as_ref(), Flip::FlipV) {
    //         self.renderer.set_skybox(TextureData {
    //             width: texture.width,
    //             height: texture.height,
    //             mip_levels: texture.mip_levels,
    //             bytes: texture.data.as_bytes(),
    //             format: rfw_backend::DataFormat::BGRA8,
    //         });
    //
    //         Ok(())
    //     } else {
    //         Err(SceneError::LoadError(path.as_ref().to_path_buf()))
    //     }
    // }

    #[cfg(feature = "serde")]
    pub fn save_scene<B: AsRef<Path>>(&self, path: B) -> Result<(), ()> {
        match self.scene.serialize(path) {
            Ok(_) => Ok(()),
            _ => Err(()),
        }
    }
}

pub fn render_system(
    camera_2d: Res<Camera2D>,
    camera_3d: Res<Camera3D>,
    mut system: ResMut<RenderSystem>,
) {
    let view_2d = camera_2d.get_view();
    let view_3d = camera_3d.get_view(system.width, system.height);
    let mode = system.mode;
    system.renderer.render(view_2d, view_3d, mode);
}
