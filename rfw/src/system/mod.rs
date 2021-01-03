use rfw_backend::{Backend, RenderMode, Setting};
use rfw_math::*;
use rfw_scene::{
    r2d::Instance2D, r2d::Mesh2D, Camera, DirectionalLight, Instance3D, LoadResult, NodeGraph,
    ObjectRef, PointLight, Scene, SceneError, SceneLights, SpotLight, ToMesh,
};
use rfw_scene::{Flip, Material, Texture};
use rfw_utils::collections::{FlaggedIterator, FlaggedIteratorMut};
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct PointLightRef(u32);
pub struct SpotLightRef(u32);
pub struct DirectionalLightRef(u32);

pub struct RenderSystem<T: Sized + Backend> {
    pub scene: Scene,
    renderer: Arc<Mutex<Box<T>>>,
}

impl<T: Sized + Backend> RenderSystem<T> {
    pub fn new<B: rfw_backend::HasRawWindowHandle>(
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

    #[cfg(feature = "serde")]
    pub fn from_scene<B: rfw_backend::HasRawWindowHandle, P: AsRef<Path>>(
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

    pub fn iter_instances<C>(&self) -> FlaggedIterator<'_, Instance3D> {
        self.scene.objects.instances.iter()
    }

    pub fn iter_instances_mut(&mut self) -> FlaggedIteratorMut<'_, Instance3D> {
        self.scene.objects.instances.iter_mut()
    }

    pub fn get_instance(&self, index: usize) -> Option<&Instance3D> {
        self.scene.objects.instances.get(index)
    }

    pub fn get_instance_mut(&mut self, index: usize) -> Option<&mut Instance3D> {
        self.scene.objects.instances.get_mut(index)
    }

    pub fn get_2d_instance(&self, index: usize) -> Option<&Instance2D> {
        self.scene.objects.d2_instances.get(index)
    }

