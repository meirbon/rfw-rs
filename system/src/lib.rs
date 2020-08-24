use glam::*;
pub use rfw_scene as scene;
use rfw_scene::graph::NodeGraph;
use rfw_scene::utils::{FlaggedIterator, FlaggedIteratorMut};
use rfw_scene::{
    raw_window_handle, Camera, DirectionalLight, Flip, Instance, LoadResult, ObjectRef, PointLight,
    RenderMode, Renderer, Scene, SceneError, SceneLights, Setting, SpotLight, Texture, ToMesh,
};
use scene::Material;
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Clone)]
pub enum SceneLight {
    Point(PointLight),
    Spot(SpotLight),
    Directional(DirectionalLight),
}

pub struct LightRef {
    id: usize,
    light: SceneLight,
    lights: Arc<RwLock<SceneLights>>,
}

impl LightRef {
    fn new(id: usize, light: SceneLight, lights: Arc<RwLock<SceneLights>>) -> Self {
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
        if let Ok(mut lights) = self.lights.write() {
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
    scene: Scene,
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
            scene: Scene::new(),
            renderer: Arc::new(Mutex::new(renderer)),
        })
    }

    pub fn from_scene<B: raw_window_handle::HasRawWindowHandle, P: AsRef<Path>>(
        scene: P,
        window: &B,
        width: usize,
        height: usize,
    ) -> Result<Self, Box<dyn Error>> {
        let renderer = T::init(window, width, height)?;

        Ok(Self {
            scene: Scene::deserialize(scene)?,
            renderer: Arc::new(Mutex::new(renderer)),
        })
    }

    pub fn iter_instances<C>(&self, cb: C)
    where
        C: FnOnce(FlaggedIterator<'_, Instance>),
    {
        let lock = self.scene.objects.instances.read().unwrap();
        cb(lock.iter());
    }

    pub fn iter_instances_mut<C>(&self, cb: C)
    where
        C: FnOnce(FlaggedIteratorMut<'_, Instance>),
    {
        let mut lock = self.scene.objects.instances.write().unwrap();
        cb(lock.iter_mut());
    }

    pub fn get_instance<C>(&self, index: usize, cb: C)
    where
        C: FnOnce(Option<&Instance>),
    {
        let lock = self.scene.objects.instances.read().unwrap();
        cb(lock.get(index))
    }

    pub fn get_instance_mut<C>(&self, index: usize, cb: C)
    where
        C: FnOnce(Option<&mut Instance>),
    {
        let mut lock = self.scene.objects.instances.write().unwrap();
        cb(lock.get_mut(index))
    }

    pub fn get_lights<C>(&self, cb: C)
    where
        C: FnOnce(&SceneLights),
    {
        let lock = self.scene.lights.read().unwrap();
        cb(&lock)
    }

    pub fn get_lights_mut<C>(&self, cb: C)
    where
        C: FnOnce(&mut SceneLights),
    {
        let mut lock = self.scene.lights.write().unwrap();
        cb(&mut lock)
    }

    pub fn find_mesh_by_name(&self, name: String) -> Vec<ObjectRef> {
        let mut result = Vec::new();
        if let (Ok(meshes), Ok(anim_meshes)) = (
            self.scene.objects.meshes.read(),
            self.scene.objects.animated_meshes.read(),
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

    pub fn load<B: AsRef<Path>>(&self, path: B) -> Result<LoadResult, SceneError> {
        futures::executor::block_on(self.scene.load(path))
    }

    pub async fn load_async<B: AsRef<Path>>(&self, path: B) -> Result<LoadResult, SceneError> {
        self.scene.load(path).await
    }

    pub fn add_material<B: Into<[f32; 3]>>(
        &self,
        color: B,
        roughness: f32,
        specular: B,
        transmission: f32,
    ) -> Result<u32, SceneError> {
        if let Ok(mut materials) = self.scene.materials.write() {
            Ok(materials.add(color, roughness, specular, transmission) as u32)
        } else {
            Err(SceneError::LockError)
        }
    }

    pub fn get_material<C>(&self, id: u32, cb: C)
    where
        C: Fn(Option<&Material>),
    {
        let materials = self.scene.materials.read().unwrap();
        cb(materials.get(id as usize));
    }

    pub fn get_material_mut<C>(&self, id: u32, cb: C)
    where
        C: Fn(Option<&mut Material>),
    {
        let mut materials = self.scene.materials.write().unwrap();
        materials.get_mut(id as usize, cb);
    }

    pub fn add_object<B: ToMesh>(&self, object: B) -> Result<ObjectRef, SceneError> {
        let m = object.into_mesh();
        match self.scene.add_object(m) {
            Ok(id) => Ok(ObjectRef::Static(id as u32)),
            Err(e) => Err(e),
        }
    }

    pub fn create_instance(&self, object: ObjectRef) -> Result<usize, SceneError> {
        self.scene.add_instance(object)
    }

    pub fn add_scene(&self, mut graph: NodeGraph) -> Result<u32, SceneError> {
        // TODO: This should be part of the scene crate API
        if let Ok(mut g) = self.scene.objects.graph.write() {
            graph.initialize(&self.scene.objects.instances);
            let id = g.add_graph(graph);
            Ok(id)
        } else {
            Err(SceneError::LockError)
        }
    }

    pub fn remove_scene(&self, id: u32) -> Result<(), SceneError> {
        // TODO: This should be part of the scene crate API
        if let Ok(mut g) = self.scene.objects.graph.write() {
            if g.remove_graph(id, &self.scene.objects.instances) {
                Ok(())
            } else {
                Err(SceneError::InvalidSceneID(id))
            }
        } else {
            Err(SceneError::LockError)
        }
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
                self.scene.lights.clone(),
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
                self.scene.lights.clone(),
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
    ) -> Result<LightRef, SceneError> {
        if let Ok(mut lights) = self.scene.lights_lock() {
            let light =
                DirectionalLight::new(Vec3A::from(direction.into()), Vec3A::from(radiance.into()));
            lights.directional_lights.push(light.clone());

            let light = LightRef::new(
                lights.directional_lights.len() - 1,
                SceneLight::Directional(light),
                self.scene.lights.clone(),
            );

            Ok(light)
        } else {
            Err(SceneError::LockError)
        }
    }

    pub fn set_animation_time(&self, time: f32) {
        // TODO: Add a function to do this on a graph by graph basis
        if let Ok(mut graph) = self.scene.objects.graph.write() {
            graph.set_animations(time);
        }
    }

    pub fn synchronize(&self) {
        if let Ok(mut renderer) = self.renderer.try_lock() {
            let mut changed = false;
            let mut update_lights = false;
            let mut found_light = false;

            if let Ok(mut graph) = self.scene.objects.graph.write() {
                graph.synchronize(&self.scene.objects.instances, &self.scene.objects.skins);
            }

            if let Ok(mut skins) = self.scene.objects.skins.write() {
                if skins.any_changed() {
                    renderer.set_skins(skins.iter_changed());
                    skins.reset_changed();
                }
            }

            if let (Ok(mut meshes), Ok(mut anim_meshes), Ok(mut instances)) = (
                self.scene.objects.meshes.write(),
                self.scene.objects.animated_meshes.write(),
                self.scene.objects.instances.write(),
            ) {
                if meshes.any_changed() {
                    renderer.set_meshes(meshes.iter_changed());
                    changed = true;
                    meshes.reset_changed();
                }

                if anim_meshes.any_changed() {
                    renderer.set_animated_meshes(anim_meshes.iter_changed());
                    changed = true;
                    anim_meshes.reset_changed();
                }

                if let Ok(materials) = self.scene.materials_lock() {
                    let light_flags = materials.light_flags();
                    changed |= instances.any_changed();

                    for (_, instance) in instances.iter_changed_mut() {
                        instance.update_transform();
                        if found_light {
                            break;
                        }

                        match instance.object_id {
                            ObjectRef::None => {
                                break;
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
                    }
                }

                if instances.any_changed() {
                    renderer.set_instances(instances.iter_changed());
                    changed = true;
                    instances.reset_changed();
                }
            }

            update_lights |= found_light;

            if let Ok(mut materials) = self.scene.materials_lock() {
                let mut mat_changed = false;
                if materials.textures_changed() {
                    renderer.set_textures(materials.iter_changed_textures());
                    changed = true;
                    mat_changed = true;
                }

                if materials.changed() {
                    renderer.set_materials(materials.get_device_materials());
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
                if lights.point_lights.any_changed() {
                    renderer.set_point_lights(lights.point_lights.iter_changed());
                    lights.point_lights.reset_changed();
                    changed = true;
                }

                if lights.spot_lights.any_changed() {
                    renderer.set_spot_lights(lights.spot_lights.iter_changed());
                    lights.spot_lights.reset_changed();
                    changed = true;
                }

                if lights.area_lights.any_changed() {
                    renderer.set_area_lights(lights.area_lights.iter_changed());
                    changed = true;
                }

                if lights.directional_lights.any_changed() {
                    renderer.set_directional_lights(lights.directional_lights.iter_changed());
                    lights.directional_lights.reset_changed();
                    changed = true;
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

    pub fn save_scene<B: AsRef<Path>>(&self, path: B) -> Result<(), ()> {
        match self.scene.serialize(path) {
            Ok(_) => Ok(()),
            _ => Err(()),
        }
    }
}
