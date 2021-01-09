use rfw_backend::*;
use rfw_math::*;

pub mod camera;
pub mod constants;
pub mod graph;
pub mod instances_2d;
pub mod instances_3d;
pub mod lights;
pub mod loaders;
pub mod material;
pub mod objects;
pub mod r2d;

pub mod utils;

pub use camera::*;
pub use graph::*;
pub use instances_2d::*;
pub use instances_3d::*;
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
    InvalidSceneID(usize),
    InvalidID(usize),
    InvalidCameraID(usize),
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
    pub meshes_3d: TrackedStorage<Mesh3D>,
    pub meshes_2d: TrackedStorage<Mesh2D>,
    pub graph: graph::SceneGraph,
    pub skins: TrackedStorage<graph::Skin>,
    pub o_to_i_mapping: HashMap<usize, HashSet<usize>>,
}

impl Default for Objects {
    fn default() -> Self {
        Self {
            meshes_3d: TrackedStorage::new(),
            meshes_2d: TrackedStorage::new(),
            graph: graph::SceneGraph::new(),
            skins: TrackedStorage::new(),
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
    pub instances_3d: InstanceList3D,
    pub instances_2d: InstanceList2D,
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
            instances_3d: InstanceList3D::new(),
            instances_2d: InstanceList2D::new(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
struct SerializableScene {
    pub instances_3d: instances_3d::List3D,
    pub instances_2d: instances_2d::List2D,
    pub meshes: TrackedStorage<Mesh3D>,
    pub d2_meshes: TrackedStorage<Mesh2D>,
    pub graph: graph::SceneGraph,
    pub skins: TrackedStorage<graph::Skin>,
    pub o_to_i_mapping: HashMap<usize, HashSet<usize>>,
    pub lights: SceneLights,
    pub materials: MaterialList,
    pub settings: Flags,
}

impl From<&Scene> for SerializableScene {
    fn from(scene: &Scene) -> Self {
        Self {
            instances_3d: scene.instances_3d.clone_inner(),
            instances_2d: scene.instances_2d.clone_inner(),
            meshes: scene.objects.meshes_3d.clone(),
            d2_meshes: scene.objects.meshes_2d.clone(),
            graph: scene.objects.graph.clone(),
            skins: scene.objects.skins.clone(),
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
                meshes_3d: self.meshes,
                meshes_2d: self.d2_meshes,
                graph: self.graph,
                skins: self.skins,
                o_to_i_mapping: self.o_to_i_mapping,
            },
            lights: self.lights,
            materials: self.materials,
            settings: self.settings,
            cameras: TrackedStorage::new(),
            instances_3d: self.instances_3d.into(),
            instances_2d: self.instances_2d.into(),
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
                &mut self.objects.meshes_3d,
            );
        }

        Err(SceneError::NoFileLoader(extension))
    }

    pub fn add_3d_scene<T: ToScene>(&mut self, scene: &T) -> GraphHandle {
        self.objects
            .graph
            .add_graph(scene.into_scene(&mut self.instances_3d, &mut self.objects.skins))
    }

    pub fn remove_3d_scene(&mut self, scene: GraphHandle) {
        self.objects
            .graph
            .remove_graph(scene, &mut self.instances_3d, &mut self.objects.skins);
    }

    pub fn add_3d_object(&mut self, object: Mesh3D) -> usize {
        let id = self.objects.meshes_3d.push(object);
        self.objects.o_to_i_mapping.insert(id, HashSet::new());
        id
    }

    pub fn add_2d_object(&mut self, object: Mesh2D) -> usize {
        self.objects.meshes_2d.push(object)
    }

    pub fn set_3d_object(&mut self, index: usize, object: Mesh3D) -> Result<(), SceneError> {
        if self.objects.meshes_3d.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        self.objects.meshes_3d[index] = object;
        Ok(())
    }

    pub fn set_2d_object(&mut self, index: usize, object: Mesh2D) -> Result<(), SceneError> {
        if self.objects.meshes_2d.get(index).is_none() {
            Err(SceneError::InvalidObjectIndex(index))
        } else {
            self.objects.meshes_2d[index] = object;
            Ok(())
        }
    }

    pub fn remove_3d_object(&mut self, index: usize) -> Result<(), SceneError> {
        // TODO: Remove instances that contained this object
        // TODO: Remove scenes that contained this object
        match self.objects.meshes_3d.erase(index as usize) {
            Ok(_) => {
                for inst in self
                    .objects
                    .o_to_i_mapping
                    .get(&index)
                    .expect("Object should exist in o_to_i_mapping")
                    .iter()
                {
                    let mut instance = self
                        .instances_3d
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
        match self.objects.meshes_2d.erase(index) {
            Ok(_) => Ok(()),
            Err(_) => Err(SceneError::InvalidObjectIndex(index)),
        }
    }

    pub fn add_instance(&mut self, index: usize) -> Result<InstanceHandle3D, SceneError> {
        if self.objects.meshes_3d.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        let instance = self.instances_3d.allocate();
        self.objects
            .o_to_i_mapping
            .get_mut(&index)
            .expect("Object should exist in o_to_i_mapping")
            .insert(instance.get_id());
        Ok(instance)
    }

    pub fn add_2d_instance(&mut self, index: usize) -> Result<InstanceHandle2D, SceneError> {
        if self.objects.meshes_2d.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        let mut instance_id = self.instances_2d.allocate();
        instance_id.set_mesh(MeshID(index as _));
        Ok(instance_id)
    }

    pub fn set_instance_object(&mut self, instance: usize, index: usize) -> Result<(), SceneError> {
        assert!(self.objects.meshes_3d.get(index as usize).is_some());

        match self.instances_3d.get(instance) {
            None => return Err(SceneError::InvalidInstanceIndex(instance)),
            Some(inst) => {
                if let Some(mesh_id) = inst.get_mesh_id().as_index() {
                    self.objects
                        .o_to_i_mapping
                        .get_mut(&(mesh_id))
                        .expect("Object should exist in o_to_i_mapping")
                        .remove(&(inst.get_id()));
                }

                self.objects
                    .o_to_i_mapping
                    .get_mut(&index)
                    .expect("Object should exist in o_to_i_mapping")
                    .insert(inst.get_id());
            }
        }

        Ok(())
    }

    pub fn remove_3d_instance(&mut self, index: usize) {
        if let Some(instance) = self.instances_3d.get(index) {
            self.instances_3d.make_invalid(instance);
        }
    }

    pub fn remove_2d_instance(&mut self, index: usize) {
        if let Some(instance) = self.instances_2d.get(index) {
            self.instances_2d.make_invalid(instance);
        }
    }

    pub fn add_texture(&mut self, texture: Texture) -> usize {
        self.materials.push_texture(texture)
    }

    pub fn set_texture(&mut self, id: usize, texture: Texture) -> Result<(), SceneError> {
        if let Some(t) = self.materials.get_texture_mut(id) {
            *t = texture;
            Ok(())
        } else {
            Err(SceneError::InvalidID(id))
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

        for (inst_idx, instance) in self.instances_3d.iter().enumerate() {
            if let Some(mesh_id) = instance.get_mesh_id().as_index() {
                let m = &self.objects.meshes_3d[mesh_id];
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

                            let vertex0: Vec3 = (transform * v0.vertex).truncate();
                            let vertex1: Vec3 = (transform * v1.vertex).truncate();
                            let vertex2: Vec3 = (transform * v2.vertex).truncate();

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
                self.objects.meshes_3d[mesh_id as usize].triangles[triangle_id as usize].light_id =
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

    pub fn add_3d_camera(&mut self) -> usize {
        self.cameras.push(Camera::default())
    }

    pub fn get_cameras(&self) -> FlaggedIterator<'_, Camera> {
        self.cameras.iter()
    }

    pub fn get_cameras_mut(&mut self) -> FlaggedIteratorMut<'_, Camera> {
        self.cameras.iter_mut()
    }
}
