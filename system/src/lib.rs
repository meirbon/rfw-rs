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
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum SceneLight {
    Point(PointLight),
    Spot(SpotLight),
    Directional(DirectionalLight),
}

pub type LightRef = (usize, SceneLight);

// impl LightRef {
//     fn new(id: usize, light: SceneLight, lights: Arc<RwLock<SceneLights>>) -> Self {
//         Self { id, light, lights }
//     }
//
//     pub fn get(&self) -> &SceneLight {
//         &self.light
//     }
//
//     pub fn get_mut(&mut self) -> &mut SceneLight {
//         &mut self.light
//     }
//
//     pub fn translate_x(&mut self, offset: f32) {
//         let translation = Vec3::new(offset, 0.0, 0.0);
//         match &mut self.light {
//             SceneLight::Spot(l) => {
//                 let position: Vec3 = Vec3::from(l.position) + translation;
//                 l.position = position.into();
//             }
//             _ => {}
//         }
//     }
//
//     pub fn translate_y(&mut self, offset: f32) {
//         let translation = Vec3::new(0.0, offset, 0.0);
//         match &mut self.light {
//             SceneLight::Spot(l) => {
//                 let position: Vec3 = Vec3::from(l.position) + translation;
//                 l.position = position.into();
//             }
//             _ => {}
//         }
//     }
//
//     pub fn translate_z(&mut self, offset: f32) {
//         let translation = Vec3::new(0.0, 0.0, offset);
//         match &mut self.light {
//             SceneLight::Spot(l) => {
//                 let position: Vec3 = Vec3::from(l.position) + translation;
//                 l.position = position.into();
//             }
//             _ => {}
//         }
//     }
//
//     pub fn rotate_x(&mut self, degrees: f32) {
//         let rotation = Mat4::from_rotation_x(degrees.to_radians());
//         match &mut self.light {
//             SceneLight::Spot(l) => {
//                 let direction: Vec3 = l.direction.into();
//                 let direction = rotation * direction.extend(0.0);
//                 l.direction = direction.truncate().into();
//             }
//             SceneLight::Directional(l) => {
//                 let direction: Vec3 = l.direction.into();
//                 let direction = rotation * direction.extend(0.0);
//                 l.direction = direction.truncate().into();
//             }
//             _ => {}
//         }
//     }
//
//     pub fn rotate_y(&mut self, degrees: f32) {
//         let rotation = Mat4::from_rotation_y(degrees.to_radians());
//         match &mut self.light {
//             SceneLight::Spot(l) => {
//                 let direction: Vec3 = l.direction.into();
//                 let direction = rotation * direction.extend(0.0);
//                 l.direction = direction.truncate().into();
//             }
//             SceneLight::Directional(l) => {
//                 let direction: Vec3 = l.direction.into();
//                 let direction = rotation * direction.extend(0.0);
//                 l.direction = direction.truncate().into();
//             }
//             _ => {}
//         }
//     }
//
//     pub fn rotate_z(&mut self, degrees: f32) {
//         let rotation = Mat4::from_rotation_z(degrees.to_radians());
//         match &mut self.light {
//             SceneLight::Spot(l) => {
//                 let direction: Vec3 = l.direction.into();
//                 let direction = rotation * direction.extend(0.0);
//                 l.direction = direction.truncate().into();
//             }
//             SceneLight::Directional(l) => {
//                 let direction: Vec3 = l.direction.into();
//                 let direction = rotation * direction.extend(0.0);
//                 l.direction = direction.truncate().into();
//             }
//             _ => {}
//         }
//     }
//
//     pub fn synchronize(&self) -> Result<(), ()> {
//         if let Ok(mut lights) = self.lights.write() {
//             match &self.light {
//                 SceneLight::Point(l) => {
//                     lights.point_lights[self.id] = l.clone();
//                 }
//                 SceneLight::Spot(l) => {
//                     lights.spot_lights[self.id] = l.clone();
//                 }
//                 SceneLight::Directional(l) => {
//                     lights.directional_lights[self.id] = l.clone();
//                 }
//             }
//
//             Ok(())
//         } else {
//             Err(())
//         }
//     }
// }

pub struct RenderSystem<T: Sized + Renderer> {
    pub scene: Scene,
    renderer: Arc<Mutex<Box<T>>>,
}

impl<T: Sized + Renderer> RenderSystem<T> {
    pub fn new<B: raw_window_handle::HasRawWindowHandle>(
        window: &B,
        window_size: (usize, usize),
        render_size: (usize, usize),
    ) -> Result<Self, Box<dyn Error>> {
        let renderer = T::init(window, window_size, render_size)?;

        Ok(Self {
            scene: Scene::new(),
            renderer: Arc::new(Mutex::new(renderer)),
        })
    }

