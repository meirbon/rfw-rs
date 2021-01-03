use rfw_math::*;

pub type PrimID = i32;
pub type InstanceID = i32;

pub mod camera;
pub mod constants;
pub mod graph;
pub mod instances;
pub mod lights;
pub mod loaders;
pub mod material;
pub mod objects;
pub mod r2d;

pub mod utils;

pub use camera::*;
pub use graph::*;
pub use instance::*;
pub use instances::*;
// pub use intersector::*;
pub use l3d::prelude::{
    load::*, mat::Flip, mat::Material, mat::Texture, mat::TextureDescriptor, mat::TextureFormat,
    mat::TextureSource,
};
pub use lights::*;
pub use loaders::*;
pub use material::*;
pub use objects::mesh::*;
pub use objects::*;
pub use r2d::*;
pub use rtbvh as bvh;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
use std::{error::Error, ffi::OsString, fs::File, io::BufReader};

use crate::utils::Flags;
use rfw_utils::collections::{FlaggedIterator, FlaggedIteratorMut, TrackedStorage};
use rtbvh::AABB;
use std::collections::HashSet;
use std::sync::{PoisonError, TryLockError};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub enum SceneError {
    InvalidObjectRef,
    InvalidObjectIndex(usize),
    InvalidInstanceIndex(usize),
    InvalidSceneID(u32),
    InvalidID(u32),
    InvalidCameraID(u32),
    LoadError(PathBuf),
    LockError,
    UnknownError,
    NoFileExtension,
    NoFileLoader(String),
}

impl<Guard> From<TryLockError<Guard>> for SceneError {
    fn from(_: TryLockError<Guard>) -> Self {
        Self::LockError
    }
}

impl<Guard> From<PoisonError<Guard>> for SceneError {
    fn from(_: PoisonError<Guard>) -> Self {
        Self::LockError
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum SceneFlags {
    BuildBVHs = 0,
}

impl Into<u8> for SceneFlags {
    fn into(self) -> u8 {
        self as u8
    }
}

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Self::InvalidObjectRef => String::from("object reference was None"),
            Self::InvalidObjectIndex(idx) => format!("invalid object index {}", idx),
            Self::InvalidInstanceIndex(idx) => format!("invalid instances index {}", idx),
            Self::InvalidSceneID(id) => format!("invalid scene id {}", id),
            Self::InvalidID(id) => format!("invalid id {}", id),
            Self::InvalidCameraID(id) => format!("invalid camera id {}", id),
            Self::LoadError(path) => format!("could not load file: {}", path.display()),
            Self::LockError => String::from("could not acquire lock"),
            Self::UnknownError => String::new(),
            Self::NoFileExtension => String::from("file had no file extension"),
            Self::NoFileLoader(ext) => format!("no file loader available for {}", ext),
        };

        write!(f, "{}", string)
    }
}

impl std::error::Error for SceneError {}

#[derive(Debug, Clone)]
pub struct Objects {
    pub meshes: TrackedStorage<Mesh3D>,
    pub d2_meshes: TrackedStorage<Mesh2D>,
    pub graph: graph::SceneGraph,
    pub skins: TrackedStorage<graph::Skin>,
    // pub instances: TrackedStorage<Instance3D>,
    pub d2_instances: TrackedStorage<Instance2D>,
    pub o_to_i_mapping: HashMap<u32, HashSet<u32>>,
}