    pub fn get_2d_instance_mut(&mut self, index: usize) -> Option<&mut Instance2D> {
        self.scene.objects.d2_instances.get_mut(index)
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
                    result.push(Some(m_id as u32));
                }
            }
        }

        result
    }

    pub fn resize<B: rfw_backend::HasRawWindowHandle>(
        &self,
        window: &B,
        window_size: (usize, usize),
        render_size: (usize, usize),
    ) {
        self.renderer
            .lock()
            .unwrap()
            .resize(window, window_size, render_size);
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
        self.scene
            .materials
            .add(color, roughness, specular, transmission) as u32
    }

    pub fn get_material<C>(&self, id: u32, cb: C)
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

    pub fn iter_materials(&self) -> FlaggedIterator<'_, Material> {
        self.scene.materials.iter()
    }

    pub fn iter_materials_mut(&mut self) -> FlaggedIteratorMut<'_, Material> {
        self.scene.materials.iter_mut()
    }

    pub fn iter_textures(&self) -> FlaggedIterator<'_, Texture> {
        self.scene.materials.tex_iter()
    }

    pub fn iter_textures_mut(&mut self) -> FlaggedIteratorMut<'_, Texture> {
        self.scene.materials.tex_iter_mut()
    }

    pub fn add_object<B: ToMesh>(&mut self, object: B) -> Result<ObjectRef, SceneError> {
        let m = object.into_mesh();
        match self.scene.add_3d_object(m) {
            Ok(id) => Ok(Some(id as u32)),
            Err(e) => Err(e),
        }
    }

    pub fn add_2d_object(&mut self, object: Mesh2D) -> Result<u32, SceneError> {
        match self.scene.add_2d_object(object) {
            Ok(id) => Ok(id as u32),
            Err(e) => Err(e),
        }
    }

    pub fn set_2d_object(&mut self, id: u32, object: Mesh2D) -> Result<(), SceneError> {
        if let Some(mesh) = self.scene.objects.d2_meshes.get_mut(id as usize) {
            *mesh = object;
            Ok(())
        } else {
            Err(SceneError::InvalidObjectIndex(id as usize))
        }
    }

    pub fn create_instance(&mut self, object: u32) -> Result<usize, SceneError> {
        self.scene.add_instance(object)
    }

    pub fn create_2d_instance(&mut self, object: u32) -> Result<usize, SceneError> {
        self.scene.add_2d_instance(object)
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
        graph.initialize(
            &mut self.scene.objects.instances,
            &mut self.scene.objects.skins,
        );
        self.scene.objects.graph.add_graph(graph)
    }

    pub fn remove_scene(&mut self, id: u32) -> Result<(), SceneError> {
        if self.scene.objects.graph.remove_graph(
            id,
            &mut self.scene.objects.instances,
            &mut self.scene.objects.skins,
        ) {
            Ok(())
        } else {
            Err(SceneError::InvalidSceneID(id))
        }
    }

    pub fn remove_instance(&mut self, id: u32) -> Result<(), SceneError> {
        self.scene.remove_instance(id as usize)
    }

    pub fn remove_2d_instance(&mut self, id: u32) -> Result<(), SceneError> {
        self.scene.remove_2d_instance(id as usize)
    }

    pub fn add_texture(&mut self, mut texture: Texture) -> Result<u32, SceneError> {
        texture.generate_mipmaps(Texture::MIP_LEVELS);
        Ok(self.scene.materials.push_texture(texture) as u32)
    }

    pub fn set_texture(&mut self, id: u32, mut texture: Texture) -> Result<(), SceneError> {
        let tex = self.scene.materials.get_texture_mut(id as usize);
        if tex.is_none() {
            return Err(SceneError::InvalidID(id));
        }

        texture.generate_mipmaps(Texture::MIP_LEVELS);
        *tex.unwrap() = texture;
        Ok(())
    }

    /// Will return a reference to the point light if the scene is not locked
    pub fn add_point_light<B: Into<[f32; 3]>>(
        &mut self,
        position: B,
        radiance: B,
    ) -> PointLightRef {
        let position: Vec3 = Vec3::from(position.into());
        let radiance: Vec3 = Vec3::from(radiance.into());

        let light = PointLight::new(position.into(), radiance.into());
        let id = self.scene.lights.point_lights.push(light.clone());

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
        SpotLightRef(id as u32)
    }

    /// Will return a reference to the directional light if the scene is not locked
    pub fn add_directional_light<B: Into<[f32; 3]>>(
        &mut self,
        direction: B,
        radiance: B,
    ) -> DirectionalLightRef {
        let light =
            DirectionalLight::new(Vec3::from(direction.into()), Vec3::from(radiance.into()));
        let id = self.scene.lights.directional_lights.push(light.clone());
        DirectionalLightRef(id as u32)
    }

    pub fn set_animation_time(&mut self, id: u32, time: f32) {
        self.scene.objects.graph.set_animation(id, time);
    }

    pub fn set_animations_time(&mut self, time: f32) {
        self.scene.objects.graph.set_animations(time);
    }

    pub fn synchronize(&mut self) {
        let mut renderer = self.renderer.try_lock().unwrap();

        let mut changed = false;
        let mut update_lights = false;
        let mut found_light = false;

        self.scene.objects.graph.synchronize(
            &mut self.scene.objects.instances,
            &mut self.scene.objects.skins,
        );

        if self.scene.objects.skins.any_changed() {
            renderer.set_skins(self.scene.objects.skins.iter_changed());
            self.scene.objects.skins.reset_changed();
        }

        if self.scene.objects.d2_meshes.any_changed() {
            renderer.set_2d_meshes(self.scene.objects.d2_meshes.iter_changed());
            self.scene.objects.d2_meshes.reset_changed();
        }

        if self.scene.objects.d2_instances.any_changed() {
            renderer.set_2d_instances(self.scene.objects.d2_instances.iter_changed());
            self.scene.objects.d2_instances.reset_changed();
        }

        if self.scene.objects.meshes.any_changed() {
            renderer.set_3d_meshes(self.scene.objects.meshes.iter_changed());
            changed = true;
            self.scene.objects.meshes.reset_changed();
        }

        let light_flags = self.scene.materials.light_flags();
        changed |= self.scene.objects.instances.any_changed();

        for (_, instance) in self.scene.objects.instances.iter_changed_mut() {
            instance.update_transform();
            if found_light {
                break;
            }

            if let Some(object_id) = instance.object_id {
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

        let deleted_meshes = self.scene.objects.meshes.take_erased();
        let instances = self.scene.objects.instances.take_erased();

        if !deleted_meshes.is_empty() {
            changed = true;
            renderer.unload_3d_meshes(deleted_meshes);
        }

        if !instances.is_empty() {
            changed = true;
            renderer.unload_instances(instances);
        }

        if changed {
            renderer.synchronize();
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
    #[cfg(feature = "serde")]
    pub fn save_scene<B: AsRef<Path>>(&self, path: B) -> Result<(), ()> {
        match self.scene.serialize(path) {
            Ok(_) => Ok(()),
            _ => Err(()),
        }
    }
}
