use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

static mut USE_MBVH: bool = true;

pub type PrimID = i32;
pub type InstanceID = i32;

pub mod camera;
pub mod constants;
pub mod graph;
pub mod intersector;
pub mod lights;
pub mod loaders;
pub mod material;
pub mod objects;
pub mod renderers;
pub mod triangle_scene;

pub mod utils;

pub use camera::*;
pub use intersector::*;
pub use lights::*;
pub use loaders::*;
pub use material::*;
pub use objects::*;
pub use renderers::*;
pub use utils::*;

use std::sync::{Arc, Mutex};

pub use bitvec::prelude::*;
pub use raw_window_handle;
pub use triangle_scene::*;

use crate::utils::{FlaggedIterator, FlaggedIteratorMut};
use glam::*;
use graph::Node;
use std::error::Error;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum SceneLight {
    Point(PointLight),
    Spot(SpotLight),
    Directional(DirectionalLight),
}

pub struct LightRef {
    id: usize,
    light: SceneLight,
    lights: Arc<Mutex<SceneLights>>,
}

impl LightRef {
    fn new(id: usize, light: SceneLight, lights: Arc<Mutex<SceneLights>>) -> Self {
        Self { id, light, lights }
    }

    pub fn get(&self) -> &SceneLight {
        &self.light
    }

    pub fn get_mut(&mut self) -> &mut SceneLight {
        &mut self.light
    }

    pub fn translate_x(&mut self, offset: f32) {
        let translation = Vec3::new(offset, 0.0, 0.0);
        match &mut self.light {
            SceneLight::Spot(l) => {
                let position: Vec3 = Vec3::from(l.position) + translation;
                l.position = position.into();
            }
            _ => {}
        }
    }

    pub fn translate_y(&mut self, offset: f32) {
        let translation = Vec3::new(0.0, offset, 0.0);
        match &mut self.light {
            SceneLight::Spot(l) => {
                let position: Vec3 = Vec3::from(l.position) + translation;
                l.position = position.into();
            }
            _ => {}
        }
    }

    pub fn translate_z(&mut self, offset: f32) {
        let translation = Vec3::new(0.0, 0.0, offset);
        match &mut self.light {
            SceneLight::Spot(l) => {
                let position: Vec3 = Vec3::from(l.position) + translation;
                l.position = position.into();
            }
            _ => {}
        }
    }

    pub fn rotate_x(&mut self, degrees: f32) {
        let rotation = Mat4::from_rotation_x(degrees.to_radians());
        match &mut self.light {
            SceneLight::Spot(l) => {
                let direction: Vec3 = l.direction.into();
                let direction = rotation * direction.extend(0.0);
                l.direction = direction.truncate().into();
            }
            SceneLight::Directional(l) => {
                let direction: Vec3 = l.direction.into();
                let direction = rotation * direction.extend(0.0);
                l.direction = direction.truncate().into();
            }
            _ => {}
        }
    }

    pub fn rotate_y(&mut self, degrees: f32) {
        let rotation = Mat4::from_rotation_y(degrees.to_radians());
        match &mut self.light {
            SceneLight::Spot(l) => {
                let direction: Vec3 = l.direction.into();
                let direction = rotation * direction.extend(0.0);
                l.direction = direction.truncate().into();
            }
            SceneLight::Directional(l) => {
                let direction: Vec3 = l.direction.into();
                let direction = rotation * direction.extend(0.0);
                l.direction = direction.truncate().into();
            }
            _ => {}
        }
    }

    pub fn rotate_z(&mut self, degrees: f32) {
        let rotation = Mat4::from_rotation_z(degrees.to_radians());
        match &mut self.light {
            SceneLight::Spot(l) => {
                let direction: Vec3 = l.direction.into();
                let direction = rotation * direction.extend(0.0);
                l.direction = direction.truncate().into();
            }
            SceneLight::Directional(l) => {
                let direction: Vec3 = l.direction.into();
                let direction = rotation * direction.extend(0.0);
                l.direction = direction.truncate().into();
            }
            _ => {}
        }
    }

