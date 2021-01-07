use rfw_backend::{
    Backend, DataFormat, DirectionalLight, InstancesData2D, InstancesData3D, MeshData2D,
    MeshData3D, PointLight, RenderMode, SkinData, SpotLight, TextureData,
};
use rfw_math::*;
use rfw_scene::{
    r2d::Mesh2D, Camera, InstanceHandle2D, InstanceHandle3D, InstanceIterator2D,
    InstanceIterator3D, LoadResult, NodeGraph, ObjectRef, Scene, SceneError, SceneLights, ToMesh,
};
use rfw_scene::{Flip, Material, Texture};
use rfw_utils::{
    collections::{FlaggedIterator, FlaggedIteratorMut},
    BytesConversion,
};
use std::error::Error;
use std::path::Path;

pub struct PointLightRef(u32);
pub struct SpotLightRef(u32);
pub struct DirectionalLightRef(u32);

pub struct RenderSystem<T: Sized + Backend> {
    pub scene: Scene,
    width: u32,
    height: u32,
    scale_factor: f64,
    renderer: Box<T>,
}

impl<T: Sized + Backend> RenderSystem<T> {
    pub fn new<B: rfw_backend::HasRawWindowHandle>(
        window: &B,
        window_size: (u32, u32),
        scale_factor: Option<f64>,
    ) -> Result<Self, Box<dyn Error>> {
        let renderer = T::init(window, window_size, scale_factor.unwrap_or(1.0))?;

        Ok(Self {
            scene: Scene::new(),
            width: window_size.0,
            height: window_size.1,
            scale_factor: scale_factor.unwrap_or(1.0),
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

        Ok(Self {
            scene: Scene::deserialize(scene)?,
            width: window_size.0,
            height: window_size.1,
            scale_factor: scale_factor.unwrap_or(1.0),
            renderer,
        })
    }

    pub fn iter_3d_instances<C>(&self) -> InstanceIterator3D {
        self.scene.instances_3d.iter()
    }

    pub fn iter_2d_instances<C>(&self) -> InstanceIterator2D {
        self.scene.instances_2d.iter()
    }

    pub fn get_3d_instance(&self, index: usize) -> Option<InstanceHandle3D> {
        self.scene.instances_3d.get(index)
    }

    pub fn get_2d_instance(&self, index: usize) -> Option<InstanceHandle2D> {
        self.scene.instances_2d.get(index)
    }

    pub fn get_lights(&self) -> &SceneLights {
        &self.scene.lights
    }

    pub fn get_lights_mut(&mut self) -> &mut SceneLights {
        &mut self.scene.lights
    }

    pub fn find_mesh_by_name(&self, name: String) -> Vec<ObjectRef> {
        let mut result = Vec::new();
        for m_id in 0..self.scene.objects.meshes_3d.len() {
            if let Some(m) = self.scene.objects.meshes_3d.get(m_id) {
                if m.name == name {
                    result.push(Some(m_id as u32));
                }
            }
        }

        result
    }

    pub fn resize<B: rfw_backend::HasRawWindowHandle>(
        &mut self,
        window: &B,
        window_size: (u32, u32),
        scale_factor: Option<f64>,
    ) {
        let scale_factor = scale_factor.unwrap_or(self.scale_factor);
        self.renderer.resize(window, window_size, scale_factor);
        self.width = window_size.0;
        self.height = window_size.1;
        self.scale_factor = scale_factor;
    }

    pub fn render(&mut self, camera_id: usize, mode: RenderMode) -> Result<(), SceneError> {
        if let Some(camera) = self.scene.cameras.get(camera_id as usize) {
            let view = camera.get_view(self.width, self.height);
            self.renderer.render(view, mode);
            Ok(())
        } else {
            Err(SceneError::InvalidCameraID(camera_id))
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

    pub fn add_object<B: ToMesh>(&mut self, object: B) -> Result<usize, SceneError> {
        let m = object.into_mesh();
        self.scene.add_3d_object(m)
    }

    pub fn add_2d_object(&mut self, object: Mesh2D) -> Result<usize, SceneError> {
        match self.scene.add_2d_object(object) {
            Ok(id) => Ok(id),
            Err(e) => Err(e),
        }
    }

    pub fn set_2d_object(&mut self, id: usize, object: Mesh2D) -> Result<(), SceneError> {
        if let Some(mesh) = self.scene.objects.meshes_2d.get_mut(id as usize) {
            *mesh = object;
            Ok(())
        } else {
            Err(SceneError::InvalidObjectIndex(id as usize))
        }
    }

    pub fn create_3d_instance(&mut self, object: usize) -> Result<InstanceHandle3D, SceneError> {
        self.scene.add_instance(object)
    }

    pub fn create_2d_instance(&mut self, object: usize) -> Result<InstanceHandle2D, SceneError> {
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

    pub fn add_scene(&mut self, mut graph: NodeGraph) -> usize {
        graph.initialize(&mut self.scene.instances_3d, &mut self.scene.objects.skins);
        self.scene.objects.graph.add_graph(graph)
    }

    pub fn remove_scene(&mut self, id: usize) -> Result<(), SceneError> {
        if self.scene.objects.graph.remove_graph(
            id,
            &mut self.scene.instances_3d,
            &mut self.scene.objects.skins,
        ) {
            Ok(())
        } else {
            Err(SceneError::InvalidSceneID(id))
        }
    }

    pub fn remove_3d_instance(&mut self, id: usize) {
        self.scene.remove_3d_instance(id);
    }

    pub fn remove_2d_instance(&mut self, id: usize) {
        self.scene.remove_2d_instance(id);
    }

    pub fn add_texture(&mut self, mut texture: Texture) -> usize {
        texture.generate_mipmaps(Texture::MIP_LEVELS);
        self.scene.materials.push_texture(texture)
    }

    pub fn set_texture(&mut self, id: usize, mut texture: Texture) -> Result<(), SceneError> {
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

    pub fn set_animation_time(&mut self, id: usize, time: f32) {
        self.scene.objects.graph.set_animation(id, time);
    }

    pub fn set_animations_time(&mut self, time: f32) {
        self.scene.objects.graph.set_animations(time);
    }

    pub fn synchronize(&mut self) {
        let mut changed = false;
        let mut update_lights = false;
        let mut found_light = false;

        self.scene
            .objects
            .graph
            .synchronize(&mut self.scene.instances_3d, &mut self.scene.objects.skins);

        if self.scene.objects.skins.any_changed() {
            let skins: Vec<SkinData> = self
                .scene
                .objects
                .skins
                .iter()
                .map(|(_, s)| SkinData {
                    name: s.name.as_str(),
                    inverse_bind_matrices: s.inverse_bind_matrices.as_slice(),
                    joint_matrices: s.joint_matrices.as_slice(),
                })
                .collect();
            self.renderer
                .set_skins(skins.as_slice(), self.scene.objects.skins.changed());
            self.scene.objects.skins.reset_changed();
        }

        if self.scene.objects.meshes_2d.any_changed() {
            for (i, m) in self.scene.objects.meshes_2d.iter_changed() {
                self.renderer.set_2d_mesh(
                    i,
                    MeshData2D {
                        vertices: m.vertices.as_slice(),
                        tex_id: m.tex_id,
                    },
                );
            }
            self.scene.objects.meshes_2d.reset_changed();
        }

        if self.scene.instances_2d.any_changed() {
            self.renderer.set_2d_instances(InstancesData2D {
                matrices: self.scene.instances_2d.matrices(),
                mesh_ids: self.scene.instances_2d.mesh_ids(),
            });
            self.scene.instances_2d.reset_changed();
        }

        if self.scene.objects.meshes_3d.any_changed() {
            for (i, m) in self.scene.objects.meshes_3d.iter_changed() {
                self.renderer.set_3d_mesh(
                    i,
                    MeshData3D {
                        name: m.name.as_str(),
                        bounds: m.bounds,
                        vertices: m.vertices.as_slice(),
                        triangles: m.triangles.as_slice(),
                        ranges: m.ranges.as_slice(),
                        skin_data: m.skin_data.as_slice(),
                    },
                );
            }
            changed = true;
            self.scene.objects.meshes_3d.reset_changed();
        }

        let light_flags = self.scene.materials.light_flags();
        changed |= self.scene.instances_3d.any_changed();

        for instance in self.scene.instances_3d.iter() {
            if found_light {
                break;
            }

            if let Some(mesh_id) = instance.get_mesh_id().as_index() {
                for j in 0..self.scene.objects.meshes_3d[mesh_id].ranges.len() {
                    match light_flags
                        .get(self.scene.objects.meshes_3d[mesh_id].ranges[j].mat_id as usize)
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

        if self.scene.instances_3d.any_changed() {
            self.renderer.set_3d_instances(InstancesData3D {
                matrices: self.scene.instances_3d.matrices(),
                mesh_ids: self.scene.instances_3d.mesh_ids(),
                skin_ids: self.scene.instances_3d.skin_ids(),
            });
            changed = true;
            self.scene.instances_3d.reset_changed();
        }

        update_lights |= found_light;

        let mut mat_changed = false;
        if self.scene.materials.textures_changed() {
            let textures = self.scene.materials.get_textures();
            let tex_data: Vec<TextureData> = textures
                .iter()
                .map(|t| TextureData {
                    width: t.width,
                    height: t.height,
                    mip_levels: t.mip_levels,
                    bytes: t.data.as_bytes(),
                    format: DataFormat::BGRA8,
                })
                .collect();

            self.renderer.set_textures(
                tex_data.as_slice(),
                self.scene.materials.get_textures_changed(),
            );
            changed = true;
            mat_changed = true;
        }

        if self.scene.materials.changed() {
            self.scene.materials.update_device_materials();
            self.renderer.set_materials(
                self.scene.materials.get_device_materials(),
                self.scene.materials.get_materials_changed(),
            );
            changed = true;
            mat_changed = true;
        }

        self.scene.materials.reset_changed();
        update_lights = update_lights || mat_changed;

        if update_lights {
            self.scene.update_lights();
        }

        if self.scene.lights.point_lights.any_changed() {
            self.renderer.set_point_lights(
                self.scene.lights.point_lights.as_slice(),
                self.scene.lights.point_lights.changed(),
            );
            self.scene.lights.point_lights.reset_changed();
            changed = true;
        }

        if self.scene.lights.spot_lights.any_changed() {
            self.renderer.set_spot_lights(
                self.scene.lights.spot_lights.as_slice(),
                self.scene.lights.spot_lights.changed(),
            );
            self.scene.lights.spot_lights.reset_changed();
            changed = true;
        }

        if self.scene.lights.area_lights.any_changed() {
            self.renderer.set_area_lights(
                self.scene.lights.area_lights.as_slice(),
                self.scene.lights.area_lights.changed(),
            );
            changed = true;
        }

        if self.scene.lights.directional_lights.any_changed() {
            self.renderer.set_directional_lights(
                self.scene.lights.directional_lights.as_slice(),
                self.scene.lights.directional_lights.changed(),
            );
            self.scene.lights.directional_lights.reset_changed();
            changed = true;
        }

        let deleted_meshes = self.scene.objects.meshes_3d.take_erased();
        let deleted_instances = self.scene.instances_3d.take_removed();

        if !deleted_meshes.is_empty() {
            changed = true;
            self.renderer.unload_3d_meshes(deleted_meshes);
        }

        if !deleted_instances.is_empty() {
            changed = true;
            self.renderer.unload_3d_instances(deleted_instances);
        }

        if changed {
            self.renderer.synchronize();
        }
    }

    pub fn get_settings(&mut self) -> &mut T::Settings {
        self.renderer.settings()
    }

    pub fn set_skybox<B: AsRef<Path>>(&mut self, path: B) -> Result<(), ()> {
        if let Ok(texture) = Texture::load(path, Flip::FlipV) {
            self.renderer.set_skybox(TextureData {
                width: texture.width,
                height: texture.height,
                mip_levels: texture.mip_levels,
                bytes: texture.data.as_bytes(),
                format: rfw_backend::DataFormat::BGRA8,
            });
            Ok(())
        } else {
            Err(())
        }
    }

    #[cfg(feature = "serde")]
    pub fn save_scene<B: AsRef<Path>>(&self, path: B) -> Result<(), ()> {
        match self.scene.serialize(path) {
            Ok(_) => Ok(()),
            _ => Err(()),
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn render_width(&self) -> u32 {
        (self.width as f64 * self.scale_factor) as u32
    }

    pub fn render_height(&self) -> u32 {
        (self.height as f64 * self.scale_factor) as u32
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }
}