impl Default for Objects {
    fn default() -> Self {
        Self {
            meshes: TrackedStorage::new(),
            d2_meshes: TrackedStorage::new(),
            graph: graph::SceneGraph::new(),
            skins: TrackedStorage::new(),
            d2_instances: TrackedStorage::new(),
            o_to_i_mapping: HashMap::new(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct SceneLights {
    pub point_lights: TrackedStorage<PointLight>,
    pub spot_lights: TrackedStorage<SpotLight>,
    pub area_lights: TrackedStorage<AreaLight>,
    pub directional_lights: TrackedStorage<DirectionalLight>,
}

impl Default for SceneLights {
    fn default() -> Self {
        Self {
            point_lights: TrackedStorage::new(),
            spot_lights: TrackedStorage::new(),
            area_lights: TrackedStorage::new(),
            directional_lights: TrackedStorage::new(),
        }
    }
}

/// Scene optimized for triangles
/// Does not support objects other than Meshes, but does not require virtual calls because of this.
#[derive(Debug)]
pub struct Scene {
    loaders: HashMap<String, Box<dyn ObjectLoader>>,
    pub objects: Objects,
    pub lights: SceneLights,
    pub materials: MaterialList,
    pub settings: Flags,
    pub cameras: TrackedStorage<Camera>,
    pub instances: InstanceList,
}

impl Default for Scene {
    fn default() -> Self {
        let loaders = Self::create_loaders();

        Self {
            loaders,
            objects: Objects::default(),
            lights: SceneLights::default(),
            materials: MaterialList::new(),
            settings: Flags::new(),
            cameras: TrackedStorage::new(),
            instances: InstanceList::new(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
struct SerializableScene {
    pub instances: instances::List,
    pub meshes: TrackedStorage<Mesh3D>,
    pub d2_meshes: TrackedStorage<Mesh2D>,
    pub graph: graph::SceneGraph,
    pub skins: TrackedStorage<graph::Skin>,
    pub d2_instances: TrackedStorage<Instance2D>,
    pub o_to_i_mapping: HashMap<u32, HashSet<u32>>,
    pub lights: SceneLights,
    pub materials: MaterialList,
    pub settings: Flags,
}

impl From<&Scene> for SerializableScene {
    fn from(scene: &Scene) -> Self {
        Self {
            instances: scene.instances.clone_inner(),
            meshes: scene.objects.meshes.clone(),
            d2_meshes: scene.objects.d2_meshes.clone(),
            graph: scene.objects.graph.clone(),
            skins: scene.objects.skins.clone(),
            d2_instances: scene.objects.d2_instances.clone(),
            o_to_i_mapping: scene.objects.o_to_i_mapping.clone(),
            lights: scene.lights.clone(),
            materials: scene.materials.clone(),
            settings: scene.settings.clone(),
        }
    }
}

impl Into<Scene> for SerializableScene {
    fn into(self) -> Scene {
        Scene {
            loaders: Scene::create_loaders(),
            objects: Objects {
                meshes: self.meshes,
                d2_meshes: self.d2_meshes,
                graph: self.graph,
                skins: self.skins,
                d2_instances: self.d2_instances,
                o_to_i_mapping: self.o_to_i_mapping,
            },
            lights: self.lights,
            materials: self.materials,
            settings: self.settings,
            cameras: TrackedStorage::new(),
            instances: self.instances.into(),
        }
    }
}

#[allow(dead_code)]
impl Scene {
    const FF_EXTENSION: &'static str = ".scenev1";

    pub fn new() -> Self {
        Self {
            objects: Objects::default(),
            lights: SceneLights::default(),
            materials: MaterialList::new(),
            settings: Flags::default(),
            ..Default::default()
        }
    }

    pub fn get_scene(&self) -> Objects {
        self.objects.clone()
    }

    pub fn get_lights(&self) -> &SceneLights {
        &self.lights
    }

    pub fn get_materials(&self) -> &MaterialList {
        &self.materials
    }

    pub fn get_lights_mut(&mut self) -> &mut SceneLights {
        &mut self.lights
    }

    pub fn get_materials_mut(&mut self) -> &mut MaterialList {
        &mut self.materials
    }

    /// Returns an id if a single mesh was loaded, otherwise it was a scene
    pub fn load<S: AsRef<Path>>(&mut self, path: S) -> Result<LoadResult, SceneError> {
        let path = path.as_ref();
        let extension = path.extension();
        let _build_bvh = self.settings.has_flag(SceneFlags::BuildBVHs);
        if extension.is_none() {
            return Err(SceneError::NoFileExtension);
        }
        let extension = extension.unwrap();

        // Load obj files
        let extension = extension.to_str().unwrap().to_string();
        if let Some(loader) = self.loaders.get(extension.as_str()) {
            return loader.load(
                path.to_path_buf(),
                &mut self.materials,
                &mut self.objects.meshes,
            );
        }

        Err(SceneError::NoFileLoader(extension))
    }

    pub fn add_3d_object(&mut self, object: Mesh3D) -> Result<usize, SceneError> {
        let id = self.objects.meshes.push(object);
        self.objects
            .o_to_i_mapping
            .insert(id as u32, HashSet::new());
        Ok(id)
    }

    pub fn add_2d_object(&mut self, object: Mesh2D) -> Result<usize, SceneError> {
        let id = self.objects.d2_meshes.push(object);
        Ok(id)
    }

    pub fn set_3d_object(&mut self, index: usize, object: Mesh3D) -> Result<(), SceneError> {
        if self.objects.meshes.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        self.objects.meshes[index] = object;
        Ok(())
    }

    pub fn set_2d_object(&mut self, index: usize, object: Mesh2D) -> Result<(), SceneError> {
        if self.objects.d2_meshes.get(index).is_none() {
            Err(SceneError::InvalidObjectIndex(index))
        } else {
            self.objects.d2_meshes[index] = object;
            Ok(())
        }
    }

    pub fn remove_3d_object(&mut self, index: u32) -> Result<(), SceneError> {
        // TODO: Remove instances that contained this object
        // TODO: Remove scenes that contained this object
        match self.objects.meshes.erase(index as usize) {
            Ok(_) => {
                for inst in self
                    .objects
                    .o_to_i_mapping
                    .get(&index)
                    .expect("Object should exist in o_to_i_mapping")
                    .iter()
                {
                    let mut instance = self
                        .instances
                        .get(*inst as usize)
                        .expect("Instance should exist");
                    instance.set_mesh(MeshID::INVALID);
                }

                Ok(())
            }
            Err(_) => Err(SceneError::InvalidObjectIndex(index as _)),
        }
    }

    pub fn remove_2d_object(&mut self, index: usize) -> Result<(), SceneError> {
        // TODO: Remove 2d instances that contained this object
        match self.objects.d2_meshes.erase(index) {
            Ok(_) => Ok(()),
            Err(_) => Err(SceneError::InvalidObjectIndex(index)),
        }
    }

    pub fn add_instance(&mut self, index: u32) -> Result<usize, SceneError> {
        match self.objects.meshes.get(index as usize) {
            None => return Err(SceneError::InvalidObjectIndex(index as usize)),
            _ => {}
        };

        let instance = self.instances.allocate().get_id();
        self.objects
            .o_to_i_mapping
            .get_mut(&index)
            .expect("Object should exist in o_to_i_mapping")
            .insert(instance as u32);
        Ok(instance)
    }

    pub fn add_2d_instance(&mut self, index: u32) -> Result<usize, SceneError> {
        let instance_id = self.objects.d2_instances.allocate();
        self.objects.d2_instances[instance_id] = Instance2D::new(index);
        Ok(instance_id)
    }

    pub fn set_instance_object(
        &mut self,
        instance: usize,
        obj_index: ObjectRef,
    ) -> Result<(), SceneError> {
        let obj_index = if let Some(index) = obj_index {
            match self.objects.meshes.get(index as usize) {
                None => return Err(SceneError::InvalidObjectIndex(index as usize)),
                Some(_) => Some(index),
            }
        } else {
            None
        };

        match self.instances.get(instance) {
            None => return Err(SceneError::InvalidInstanceIndex(instance)),
            Some(inst) => {
                if let Some(mesh_id) = inst.get_mesh_id().as_index() {
                    self.objects
                        .o_to_i_mapping
                        .get_mut(&(mesh_id as u32))
                        .expect("Object should exist in o_to_i_mapping")
                        .remove(&(inst.get_id() as u32));
                }

                if let Some(id) = obj_index {
                    self.objects
                        .o_to_i_mapping
                        .get_mut(&id)
                        .expect("Object should exist in o_to_i_mapping")
                        .insert(inst.get_id() as u32);
                }
            }
        }

        Ok(())
    }

    pub fn remove_instance(&mut self, index: usize) -> Result<(), SceneError> {
        match self.objects.meshes.get(index) {
            None => return Err(SceneError::InvalidObjectIndex(index)),
            _ => {}
        };

        if let Some(instance) = self.instances.get(index) {
            self.instances.make_invalid(instance);
        }
        Ok(())
    }

    pub fn remove_2d_instance(&mut self, index: usize) -> Result<(), SceneError> {
        match self.objects.d2_instances.erase(index) {
            Ok(_) => Ok(()),
            Err(_) => Err(SceneError::InvalidInstanceIndex(index)),
        }
    }

    #[cfg(feature = "serde")]
    pub fn serialize<S: AsRef<Path>>(&self, path: S) -> Result<(), Box<dyn Error>> {
        use std::io::prelude::*;

        let ser_object = SerializableScene::from(self);
        let encoded: Vec<u8> = bincode::serialize(&ser_object)?;

        let mut output = OsString::from(path.as_ref().as_os_str());
        output.push(Self::FF_EXTENSION);

        let mut file = File::create(output)?;
        file.write_all(encoded.as_ref())?;
        Ok(())
    }

    #[cfg(feature = "serde")]
    pub fn deserialize<S: AsRef<Path>>(path: S) -> Result<Self, Box<dyn Error>> {
        let mut input = OsString::from(path.as_ref().as_os_str());
        input.push(Self::FF_EXTENSION);
        let file = File::open(input)?;
        let reader = BufReader::new(file);
        let mut object: SerializableScene = bincode::deserialize_from(reader)?;

        object.skins.trigger_changed_all();
        object.materials.set_changed();
        object.instances.trigger_changed_all();
        object.meshes.trigger_changed_all();
        object.animated_meshes.trigger_changed_all();
        object.lights.point_lights.trigger_changed_all();
        object.lights.spot_lights.trigger_changed_all();
        object.lights.area_lights.trigger_changed_all();
        object.lights.directional_lights.trigger_changed_all();

        let object: Self = object.into();

        Ok(object)
    }

    pub fn add_point_light(&mut self, pos: Vec3, radiance: Vec3) -> usize {
        self.lights
            .point_lights
            .push(PointLight::new(pos, radiance));
        self.lights.point_lights.len() - 1
    }

    pub fn add_spot_light(
        &mut self,
        pos: Vec3,
        direction: Vec3,
        radiance: Vec3,
        inner_angle: f32,
        outer_angle: f32,
    ) -> usize {
        self.lights.spot_lights.push(SpotLight::new(
            pos,
            direction,
            inner_angle,
            outer_angle,
            radiance,
        ));
        self.lights.spot_lights.len() - 1
    }

    pub fn add_directional_light(&mut self, direction: Vec3, radiance: Vec3) -> usize {
        self.lights
            .directional_lights
            .push(DirectionalLight::new(direction, radiance))
    }

    pub fn reset_changed(&mut self) {
        self.lights.point_lights.reset_changed();
        self.lights.spot_lights.reset_changed();
        self.lights.area_lights.reset_changed();
        self.lights.directional_lights.reset_changed();

        self.materials.reset_changed();
    }

    pub fn update_lights(&mut self) {
        let light_flags = self.materials.light_flags();
        if light_flags.not_any() {
            self.lights.area_lights = TrackedStorage::new();
            return;
        }

        let mut area_lights: Vec<AreaLight> = Vec::new();

        let mut triangle_light_ids: Vec<(u32, u32, u32)> = Vec::new();

        for (inst_idx, instance) in self.instances.iter().enumerate() {
            if let Some(mesh_id) = instance.get_mesh_id().as_index() {
                let m = &self.objects.meshes[mesh_id];
                for v in m.ranges.iter() {
                    let light_flag = light_flags.get(v.mat_id as usize);
                    if light_flag.is_none() {
                        continue;
                    }

                    if *light_flag.unwrap() {
                        for i in (v.first as usize / 3)..(v.last as usize / 3) {
                            let i0 = i;
                            let i1 = i + 1;
                            let i2 = i + 2;

                            let v0 = &m.vertices[i0];
                            let v1 = &m.vertices[i1];
                            let v2 = &m.vertices[i2];

                            let transform = instance.get_matrix();

                            let vertex0: Vec3 = (transform * Vec4::from(v0.vertex)).truncate();
                            let vertex1: Vec3 = (transform * Vec4::from(v1.vertex)).truncate();
                            let vertex2: Vec3 = (transform * Vec4::from(v2.vertex)).truncate();

                            let normal = RTTriangle::normal(vertex0, vertex1, vertex2);
                            let position = (vertex0 + vertex1 + vertex2) * (1.0 / 3.0);
                            let color = self.materials[v.mat_id as usize].color;

                            let triangle_id = i;
                            let id = area_lights.len();
                            triangle_light_ids.push((
                                mesh_id as u32,
                                triangle_id as u32,
                                id as u32,
                            ));

                            area_lights.push(AreaLight::new(
                                position,
                                Vec3::new(color[0], color[1], color[2]),
                                normal,
                                inst_idx as i32,
                                vertex0,
                                vertex1,
                                vertex2,
                            ));
                        }
                    }
                }
            }
        }

        triangle_light_ids
            .into_iter()
            .for_each(|(mesh_id, triangle_id, id)| {
                self.objects.meshes[mesh_id as usize].triangles[triangle_id as usize].light_id =
                    id as i32;
            });

        self.lights.area_lights = TrackedStorage::from(area_lights);
    }

    fn create_loaders() -> HashMap<String, Box<dyn ObjectLoader>> {
        let mut loaders: HashMap<String, Box<dyn ObjectLoader>> = HashMap::new();

        loaders.insert(
            String::from("gltf"),
            Box::new(crate::loaders::gltf::GltfLoader::default()),
        );
        loaders.insert(
            String::from("glb"),
            Box::new(crate::loaders::gltf::GltfLoader::default()),
        );
        loaders.insert(
            String::from("obj"),
            Box::new(crate::loaders::obj::ObjLoader::default()),
        );

        loaders
    }

    fn get_bounds(&self, index: ObjectRef) -> Result<AABB, SceneError> {
        let bounds = match index {
            ObjectRef::Some(id) => match self.objects.meshes.get(id as usize) {
                None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                _ => self.objects.meshes.get(id as usize).unwrap().bounds,
            },
            ObjectRef::None => AABB::empty(),
        };

        Ok(bounds)
    }

    pub fn add_camera(&mut self, width: u32, height: u32) -> usize {
        self.cameras.push(Camera::new(width, height))
    }

    pub fn get_cameras(&self) -> FlaggedIterator<'_, Camera> {
        self.cameras.iter()
    }

    pub fn get_cameras_mut(&mut self) -> FlaggedIteratorMut<'_, Camera> {
        self.cameras.iter_mut()
    }
}
