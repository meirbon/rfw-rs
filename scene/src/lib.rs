use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

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
pub mod r2d;
pub mod renderer;

pub mod utils;

pub use camera::*;
pub use intersector::*;
pub use lights::*;
pub use loaders::*;
pub use material::*;
pub use objects::*;
pub use renderer::*;
pub use rtbvh as bvh;

pub use instance::*;
pub use raw_window_handle;
pub use rfw_utils::prelude::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
use std::{error::Error, ffi::OsString, fs::File, io::BufReader};

use crate::r2d::{D2Instance, D2Mesh};
use crate::utils::Flags;
use rtbvh::{Bounds, AABB};
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
    pub meshes: TrackedStorage<Mesh>,
    pub d2_meshes: TrackedStorage<D2Mesh>,
    pub animated_meshes: TrackedStorage<AnimatedMesh>,
    pub graph: graph::SceneGraph,
    pub skins: TrackedStorage<graph::Skin>,
    pub instances: TrackedStorage<Instance>,
    pub d2_instances: TrackedStorage<D2Instance>,
    pub o_to_i_mapping: HashMap<ObjectRef, HashSet<u32>>,
}

impl Default for Objects {
    fn default() -> Self {
        Self {
            meshes: TrackedStorage::new(),
            d2_meshes: TrackedStorage::new(),
            animated_meshes: TrackedStorage::new(),
            graph: graph::SceneGraph::new(),
            skins: TrackedStorage::new(),
            instances: TrackedStorage::new(),
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
}

impl Default for Scene {
    fn default() -> Self {
        let loaders = Self::create_loaders();

        Self {
            loaders,
            objects: Objects::default(),
            lights: SceneLights::default(),
            materials: MaterialList::new(),
            settings: Flags::default(),
            cameras: TrackedStorage::new(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
struct SerializableScene {
    pub meshes: TrackedStorage<Mesh>,
    pub d2_meshes: TrackedStorage<D2Mesh>,
    pub animated_meshes: TrackedStorage<AnimatedMesh>,
    pub graph: graph::SceneGraph,
    pub skins: TrackedStorage<graph::Skin>,
    pub instances: TrackedStorage<Instance>,
    pub d2_instances: TrackedStorage<D2Instance>,
    pub o_to_i_mapping: HashMap<ObjectRef, HashSet<u32>>,
    pub lights: SceneLights,
    pub materials: MaterialList,
    pub settings: Flags,
}

impl From<&Scene> for SerializableScene {
    fn from(scene: &Scene) -> Self {
        Self {
            meshes: scene.objects.meshes.clone(),
            d2_meshes: scene.objects.d2_meshes.clone(),
            animated_meshes: scene.objects.animated_meshes.clone(),
            graph: scene.objects.graph.clone(),
            skins: scene.objects.skins.clone(),
            instances: scene.objects.instances.clone(),
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
                animated_meshes: self.animated_meshes,
                graph: self.graph,
                skins: self.skins,
                instances: self.instances,
                d2_instances: self.d2_instances,
                o_to_i_mapping: self.o_to_i_mapping,
            },
            lights: self.lights,
            materials: self.materials,
            settings: self.settings,
            cameras: TrackedStorage::new(),
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
                &mut self.objects.animated_meshes,
            );
        }

        Err(SceneError::NoFileLoader(extension))
    }

    pub fn add_object(&mut self, object: Mesh) -> Result<usize, SceneError> {
        let id = self.objects.meshes.push(object);
        self.objects
            .o_to_i_mapping
            .insert(ObjectRef::Static(id as u32), HashSet::new());
        Ok(id)
    }

    pub fn add_2d_object(&mut self, object: D2Mesh) -> Result<usize, SceneError> {
        let id = self.objects.d2_meshes.push(object);
        Ok(id)
    }

    pub fn add_animated_object(&mut self, object: AnimatedMesh) -> Result<usize, SceneError> {
        let id = self.objects.animated_meshes.push(object);
        self.objects
            .o_to_i_mapping
            .insert(ObjectRef::Animated(id as u32), HashSet::new());
        Ok(id)
    }

    pub fn set_object(&mut self, index: usize, object: Mesh) -> Result<(), SceneError> {
        if self.objects.meshes.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        self.objects.meshes[index] = object;
        Ok(())
    }

    pub fn set_animated_object(
        &mut self,
        index: usize,
        object: AnimatedMesh,
    ) -> Result<(), SceneError> {
        if self.objects.animated_meshes.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        self.objects.animated_meshes[index] = object;
        Ok(())
    }

    pub fn set_2d_object(&mut self, index: usize, object: D2Mesh) -> Result<(), SceneError> {
        if self.objects.d2_meshes.get(index).is_none() {
            Err(SceneError::InvalidObjectIndex(index))
        } else {
            self.objects.d2_meshes[index] = object;
            Ok(())
        }
    }

    pub fn remove_object(&mut self, index: usize) -> Result<(), SceneError> {
        // TODO: Remove instances that contained this object
        // TODO: Remove scenes that contained this object
        match self.objects.meshes.erase(index) {
            Ok(_) => {
                for inst in self
                    .objects
                    .o_to_i_mapping
                    .get(&ObjectRef::Static(index as u32))
                    .expect("Object should exist in o_to_i_mapping")
                    .iter()
                {
                    let instance: &mut Instance = self
                        .objects
                        .instances
                        .get_mut(*inst as usize)
                        .expect("Instance should exist");
                    instance.object_id = ObjectRef::None;
                }
                self.objects
                    .o_to_i_mapping
                    .remove(&ObjectRef::Animated(index as u32));
                Ok(())
            }
            Err(_) => Err(SceneError::InvalidObjectIndex(index)),
        }
    }

    pub fn remove_animated_object(&mut self, index: usize) -> Result<(), SceneError> {
        // TODO: Remove instances that contained this object
        // TODO: Remove scenes that contained this object
        match self.objects.animated_meshes.erase(index) {
            Ok(_) => {
                for inst in self
                    .objects
                    .o_to_i_mapping
                    .get(&ObjectRef::Animated(index as u32))
                    .expect("Object should exist in o_to_i_mapping")
                    .iter()
                {
                    let instance: &mut Instance = self
                        .objects
                        .instances
                        .get_mut(*inst as usize)
                        .expect("Instance should exist");
                    instance.object_id = ObjectRef::None;
                }
                self.objects
                    .o_to_i_mapping
                    .remove(&ObjectRef::Animated(index as u32));
                Ok(())
            }
            Err(_) => Err(SceneError::InvalidObjectIndex(index)),
        }
    }

    pub fn remove_2d_object(&mut self, index: usize) -> Result<(), SceneError> {
        // TODO: Remove 2d instances that contained this object
        match self.objects.d2_meshes.erase(index) {
            Ok(_) => Ok(()),
            Err(_) => Err(SceneError::InvalidObjectIndex(index)),
        }
    }

    pub fn add_instance(&mut self, index: ObjectRef) -> Result<usize, SceneError> {
        let bounds = match index {
            ObjectRef::None => {
                return Err(SceneError::InvalidObjectRef);
            }
            ObjectRef::Static(id) => match self.objects.meshes.get(id as usize) {
                None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                Some(m) => m.bounds.clone(),
            },
            ObjectRef::Animated(id) => match self.objects.animated_meshes.get(id as usize) {
                None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                Some(m) => m.bounds.clone(),
            },
        };

        let instance_id = self.objects.instances.allocate();
        self.objects
            .o_to_i_mapping
            .get_mut(&index)
            .expect("Object should exist in o_to_i_mapping")
            .insert(instance_id as u32);
        self.objects.instances[instance_id] = Instance::new(index, &bounds);
        Ok(instance_id)
    }

    pub fn add_2d_instance(&mut self, index: u32) -> Result<usize, SceneError> {
        let instance_id = self.objects.d2_instances.allocate();
        self.objects.d2_instances[instance_id] = D2Instance::new(index);
        Ok(instance_id)
    }

    pub fn set_instance_object(
        &mut self,
        instance: usize,
        obj_index: ObjectRef,
    ) -> Result<(), SceneError> {
        let bounds = match obj_index {
            ObjectRef::None => {
                return Err(SceneError::InvalidObjectRef);
            }
            ObjectRef::Static(id) => match self.objects.meshes.get(id as usize) {
                None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                Some(m) => m.bounds.clone(),
            },
            ObjectRef::Animated(id) => match self.objects.animated_meshes.get(id as usize) {
                None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                Some(m) => m.bounds.clone(),
            },
        };

        match self.objects.instances.get_mut(instance) {
            None => return Err(SceneError::InvalidInstanceIndex(instance)),
            Some(inst) => {
                if inst.object_id != ObjectRef::None {
                    self.objects
                        .o_to_i_mapping
                        .get_mut(&inst.object_id)
                        .expect("Object should exist in o_to_i_mapping")
                        .remove(&(instance as u32));
                }
                self.objects
                    .o_to_i_mapping
                    .get_mut(&obj_index)
                    .expect("Object should exist in o_to_i_mapping")
                    .insert(instance as u32);

                inst.object_id = obj_index;
                inst.set_bounds(bounds);
            }
        }

        Ok(())
    }

    pub fn remove_instance(&mut self, index: usize) -> Result<(), SceneError> {
        match self.objects.meshes.get(index) {
            None => return Err(SceneError::InvalidObjectIndex(index)),
            _ => {}
        };

        match self.objects.instances.erase(index) {
            Ok(_) => Ok(()),
            Err(_) => Err(SceneError::UnknownError),
        }
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

        self.objects
            .instances
            .iter()
            .for_each(|(inst_idx, instance)| match instance.object_id {
                ObjectRef::None => return,
                ObjectRef::Static(mesh_id) => {
                    let m = &self.objects.meshes[mesh_id as usize];
                    for v in m.meshes.iter() {
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

                                let vertex0: Vec3 =
                                    instance.transform_vertex(Vec3::from(Vec4::from(v0.vertex)));
                                let vertex1: Vec3 =
                                    instance.transform_vertex(Vec3::from(Vec4::from(v1.vertex)));
                                let vertex2: Vec3 =
                                    instance.transform_vertex(Vec3::from(Vec4::from(v2.vertex)));

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
                ObjectRef::Animated(mesh_id) => {
                    let m = &self.objects.animated_meshes[mesh_id as usize];
                    for v in m.meshes.iter() {
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

                                let vertex0: Vec3 =
                                    instance.transform_vertex(Vec4::from(v0.vertex).into());
                                let vertex1: Vec3 =
                                    instance.transform_vertex(Vec4::from(v1.vertex).into());
                                let vertex2: Vec3 =
                                    instance.transform_vertex(Vec4::from(v2.vertex).into());

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
                                    Vec4::from(color).into(),
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
            });

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
            Box::new(crate::gltf::GltfLoader::default()),
        );
        loaders.insert(
            String::from("glb"),
            Box::new(crate::gltf::GltfLoader::default()),
        );
        loaders.insert(
            String::from("obj"),
            Box::new(crate::obj::ObjLoader::default()),
        );

        loaders
    }

    fn get_bounds(&self, index: ObjectRef) -> Result<AABB, SceneError> {
        let bounds = match index {
            ObjectRef::Static(id) => match self.objects.meshes.get(id as usize) {
                None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                _ => self.objects.meshes.get(id as usize).unwrap().bounds,
            },
            ObjectRef::Animated(id) => match self.objects.animated_meshes.get(id as usize) {
                None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                _ => {
                    self.objects
                        .animated_meshes
                        .get(id as usize)
                        .unwrap()
                        .bounds
                }
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

impl Bounds for Objects {
    fn bounds(&self) -> AABB {
        let mut aabb = AABB::new();

        for (_, instance) in self.instances.iter() {
            aabb.grow_bb(&instance.bounds());
        }

        aabb
    }
}
