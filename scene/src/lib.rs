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

pub mod utils;

pub use camera::*;
pub use intersector::*;
pub use lights::*;
pub use loaders::*;
pub use material::*;
pub use objects::*;
pub use renderers::*;
pub use rtbvh as bvh;
pub use utils::*;

pub use bitvec::prelude::*;
pub use instance::*;
pub use raw_window_handle;

#[cfg(feature = "object_caching")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "object_caching")]
use std::{cmp::Ordering, error::Error, ffi::OsString, fmt, fs::File, io::BufReader};

use glam::*;
use rtbvh::{Bounds, AABB};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, TryLockResult},
};

#[derive(Debug, Clone)]
pub enum SceneError {
    InvalidObjectRef,
    InvalidObjectIndex(usize),
    InvalidInstanceIndex(usize),
    LoadError(PathBuf),
    LockError,
    UnknownError,
    NoFileExtension,
    NoFileLoader(String),
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

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Objects {
    pub meshes: Arc<Mutex<TrackedStorage<Mesh>>>,
    pub animations: Arc<Mutex<TrackedStorage<graph::animation::Animation>>>,
    pub animated_meshes: Arc<Mutex<TrackedStorage<AnimatedMesh>>>,
    pub nodes: Arc<Mutex<graph::NodeGraph>>,
    pub skins: Arc<Mutex<TrackedStorage<graph::Skin>>>,
    pub instances: Arc<Mutex<TrackedStorage<Instance>>>,
}

impl Default for Objects {
    fn default() -> Self {
        Self {
            meshes: Arc::new(Mutex::new(TrackedStorage::new())),
            animations: Arc::new(Mutex::new(TrackedStorage::new())),
            animated_meshes: Arc::new(Mutex::new(TrackedStorage::new())),
            nodes: Arc::new(Mutex::new(graph::NodeGraph::new())),
            skins: Arc::new(Mutex::new(TrackedStorage::new())),
            instances: Arc::new(Mutex::new(TrackedStorage::new())),
        }
    }
}

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
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
    pub lights: Arc<Mutex<SceneLights>>,
    pub materials: Arc<Mutex<MaterialList>>,
    pub settings: Arc<Mutex<Flags>>,
}

impl Default for Scene {
    fn default() -> Self {
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

        Self {
            loaders,
            objects: Objects::default(),
            lights: Arc::new(Mutex::new(SceneLights::default())),
            materials: Arc::new(Mutex::new(MaterialList::new())),
            settings: Arc::new(Mutex::new(Flags::default())),
        }
    }
}

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug)]
struct SerializableScene {
    pub meshes: TrackedStorage<Mesh>,
    pub animated_meshes: TrackedStorage<AnimatedMesh>,
    pub nodes: graph::NodeGraph,
    pub skins: TrackedStorage<graph::Skin>,
    pub instances: TrackedStorage<Instance>,
    pub lights: SceneLights,
    pub materials: MaterialList,
    pub settings: Flags,
}

impl From<&Scene> for SerializableScene {
    fn from(scene: &Scene) -> Self {
        let lights = scene.lights.lock().unwrap();
        let mat_lock = scene.materials.lock().unwrap();
        let settings = scene.settings.lock().unwrap();

        Self {
            meshes: scene.objects.meshes.lock().unwrap().clone(),
            animated_meshes: scene.objects.animated_meshes.lock().unwrap().clone(),
            nodes: scene.objects.nodes.lock().unwrap().clone(),
            skins: scene.objects.skins.lock().unwrap().clone(),
            instances: scene.objects.instances.lock().unwrap().clone(),
            lights: lights.clone(),
            materials: mat_lock.clone(),
            settings: settings.clone(),
        }
    }
}

impl Into<Scene> for SerializableScene {
    fn into(self) -> Scene {
        Scene {
            objects: Objects::default(),
            lights: Arc::new(Mutex::new(self.lights)),
            materials: Arc::new(Mutex::new(self.materials)),
            settings: Arc::new(Mutex::new(self.settings)),
            ..Default::default()
        }
    }
}

#[allow(dead_code)]
impl Scene {
    const FF_EXTENSION: &'static str = ".scenev1";

    pub fn new() -> Self {
        Self {
            objects: Objects::default(),
            lights: Arc::new(Mutex::new(SceneLights::default())),
            materials: Arc::new(Mutex::new(MaterialList::new())),
            settings: Arc::new(Mutex::new(Flags::default())),
            ..Default::default()
        }
    }

    pub fn get_scene(&self) -> Objects {
        self.objects.clone()
    }

    pub fn get_lights(&self) -> Arc<Mutex<SceneLights>> {
        self.lights.clone()
    }

    pub fn get_materials(&self) -> Arc<Mutex<MaterialList>> {
        self.materials.clone()
    }