    pub fn synchronize(&self) -> Result<(), ()> {
        if let Ok(mut lights) = self.lights.try_lock() {
            match &self.light {
                SceneLight::Point(l) => {
                    lights.point_lights[self.id] = l.clone();
                }
                SceneLight::Spot(l) => {
                    lights.spot_lights[self.id] = l.clone();
                }
                SceneLight::Directional(l) => {
                    lights.directional_lights[self.id] = l.clone();
                }
            }

            Ok(())
        } else {
            Err(())
        }
    }
}

pub struct RenderSystem<T: Sized + Renderer> {
    scene: TriangleScene,
    renderer: Arc<Mutex<Box<T>>>,
}

impl<T: Sized + Renderer> RenderSystem<T> {
    pub fn new<B: raw_window_handle::HasRawWindowHandle>(
        window: &B,
        width: usize,
        height: usize,
    ) -> Result<Self, Box<dyn Error>> {
        let renderer = T::init(window, width, height)?;

        Ok(Self {
            scene: TriangleScene::new(),
            renderer: Arc::new(Mutex::new(renderer)),
        })
    }

    pub fn iter_instances<C>(&self, cb: C)
        where
            C: FnOnce(FlaggedIterator<'_, Instance>),
    {
        let lock = self.scene.objects.instances.lock().unwrap();
        cb(lock.iter());
    }

    pub fn iter_instances_mut<C>(&self, cb: C)
        where
            C: FnOnce(FlaggedIteratorMut<'_, Instance>),
    {
        let mut lock = self.scene.objects.instances.lock().unwrap();
        cb(lock.iter_mut());
    }

    pub fn get_instance<C>(&self, index: usize, cb: C)
        where
            C: FnOnce(Option<&Instance>),
    {
        let lock = self.scene.objects.instances.lock().unwrap();
        cb(lock.get(index))
    }

    pub fn get_instance_mut<C>(&self, index: usize, cb: C)
        where
            C: FnOnce(Option<&mut Instance>),
    {
        let mut lock = self.scene.objects.instances.lock().unwrap();
        cb(lock.get_mut(index))
    }

    pub fn get_lights<C>(&self, cb: C)
        where C: FnOnce(&SceneLights) {
        let lock = self.scene.lights.lock().unwrap();
        cb(&lock)
    }

    pub fn get_lights_mut<C>(&self, cb: C)
        where C: FnOnce(&mut SceneLights) {
        let mut lock = self.scene.lights.lock().unwrap();
        cb(&mut lock)
    }

    pub fn get_node<C>(&self, index: usize, cb: C)
        where
            C: FnOnce(Option<&Node>),
    {
        let lock = self.scene.objects.nodes.lock().unwrap();
        cb(lock.get(index))
    }

    pub fn get_node_mut<C>(&self, index: u32, cb: C)
        where
            C: FnOnce(Option<&mut Node>),
    {
        let mut lock = self.scene.objects.nodes.lock().unwrap();
        cb(lock.get_mut(index as usize))
    }

    pub fn find_mesh_by_name(&self, name: String) -> Vec<ObjectRef> {
        let mut result = Vec::new();
        if let (Ok(meshes), Ok(anim_meshes)) = (
            self.scene.objects.meshes.lock(),
            self.scene.objects.animated_meshes.lock(),
        ) {
            for m_id in 0..meshes.len() {
                if let Some(m) = meshes.get(m_id) {
                    if m.name == name {
                        result.push(ObjectRef::Static(m_id as u32));
                    }
                }
            }

            for m_id in 0..anim_meshes.len() {
                if let Some(m) = anim_meshes.get(m_id) {
                    if m.name == name {
                        result.push(ObjectRef::Animated(m_id as u32));
                    }
                }
            }
        }

        result
    }

    pub fn resize<B: raw_window_handle::HasRawWindowHandle>(
        &self,
        window: &B,
        width: usize,
        height: usize,
    ) {
        self.renderer.lock().unwrap().resize(window, width, height);
    }

    pub fn render(&self, camera: &Camera, mode: RenderMode) {
        if let Ok(mut renderer) = self.renderer.try_lock() {
            renderer.render(camera, mode);
        }
    }

    pub fn load<B: AsRef<Path>>(&self, path: B) -> Result<LoadResult, triangle_scene::SceneError> {
        futures::executor::block_on(self.scene.load(path))
    }

    pub async fn load_async<B: AsRef<Path>>(&self, path: B) -> Result<LoadResult, triangle_scene::SceneError> {
        self.scene.load(path).await
    }

    pub fn add_material<B: Into<[f32; 3]>>(
        &self,
        color: B,
        roughness: f32,
        specular: B,
        transmission: f32,
    ) -> Result<u32, triangle_scene::SceneError> {
        if let Ok(mut materials) = self.scene.materials.lock() {
            Ok(materials.add(color, roughness, specular, transmission) as u32)
        } else {
            Err(triangle_scene::SceneError::LockError)
        }
    }

    pub fn add_object<B: ToMesh>(
        &self,
        object: B,
    ) -> Result<ObjectRef, triangle_scene::SceneError> {
        let m = object.into_mesh();
        match self.scene.add_object(m) {
            Ok(id) => Ok(ObjectRef::Static(id as u32)),
            Err(e) => Err(e)
        }
    }

    pub fn create_instance(&self, object: ObjectRef) -> Result<usize, triangle_scene::SceneError> {
        self.scene.add_instance(object)
    }

    /// Will return a reference to the point light if the scene is not locked
    pub fn add_point_light<B: Into<[f32; 3]>>(&self, position: B, radiance: B) -> Option<LightRef> {
        if let Ok(mut lights) = self.scene.lights_lock() {
            let position: Vec3 = Vec3::from(position.into());
            let radiance: Vec3 = Vec3::from(radiance.into());

            let light = PointLight::new(position.into(), radiance.into());
            lights.point_lights.push(light.clone());

            let light = LightRef::new(
                lights.point_lights.len() - 1,
                SceneLight::Point(light),
                self.scene.get_lights(),
            );

            Some(light)
        } else {
            None
        }
    }

    /// Will return a reference to the spot light if the scene is not locked
    pub fn add_spot_light<B: Into<[f32; 3]>>(
        &self,
        position: B,
        direction: B,
        radiance: B,
        inner_degrees: f32,
        outer_degrees: f32,
    ) -> Option<LightRef> {
        if let Ok(mut lights) = self.scene.lights_lock() {
            let position = Vec3::from(position.into());
            let direction = Vec3::from(direction.into());
            let radiance = Vec3::from(radiance.into());

            let light = SpotLight::new(
                position.into(),
                direction.into(),
                inner_degrees,
                outer_degrees,
                radiance.into(),
            );

            lights.spot_lights.push(light.clone());

            let light = LightRef::new(
                lights.spot_lights.len() - 1,
                SceneLight::Spot(light),
                self.scene.get_lights(),
            );

            Some(light)
        } else {
            None
        }
    }

    /// Will return a reference to the directional light if the scene is not locked
    pub fn add_directional_light<B: Into<[f32; 3]>>(
        &self,
        direction: B,
        radiance: B,
    ) -> Option<LightRef> {
        if let Ok(mut lights) = self.scene.lights_lock() {
            let light =
                DirectionalLight::new(Vec3A::from(direction.into()), Vec3A::from(radiance.into()));
            lights.directional_lights.push(light.clone());

            let light = LightRef::new(
                lights.directional_lights.len() - 1,
                SceneLight::Directional(light),
                self.scene.get_lights(),
            );

            Some(light)
        } else {
            None
        }
    }

    pub fn set_animation_time(&self, time: f32) {
        if let (Ok(mut nodes), Ok(mut animations)) = (
            self.scene.objects.nodes.lock(),
            self.scene.objects.animations.lock(),
        ) {
            animations.iter_mut().for_each(|(_, anim)| {
                anim.set_time(time, &mut nodes);
            });
        }
    }

    pub fn synchronize(&self) {
        if let Ok(mut renderer) = self.renderer.try_lock() {
            let mut changed = false;
            let mut update_lights = false;
            let mut found_light = false;

            if let (Ok(mut nodes), Ok(mut skins), Ok(mut instances)) = (
                self.scene.objects.nodes.lock(),
                self.scene.objects.skins.lock(),
                self.scene.objects.instances.lock(),
            ) {
                if nodes.any_changed() {
                    nodes.update(&mut instances, &mut skins);
                }

                skins
                    .iter_changed()
                    .for_each(|(id, skin)| renderer.set_skin(id, skin));
                skins.reset_changed();
                nodes.reset_changed();
            }

            if let (Ok(mut meshes), Ok(mut anim_meshes), Ok(mut instances)) = (
                self.scene.objects.meshes.lock(),
                self.scene.objects.animated_meshes.lock(),
                self.scene.objects.instances.lock(),
            ) {
                meshes.iter_changed().for_each(|(i, m)| {
                    renderer.set_mesh(i, m);
                    changed = true;
                });

                anim_meshes.iter_changed().for_each(|(i, m)| {
                    renderer.set_animated_mesh(i, m);
                    changed = true;
                });

                if let Ok(materials) = self.scene.materials_lock() {
                    let light_flags = materials.light_flags();
                    instances.iter_changed_mut().for_each(|(i, instance)| {
                        instance.update_transform();
                        renderer.set_instance(i, instance);
                        changed = true;

                        if found_light {
                            return;
                        }

                        match instance.object_id {
                            ObjectRef::None => {
                                return;
                            }
                            ObjectRef::Static(object_id) => {
                                let object_id = object_id as usize;
                                for j in 0..meshes[object_id].meshes.len() {
                                    match light_flags
                                        .get(meshes[object_id].meshes[j].mat_id as usize)
                                    {
                                        None => {}
                                        Some(flag) => {
                                            if *flag {
                                                found_light = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            ObjectRef::Animated(object_id) => {
                                let object_id = object_id as usize;
                                for j in 0..anim_meshes[object_id].meshes.len() {
                                    match light_flags
                                        .get(anim_meshes[object_id].meshes[j].mat_id as usize)
                                    {
                                        None => {}
                                        Some(flag) => {
                                            if *flag {
                                                found_light = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    });
                }

                meshes.reset_changed();
                anim_meshes.reset_changed();
                instances.reset_changed();
            }

            update_lights |= found_light;

            if let Ok(mut materials) = self.scene.materials_lock() {
                let mut mat_changed = false;
                if materials.textures_changed() {
                    renderer.set_textures(materials.textures_slice());
                    changed = true;
                    mat_changed = true;
                }

                if materials.changed() {
                    let device_materials = materials.into_device_materials();
                    renderer.set_materials(materials.as_slice(), device_materials.as_slice());
                    changed = true;
                    mat_changed = true;
                }

                materials.reset_changed();
                update_lights = update_lights || mat_changed;
            };

            if update_lights {
                self.scene.update_lights();
            }

            if let Ok(mut lights) = self.scene.lights_lock() {
                unsafe {
                    if lights.point_lights.any_changed() {
                        renderer.set_point_lights(&lights.point_lights.changed(), lights.point_lights.as_slice());
                        lights.point_lights.reset_changed();
                        changed = true;
                    }

                    if lights.spot_lights.any_changed() {
                        renderer.set_spot_lights(&lights.spot_lights.changed(), lights.spot_lights.as_slice());
                        lights.spot_lights.reset_changed();
                        changed = true;
                    }

                    if lights.area_lights.any_changed() {
                        renderer.set_area_lights(&lights.area_lights.changed(), lights.area_lights.as_slice());
                        lights.area_lights.reset_changed();
                        changed = true;
                    }

                    if lights.directional_lights.any_changed() {
                        renderer.set_directional_lights(
                            &lights.directional_lights.changed(),
                            lights.directional_lights.as_slice(),
                        );
                        lights.directional_lights.reset_changed();
                        changed = true;
                    }
                }
            }

            if changed {
                renderer.synchronize();
            }
        }
    }

    pub fn get_settings(&self) -> Result<Vec<Setting>, ()> {
        if let Ok(renderer) = self.renderer.try_lock() {
            Ok(renderer.get_settings())
        } else {
            Err(())
        }
    }

    pub fn set_setting(&self, setting: Setting) -> Result<(), ()> {
        if let Ok(mut renderer) = self.renderer.try_lock() {
            renderer.set_setting(setting);
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn set_skybox<B: AsRef<Path>>(&self, path: B) -> Result<(), ()> {
        if let Ok(texture) = Texture::load(path, Flip::FlipV) {
            if let Ok(mut renderer) = self.renderer.try_lock() {
                renderer.set_skybox(texture);
                return Ok(());
            }
        }

        Err(())
    }
}