    pub fn from_scene<B: raw_window_handle::HasRawWindowHandle, P: AsRef<Path>>(
        scene: P,
        window: &B,
        window_size: (usize, usize),
        render_size: (usize, usize),
    ) -> Result<Self, Box<dyn Error>> {
        let renderer = T::init(window, window_size, render_size)?;

        Ok(Self {
            scene: Scene::deserialize(scene)?,
            renderer: Arc::new(Mutex::new(renderer)),
        })
    }

    pub fn iter_instances<C>(&self) -> FlaggedIterator<'_, Instance> {
        self.scene.objects.instances.iter()
    }

    pub fn iter_instances_mut(&mut self) -> FlaggedIteratorMut<'_, Instance> {
        self.scene.objects.instances.iter_mut()
    }

    pub fn get_instance(&self, index: usize) -> Option<&Instance> {
        self.scene.objects.instances.get(index)
    }

    pub fn get_instance_mut(&mut self, index: usize) -> Option<&mut Instance> {
        self.scene.objects.instances.get_mut(index)
    }

    pub fn get_lights(&self) -> &SceneLights {
        &self.scene.lights
    }

    pub fn get_lights_mut(&mut self) -> &mut SceneLights {
        &mut self.scene.lights
    }

    pub fn find_mesh_by_name(&self, name: String) -> Vec<ObjectRef> {
        let mut result = Vec::new();
        for m_id in 0..self.scene.objects.meshes.len() {
            if let Some(m) = self.scene.objects.meshes.get(m_id) {
                if m.name == name {
                    result.push(ObjectRef::Static(m_id as u32));
                }
            }
        }

        for m_id in 0..self.scene.objects.animated_meshes.len() {
            if let Some(m) = self.scene.objects.animated_meshes.get(m_id) {
                if m.name == name {
                    result.push(ObjectRef::Animated(m_id as u32));
                }
            }
        }

        result
    }

    pub fn resize<B: raw_window_handle::HasRawWindowHandle>(
        &self,
        window: &B,
        window_size: (usize, usize),
        render_size: (usize, usize),
    ) {
        self.renderer.lock().unwrap().resize(window, window_size, render_size);
    }

    pub fn render(&self, camera_id: usize, mode: RenderMode) -> Result<(), SceneError> {
        let mut renderer = self.renderer.try_lock()?;
        if let Some(camera) = self.scene.cameras.get(camera_id as usize) {
            renderer.render(camera, mode);
            Ok(())
        } else {
            Err(SceneError::InvalidCameraID(camera_id as u32))
        }
    }

    pub fn load<B: AsRef<Path>>(&mut self, path: B) -> Result<LoadResult, SceneError> {
        self.scene.load(path)
    }

    pub fn add_material<B: Into<[f32; 3]>>(
        &mut self,
        color: B,
        roughness: f32,
        specular: B,
        transmission: f32,
    ) -> u32 {
        self.scene.materials.add(color, roughness, specular, transmission) as u32
    }

    pub fn get_material<C>(&mut self, id: u32, cb: C)
        where
            C: Fn(Option<&Material>),
    {
        cb(self.scene.materials.get(id as usize));
    }

    pub fn get_material_mut<C>(&mut self, id: u32, cb: C)
        where
            C: Fn(Option<&mut Material>),
    {
        self.scene.materials.get_mut(id as usize, cb);
    }

    pub fn add_object<B: ToMesh>(&mut self, object: B) -> Result<ObjectRef, SceneError> {
        let m = object.into_mesh();
        match self.scene.add_object(m) {
            Ok(id) => Ok(ObjectRef::Static(id as u32)),
            Err(e) => Err(e),
        }
    }

    pub fn create_instance(&mut self, object: ObjectRef) -> Result<usize, SceneError> {
        self.scene.add_instance(object)
    }

    pub fn create_camera(&mut self, width: u32, height: u32) -> usize {
        self.scene.add_camera(width, height)
    }

    pub fn get_camera(&self, id: usize) -> Option<&Camera> {
        self.scene.cameras.get(id)
    }

    pub fn get_camera_mut(&mut self, id: usize) -> Option<&mut Camera> {
        self.scene.cameras.get_mut(id)
    }

    pub fn add_scene(&mut self, mut graph: NodeGraph) -> u32 {
        // TODO: This should be part of the scene crate API
        graph.initialize(&mut self.scene.objects.instances, &mut self.scene.objects.skins);
        self.scene.objects.graph.add_graph(graph)
    }

    pub fn remove_scene(&mut self, id: u32) -> Result<(), SceneError> {
        // TODO: This should be part of the scene crate API
        if self.scene.objects.graph.remove_graph(id, &mut self.scene.objects.instances, &mut self.scene.objects.skins) {
            Ok(())
        } else {
            Err(SceneError::InvalidSceneID(id))
        }
    }

    /// Will return a reference to the point light if the scene is not locked
    pub fn add_point_light<B: Into<[f32; 3]>>(&mut self, position: B, radiance: B) -> LightRef {
        let position: Vec3 = Vec3::from(position.into());
        let radiance: Vec3 = Vec3::from(radiance.into());

        let light = PointLight::new(position.into(), radiance.into());
        let id = self.scene.lights.point_lights.push(light.clone());

        (id, SceneLight::Point(light))
    }

    /// Will return a reference to the spot light if the scene is not locked
    pub fn add_spot_light<B: Into<[f32; 3]>>(
        &mut self,
        position: B,
        direction: B,
        radiance: B,
        inner_degrees: f32,
        outer_degrees: f32,
    ) -> LightRef {
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

        let id = self.scene.lights.spot_lights.push(light.clone());

        (id, SceneLight::Spot(light))
    }

    /// Will return a reference to the directional light if the scene is not locked
    pub fn add_directional_light<B: Into<[f32; 3]>>(
        &mut self,
        direction: B,
        radiance: B,
    ) -> LightRef {
        let light =
            DirectionalLight::new(Vec3A::from(direction.into()), Vec3A::from(radiance.into()));
        let id = self.scene.lights.directional_lights.push(light.clone());
        (id, SceneLight::Directional(light))
    }

    pub fn set_animation_time(&mut self, id: u32, time: f32) {
        // TODO: Add a function to do this on a graph by graph basis
        self.scene.objects.graph.set_animation(id, time);
    }

    pub fn set_animations_time(&mut self, time: f32) {
        // TODO: Add a function to do this on a graph by graph basis
        self.scene.objects.graph.set_animations(time);
    }

    pub fn synchronize(&mut self) {
        if let Ok(mut renderer) = self.renderer.try_lock() {
            let mut changed = false;
            let mut update_lights = false;
            let mut found_light = false;

            self.scene.objects.graph.synchronize(&mut self.scene.objects.instances, &mut self.scene.objects.skins);


            if self.scene.objects.skins.any_changed() {
                renderer.set_skins(self.scene.objects.skins.iter_changed());
                self.scene.objects.skins.reset_changed();
            }

            if self.scene.objects.meshes.any_changed() {
                renderer.set_meshes(self.scene.objects.meshes.iter_changed());
                changed = true;
                self.scene.objects.meshes.reset_changed();
            }

            if self.scene.objects.animated_meshes.any_changed() {
                renderer.set_animated_meshes(self.scene.objects.animated_meshes.iter_changed());
                changed = true;
                self.scene.objects.animated_meshes.reset_changed();
            }

            let light_flags = self.scene.materials.light_flags();
            changed |= self.scene.objects.instances.any_changed();

            for (_, instance) in self.scene.objects.instances.iter_changed_mut() {
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
                        for j in 0..self.scene.objects.meshes[object_id].meshes.len() {
                            match light_flags
                                .get(self.scene.objects.meshes[object_id].meshes[j].mat_id as usize)
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
                        for j in 0..self.scene.objects.animated_meshes[object_id].meshes.len() {
                            match light_flags
                                .get(self.scene.objects.animated_meshes[object_id].meshes[j].mat_id as usize)
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

            if self.scene.objects.instances.any_changed() {
                renderer.set_instances(self.scene.objects.instances.iter_changed());
                changed = true;
                self.scene.objects.instances.reset_changed();
            }

            update_lights |= found_light;

            let mut mat_changed = false;
            if self.scene.materials.textures_changed() {
                renderer.set_textures(self.scene.materials.iter_changed_textures());
                changed = true;
                mat_changed = true;
            }

            if self.scene.materials.changed() {
                renderer.set_materials(self.scene.materials.get_device_materials());
                changed = true;
                mat_changed = true;
            }

            self.scene.materials.reset_changed();
            update_lights = update_lights || mat_changed;

            if update_lights {
                self.scene.update_lights();
            }

            if self.scene.lights.point_lights.any_changed() {
                renderer.set_point_lights(self.scene.lights.point_lights.iter_changed());
                self.scene.lights.point_lights.reset_changed();
                changed = true;
            }

            if self.scene.lights.spot_lights.any_changed() {
                renderer.set_spot_lights(self.scene.lights.spot_lights.iter_changed());
                self.scene.lights.spot_lights.reset_changed();
                changed = true;
            }

            if self.scene.lights.area_lights.any_changed() {
                renderer.set_area_lights(self.scene.lights.area_lights.iter_changed());
                changed = true;
            }

            if self.scene.lights.directional_lights.any_changed() {
                renderer.set_directional_lights(self.scene.lights.directional_lights.iter_changed());
                self.scene.lights.directional_lights.reset_changed();
                changed = true;
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