    pub fn lights_lock(&self) -> TryLockResult<MutexGuard<'_, SceneLights>> {
        self.lights.try_lock()
    }

    pub fn materials_lock(&self) -> TryLockResult<MutexGuard<'_, MaterialList>> {
        self.materials.try_lock()
    }

    /// Returns an id if a single mesh was loaded, otherwise it was a scene
    pub async fn load<S: AsRef<Path>>(&self, path: S) -> Result<LoadResult, SceneError> {
        let path = path.as_ref();
        let extension = path.extension();
        let _build_bvh = self
            .settings
            .lock()
            .unwrap()
            .has_flag(SceneFlags::BuildBVHs);
        if extension.is_none() {
            return Err(SceneError::NoFileExtension);
        }
        let extension = extension.unwrap();

        // TODO: Reimplement
        // #[cfg(feature = "object_caching")]
        //     {
        //         let cache_mesh = |mesh: &mut Mesh, cached_object: &PathBuf| {
        //             if build_bvh {
        //                 mesh.construct_bvh();
        //             }
        //
        //             let materials = self.materials.lock().unwrap();
        //             mesh.serialize_object(cached_object.as_path(), &materials)
        //                 .unwrap();
        //         };
        //
        //         let cached_object = path.with_extension("rm");
        //         // First check if cached object exists and check whether we can load it
        //         if cached_object.exists() {
        //             // Did object change, if so -> reload object
        //             let should_reload = if let (Ok(cached_changed), Ok(mesh_changed)) =
        //             (cached_object.as_path().metadata(), path.metadata())
        //             {
        //                 let cached_changed = cached_changed.modified();
        //                 let mesh_changed = mesh_changed.modified();
        //                 if let (Ok(cached_changed), Ok(mesh_changed)) = (cached_changed, mesh_changed) {
        //                     mesh_changed.cmp(&cached_changed) == Ordering::Less
        //                 } else {
        //                     true
        //                 }
        //             } else {
        //                 true
        //             };
        //
        //             // Object did not change, attempt to deserialize
        //             if !should_reload {
        //                 if let Ok(mut mesh) = {
        //                     let mut materials = self.materials.lock().unwrap();
        //                     // Attempt to deserialize
        //                     Mesh::deserialize_object(cached_object.as_path(), &mut materials)
        //                 } {
        //                     // Build BVH if necessary
        //                     if build_bvh && mesh.bvh.is_none() {
        //                         mesh.construct_bvh();
        //                     }
        //                     return self.add_object(mesh);
        //                 }
        //             }
        //         }
        //     }

        // Load obj files
        let extension = extension.to_str().unwrap().to_string();
        if let Some(loader) = self.loaders.get(extension.as_str()) {
            return loader.load(
                path.to_path_buf(),
                self.materials.as_ref(),
                self.objects.meshes.as_ref(),
                self.objects.animations.as_ref(),
                self.objects.animated_meshes.as_ref(),
                self.objects.nodes.as_ref(),
                self.objects.skins.as_ref(),
                self.objects.instances.as_ref(),
            );
        }

        Err(SceneError::NoFileLoader(extension))
    }

    pub fn add_object(&self, object: Mesh) -> Result<usize, SceneError> {
        let mut meshes = match self.objects.meshes.lock() {
            Ok(m) => m,
            Err(_) => return Err(SceneError::LockError),
        };
        let id = meshes.push(object);
        Ok(id)
    }

    pub fn add_animated_object(&self, object: AnimatedMesh) -> Result<usize, SceneError> {
        let mut meshes = match self.objects.animated_meshes.lock() {
            Ok(m) => m,
            Err(_) => return Err(SceneError::LockError),
        };
        let id = meshes.push(object);
        Ok(id)
    }

    pub fn set_object(&self, index: usize, object: Mesh) -> Result<(), SceneError> {
        let mut meshes = match self.objects.meshes.lock() {
            Ok(m) => m,
            Err(_) => return Err(SceneError::LockError),
        };

        if meshes.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        meshes[index] = object;
        Ok(())
    }

    pub fn set_animated_object(
        &self,
        index: usize,
        object: AnimatedMesh,
    ) -> Result<(), SceneError> {
        let mut meshes = match self.objects.animated_meshes.lock() {
            Ok(m) => m,
            Err(_) => return Err(SceneError::LockError),
        };

        if meshes.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        meshes[index] = object;
        Ok(())
    }

    pub fn remove_object(&mut self, index: usize) -> Result<(), SceneError> {
        // TODO: Remove instances that contained this object
        let mut meshes = match self.objects.meshes.lock() {
            Ok(m) => m,
            Err(_) => return Err(SceneError::LockError),
        };

        match meshes.erase(index) {
            Ok(_) => Ok(()),
            Err(_) => Err(SceneError::InvalidObjectIndex(index)),
        }
    }

    pub fn remove_animated_object(&mut self, index: usize) -> Result<(), SceneError> {
        // TODO: Remove instances that contained this object
        let mut meshes = match self.objects.animated_meshes.lock() {
            Ok(m) => m,
            Err(_) => return Err(SceneError::LockError),
        };

        match meshes.erase(index) {
            Ok(_) => Ok(()),
            Err(_) => Err(SceneError::InvalidObjectIndex(index)),
        }
    }

    pub fn add_instance(&self, index: ObjectRef) -> Result<usize, SceneError> {
        let bounds = match index {
            ObjectRef::None => {
                return Err(SceneError::InvalidObjectRef);
            }
            ObjectRef::Static(id) => match self.objects.meshes.lock() {
                Ok(m) => match m.get(id as usize) {
                    None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                    _ => m.get(id as usize).unwrap().bounds.clone(),
                },
                Err(_) => return Err(SceneError::LockError),
            },
            ObjectRef::Animated(id) => match self.objects.animated_meshes.lock() {
                Ok(m) => match m.get(id as usize) {
                    None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                    _ => m.get(id as usize).unwrap().bounds.clone(),
                },
                Err(_) => return Err(SceneError::LockError),
            },
        };

        let mut instances = match self.objects.instances.lock() {
            Ok(i) => i,
            Err(_) => return Err(SceneError::LockError),
        };

        let instance_id = instances.allocate();
        instances[instance_id] = Instance::new(index, &bounds);
        Ok(instance_id)
    }

    pub fn set_instance_object(
        &self,
        instance: usize,
        obj_index: ObjectRef,
    ) -> Result<(), SceneError> {
        let bounds = match obj_index {
            ObjectRef::None => {
                return Err(SceneError::InvalidObjectRef);
            }
            ObjectRef::Static(id) => match self.objects.meshes.lock() {
                Ok(m) => match m.get(id as usize) {
                    None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                    _ => m.get(id as usize).unwrap().bounds.clone(),
                },
                Err(_) => return Err(SceneError::LockError),
            },
            ObjectRef::Animated(id) => match self.objects.animated_meshes.lock() {
                Ok(m) => match m.get(id as usize) {
                    None => return Err(SceneError::InvalidObjectIndex(id as usize)),
                    _ => m.get(id as usize).unwrap().bounds.clone(),
                },
                Err(_) => return Err(SceneError::LockError),
            },
        };

        let mut instances = match self.objects.instances.lock() {
            Ok(i) => i,
            Err(_) => return Err(SceneError::LockError),
        };

        match instances.get_mut(instance) {
            None => return Err(SceneError::InvalidInstanceIndex(instance)),
            Some(inst) => {
                inst.object_id = obj_index;
                inst.set_bounds(bounds);
            }
        }

        Ok(())
    }

    pub fn remove_instance(&self, index: usize) -> Result<(), SceneError> {
        match self.objects.meshes.lock() {
            Ok(m) => match m.get(index) {
                None => return Err(SceneError::InvalidObjectIndex(index)),
                _ => {}
            },
            Err(_) => return Err(SceneError::LockError),
        };

        let mut instances = match self.objects.instances.lock() {
            Ok(i) => i,
            Err(_) => return Err(SceneError::LockError),
        };

        match instances.erase(index) {
            Ok(_) => Ok(()),
            Err(_) => Err(SceneError::UnknownError),
        }
    }

    #[cfg(feature = "object_caching")]
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

    #[cfg(feature = "object_caching")]
    pub fn deserialize<S: AsRef<Path>>(path: S) -> Result<Self, Box<dyn Error>> {
        let mut input = OsString::from(path.as_ref().as_os_str());
        input.push(Self::FF_EXTENSION);
        let file = File::open(input)?;
        let reader = BufReader::new(file);
        let mut object: SerializableScene = bincode::deserialize_from(reader)?;

        object.objects.instances_changed.set_all(true);
        object.lights.pl_changed.set_all(true);
        object.lights.sl_changed.set_all(true);
        object.lights.al_changed.set_all(true);
        object.lights.dl_changed.set_all(true);

        let object: Self = object.into();

        Ok(object)
    }

    pub fn add_point_light(&mut self, pos: Vec3A, radiance: Vec3A) -> Result<usize, SceneError> {
        match self.lights.try_lock() {
            Ok(mut lights) => {
                lights.point_lights.push(PointLight::new(pos, radiance));
                Ok(lights.point_lights.len() - 1)
            }
            Err(_) => Err(SceneError::LockError),
        }
    }

    pub fn add_spot_light(
        &mut self,
        pos: Vec3A,
        direction: Vec3A,
        radiance: Vec3A,
        inner_angle: f32,
        outer_angle: f32,
    ) -> Result<usize, SceneError> {
        match self.lights.try_lock() {
            Ok(mut lights) => {
                lights.spot_lights.push(SpotLight::new(
                    pos,
                    direction,
                    inner_angle,
                    outer_angle,
                    radiance,
                ));
                Ok(lights.spot_lights.len() - 1)
            }
            Err(_) => Err(SceneError::LockError),
        }
    }

    pub fn add_directional_light(
        &mut self,
        direction: Vec3A,
        radiance: Vec3A,
    ) -> Result<usize, SceneError> {
        match self.lights.try_lock() {
            Ok(mut lights) => {
                lights
                    .directional_lights
                    .push(DirectionalLight::new(direction, radiance));
                Ok(lights.directional_lights.len() - 1)
            }
            Err(_) => Err(SceneError::LockError),
        }
    }

    pub fn reset_changed(&self) -> Result<(), SceneError> {
        let lights = self.lights.try_lock();
        if let Ok(mut lights) = lights {
            lights.point_lights.reset_changed();
            lights.spot_lights.reset_changed();
            lights.area_lights.reset_changed();
            lights.directional_lights.reset_changed();
        } else {
            return Err(SceneError::LockError);
        }

        let materials = self.materials.try_lock();
        if let Ok(mut materials) = materials {
            materials.reset_changed();
        } else {
            return Err(SceneError::LockError);
        }

        Ok(())
    }

    pub fn update_lights(&self) {
        let materials = self.materials.lock().unwrap();
        let light_flags = materials.light_flags();
        if light_flags.not_any() {
            if let Ok(mut lights) = self.lights.lock() {
                lights.area_lights = TrackedStorage::new();
            }
            return;
        }

        let mut area_lights: Vec<AreaLight> = Vec::new();

        if let (Ok(meshes), Ok(anim_meshes), Ok(instances)) = (
            self.objects.meshes.lock(),
            self.objects.animated_meshes.lock(),
            self.objects.instances.lock(),
        ) {
            let mut triangle_light_ids: Vec<(u32, u32, u32)> = Vec::new();

            instances
                .iter()
                .for_each(|(inst_idx, instance)| match instance.object_id {
                    ObjectRef::None => return,
                    ObjectRef::Static(mesh_id) => {
                        let m = &meshes[mesh_id as usize];
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

                                    let vertex0: Vec3A =
                                        instance.transform_vertex(Vec4::from(v0.vertex).truncate());
                                    let vertex1: Vec3A =
                                        instance.transform_vertex(Vec4::from(v1.vertex).truncate());
                                    let vertex2: Vec3A =
                                        instance.transform_vertex(Vec4::from(v2.vertex).truncate());

                                    let normal = RTTriangle::normal(vertex0, vertex1, vertex2);
                                    let position = (vertex0 + vertex1 + vertex2) * (1.0 / 3.0);
                                    let color = materials[v.mat_id as usize].color;

                                    let triangle_id = i;
                                    let id = area_lights.len();
                                    triangle_light_ids.push((
                                        mesh_id as u32,
                                        triangle_id as u32,
                                        id as u32,
                                    ));

                                    area_lights.push(AreaLight::new(
                                        position,
                                        Vec4::from(color).truncate(),
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
                        let m = &anim_meshes[mesh_id as usize];
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

                                    let vertex0: Vec3A =
                                        instance.transform_vertex(Vec4::from(v0.vertex).truncate());
                                    let vertex1: Vec3A =
                                        instance.transform_vertex(Vec4::from(v1.vertex).truncate());
                                    let vertex2: Vec3A =
                                        instance.transform_vertex(Vec4::from(v2.vertex).truncate());

                                    let normal = RTTriangle::normal(vertex0, vertex1, vertex2);
                                    let position = (vertex0 + vertex1 + vertex2) * (1.0 / 3.0);
                                    let color = materials[v.mat_id as usize].color;

                                    let triangle_id = i;
                                    let id = area_lights.len();
                                    triangle_light_ids.push((
                                        mesh_id as u32,
                                        triangle_id as u32,
                                        id as u32,
                                    ));

                                    area_lights.push(AreaLight::new(
                                        position,
                                        Vec4::from(color).truncate(),
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

            let mut meshes = meshes;
            triangle_light_ids
                .into_iter()
                .for_each(|(mesh_id, triangle_id, id)| {
                    meshes[mesh_id as usize].triangles[triangle_id as usize].light_id = id as i32;
                });
        }

        if let Ok(mut lights) = self.lights.lock() {
            lights.area_lights = TrackedStorage::from(area_lights);
        }
    }
}

impl Bounds for Objects {
    fn bounds(&self) -> AABB {
        let mut aabb = AABB::new();

        if let Ok(instances) = self.instances.lock() {
            for (_, instance) in instances.iter() {
                aabb.grow_bb(&instance.bounds());
            }
        }

        aabb
    }
}
