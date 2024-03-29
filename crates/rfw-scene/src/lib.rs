use rfw_backend::*;
use rfw_math::*;

mod camera;
mod constants;
mod graph;
mod instances_2d;
mod instances_3d;
mod lights;
mod loaders;
mod material;
mod objects_2d;
mod objects_3d;

mod utils;

pub use camera::*;
pub use constants::*;
pub use graph::*;
pub use instances_2d::*;
pub use instances_3d::*;
pub use l3d::prelude::{
    load::*, mat::Flip, mat::Material, mat::Texture, mat::TextureDescriptor, mat::TextureFormat,
    mat::TextureSource,
};
pub use lights::*;
pub use loaders::*;
pub use material::*;
pub use objects_2d::*;
pub use objects_3d::*;
pub use rtbvh as bvh;
pub use utils::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
use std::{error::Error, ffi::OsString, fs::File, io::BufReader};

use rfw_utils::collections::{FlaggedIterator, FlaggedIteratorMut, FlaggedStorage, TrackedStorage};
use std::sync::{PoisonError, TryLockError};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub struct PointLightRef(usize);
pub struct SpotLightRef(usize);
pub struct DirectionalLightRef(usize);

#[derive(Debug, Clone)]
pub enum SceneError {
    InvalidObjectRef,
    InvalidObjectIndex(usize),
    InvalidInstanceIndex(usize),
    InvalidSceneId(usize),
    InvalidId(usize),
    InvalidCameraId(usize),
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

impl std::fmt::Display for SceneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            Self::InvalidObjectRef => String::from("object reference was None"),
            Self::InvalidObjectIndex(idx) => format!("invalid object index {}", idx),
            Self::InvalidInstanceIndex(idx) => format!("invalid instances index {}", idx),
            Self::InvalidSceneId(id) => format!("invalid scene id {}", id),
            Self::InvalidId(id) => format!("invalid id {}", id),
            Self::InvalidCameraId(id) => format!("invalid camera id {}", id),
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

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Lights {
    pub point_lights: TrackedStorage<PointLight>,
    pub spot_lights: TrackedStorage<SpotLight>,
    pub area_lights: TrackedStorage<AreaLight>,
    pub directional_lights: TrackedStorage<DirectionalLight>,
}

impl Default for Lights {
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
    loaded_meshes_3d: HashMap<String, usize>,
    pub(crate) instances_2d: FlaggedStorage<InstanceList2D>,
    pub(crate) instances_3d: FlaggedStorage<InstanceList3D>,
    pub(crate) meshes_3d: TrackedStorage<Mesh3D>,
    pub(crate) meshes_2d: TrackedStorage<Mesh2D>,
    pub(crate) graph: graph::SceneGraph,
    pub(crate) skins: TrackedStorage<graph::Skin>,
    pub(crate) lights: Lights,
    pub(crate) materials: Materials,
    pub(crate) cameras: TrackedStorage<Camera3D>,
}

impl Default for Scene {
    fn default() -> Self {
        let loaders = Self::create_loaders();

        Self {
            loaders,
            loaded_meshes_3d: HashMap::new(),
            instances_2d: Default::default(),
            instances_3d: Default::default(),
            meshes_3d: Default::default(),
            meshes_2d: Default::default(),
            graph: Default::default(),
            skins: Default::default(),
            lights: Lights::default(),
            materials: Materials::new(),
            cameras: TrackedStorage::new(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug)]
struct SerializableScene {
    instances_3d: FlaggedStorage<InstanceList3D>,
    instances_2d: FlaggedStorage<InstanceList2D>,
    meshes_3d: TrackedStorage<Mesh3D>,
    meshes_2d: TrackedStorage<Mesh2D>,
    graph: graph::SceneGraph,
    skins: TrackedStorage<graph::Skin>,
    lights: Lights,
    materials: Materials,
    cameras: TrackedStorage<Camera3D>,
}

impl From<&Scene> for SerializableScene {
    fn from(scene: &Scene) -> Self {
        Self {
            instances_3d: scene.instances_3d.clone(),
            instances_2d: scene.instances_2d.clone(),
            meshes_3d: scene.meshes_3d.clone(),
            meshes_2d: scene.meshes_2d.clone(),
            graph: scene.graph.clone(),
            skins: scene.skins.clone(),
            lights: scene.lights.clone(),
            materials: scene.materials.clone(),
            cameras: scene.cameras.clone(),
        }
    }
}

impl From<SerializableScene> for Scene {
    fn from(s: SerializableScene) -> Self {
        Scene {
            loaders: Scene::create_loaders(),
            loaded_meshes_3d: HashMap::new(),
            instances_2d: s.instances_2d,
            instances_3d: s.instances_3d,
            meshes_3d: s.meshes_3d,
            meshes_2d: s.meshes_2d,
            graph: s.graph,
            skins: s.skins,
            lights: s.lights,
            materials: s.materials,
            cameras: s.cameras,
        }
    }
}

#[allow(dead_code)]
impl Scene {
    const FF_EXTENSION: &'static str = ".scenev1";

    pub fn new() -> Self {
        Self {
            lights: Lights::default(),
            materials: Materials::new(),
            ..Default::default()
        }
    }

    pub fn get_lights(&self) -> &Lights {
        &self.lights
    }

    pub fn get_lights_mut(&mut self) -> &mut Lights {
        &mut self.lights
    }

    pub fn get_materials(&self) -> &Materials {
        &self.materials
    }

    pub fn get_materials_mut(&mut self) -> &mut Materials {
        &mut self.materials
    }

    pub fn get_instances_3d(&self) -> &FlaggedStorage<InstanceList3D> {
        &self.instances_3d
    }

    pub fn get_instances_2d(&self) -> &FlaggedStorage<InstanceList2D> {
        &self.instances_2d
    }

    pub fn get_meshes_3d(&self) -> &TrackedStorage<Mesh3D> {
        &self.meshes_3d
    }

    pub fn get_erased_meshed_3d(&mut self) -> &[usize] {
        self.meshes_3d.get_erased()
    }

    pub fn get_meshes_2d(&self) -> &TrackedStorage<Mesh2D> {
        &self.meshes_2d
    }

    pub fn get_erased_meshed_2d(&mut self) -> &[usize] {
        self.meshes_2d.get_erased()
    }

    pub fn get_graph(&self) -> &graph::SceneGraph {
        &self.graph
    }

    pub fn get_skins(&self) -> &TrackedStorage<graph::Skin> {
        &self.skins
    }

    pub fn synchronize_graph(&mut self) -> bool {
        self.graph
            .synchronize(&mut self.meshes_3d, &mut self.instances_3d, &mut self.skins)
    }

    /// Returns an id if a single mesh was loaded, otherwise it was a scene
    pub fn load<S: AsRef<Path>>(&mut self, path: S) -> Result<LoadResult, SceneError> {
        let path = path.as_ref();
        let extension = path.extension();
        if extension.is_none() {
            return Err(SceneError::NoFileExtension);
        }
        let extension = extension.unwrap();

        // Load obj files
        let extension = extension.to_str().unwrap().to_string();
        if let Some(loader) = self.loaders.get(extension.as_str()) {
            let result = loader.load(path.to_path_buf(), &mut self.materials, &mut self.meshes_3d);
            if let Ok(r) = result.as_ref() {
                match r {
                    LoadResult::Object(i) => {
                        self.instances_3d
                            .overwrite_val(i.as_index().unwrap(), InstanceList3D::default());
                    }
                    LoadResult::Scene(s) => s.meshes.iter().for_each(|i| {
                        self.instances_3d
                            .overwrite_val(i.as_index().unwrap(), InstanceList3D::default());
                    }),
                }
            }

            return result;
        }

        Err(SceneError::NoFileLoader(extension))
    }

    pub fn add_3d<T: ToScene>(&mut self, scene: &T) -> GraphHandle {
        let scene = scene.to_scene(&mut self.meshes_3d, &mut self.instances_3d, &mut self.skins);

        let name = scene.get(scene.root_node()).unwrap().name.clone();

        let handle = self.graph.add_graph(scene);

        rfw_utils::log::info!("added graph \"{}\" with id {}", name, handle.get_id());

        handle
    }

    pub fn remove_3d(&mut self, scene: GraphHandle) {
        rfw_utils::log::info!("removed graph with id {}", scene.get_id());
        self.graph.remove_graph(
            scene,
            &mut self.meshes_3d,
            &mut self.instances_3d,
            &mut self.skins,
        );
    }

    pub fn add_3d_object<T: ToMesh3D>(&mut self, object: T) -> MeshId3D {
        let mesh = object.into_mesh_3d();
        let name = mesh.name.clone();
        let id = self.meshes_3d.push(mesh);
        self.instances_3d
            .overwrite_val(id, InstanceList3D::default());
        rfw_utils::log::info!("added 3d mesh \"{}\" with id {}", name, id);
        MeshId3D(id as _)
    }

    pub fn get_3d_object(&self, id: MeshId3D) -> Option<&Mesh3D> {
        if let Some(index) = id.as_index() {
            self.meshes_3d.get(index)
        } else {
            None
        }
    }

    pub fn get_3d_object_mut(&mut self, id: MeshId3D) -> Option<&mut Mesh3D> {
        if let Some(index) = id.as_index() {
            self.meshes_3d.get_mut(index)
        } else {
            None
        }
    }

    pub fn add_2d<T: ToMesh2D>(&mut self, object: T) -> MeshId2D {
        let mesh = object.into_mesh_2d();
        let id = self.meshes_2d.push(mesh);
        rfw_utils::log::info!("added 2d mesh with id {}", id);
        self.instances_2d
            .overwrite_val(id, InstanceList2D::default());
        MeshId2D(id as _)
    }

    pub fn get_2d_object(&self, id: MeshId2D) -> Option<&Mesh2D> {
        if let Some(index) = id.as_index() {
            self.meshes_2d.get(index)
        } else {
            None
        }
    }

    pub fn get_2d_object_mut(&mut self, id: MeshId2D) -> Option<&mut Mesh2D> {
        if let Some(index) = id.as_index() {
            self.meshes_2d.get_mut(index)
        } else {
            None
        }
    }

    /// Sets an index to the given object.
    /// This removes all instances that contained this object.
    pub fn set_3d_object(&mut self, index: MeshId3D, object: Mesh3D) -> Result<(), SceneError> {
        let index = if let Some(index) = index.as_index() {
            index
        } else {
            rfw_utils::log::warn!("3d mesh id {} was invalid", index.0);
            return Err(SceneError::InvalidObjectIndex(index.into()));
        };

        if self.meshes_3d.get(index).is_none() {
            rfw_utils::log::warn!(
                "could not update 3d mesh with id {}, invalid object index {}",
                index,
                index
            );
            return Err(SceneError::InvalidObjectIndex(index));
        }

        self.meshes_3d[index] = object;
        Ok(())
    }

    /// Sets an index to the given object.
    /// This removes all instances that contained this object.
    pub fn set_2d_object(&mut self, index: MeshId2D, object: Mesh2D) -> Result<(), SceneError> {
        let index = if let Some(index) = index.as_index() {
            index
        } else {
            rfw_utils::log::warn!("2d mesh id {} was invalid", index.0);
            return Err(SceneError::InvalidObjectIndex(index.into()));
        };

        if self.meshes_2d.get(index).is_none() {
            rfw_utils::log::warn!(
                "could not update 2d mesh with id {}, invalid object index {}",
                index,
                index
            );
            Err(SceneError::InvalidObjectIndex(index))
        } else {
            self.meshes_2d[index] = object;
            Ok(())
        }
    }

    pub fn remove_3d_object(&mut self, index: MeshId3D) -> Result<(), SceneError> {
        let index = if let Some(index) = index.as_index() {
            index
        } else {
            rfw_utils::log::warn!("3d mesh id {} was invalid", index.0);
            return Err(SceneError::InvalidObjectIndex(index.into()));
        };

        match self.meshes_3d.erase(index as usize) {
            Ok(m) => {
                rfw_utils::log::info!("removed 3d mesh \"{}\" with id {}", m.name, index);
                self.instances_3d.erase(index).unwrap();
                Ok(())
            }
            Err(_) => {
                rfw_utils::log::warn!(
                    "could not remove 3d mesh with id {}, mesh did not exist",
                    index
                );
                Err(SceneError::InvalidObjectIndex(index as _))
            }
        }
    }

    pub fn remove_2d_object(&mut self, index: MeshId2D) -> Result<(), SceneError> {
        if let Some(index) = index.as_index() {
            match self.meshes_2d.erase(index) {
                Ok(_) => {
                    rfw_utils::log::info!("removed 2d mesh with id {}", index);
                    self.instances_2d.erase(index).unwrap();
                    return Ok(());
                }
                Err(_) => {
                    rfw_utils::log::warn!(
                        "could not remove 2d mesh with id {}, mesh did not exist",
                        index
                    );
                    return Err(SceneError::InvalidObjectIndex(index));
                }
            }
        }

        rfw_utils::log::warn!("2d mesh id {} was invalid", index.0);
        Err(SceneError::InvalidObjectIndex(index.0 as usize))
    }

    pub fn add_3d_instance(&mut self, mesh: MeshId3D) -> Result<InstanceHandle3D, SceneError> {
        let id = if let Some(id) = mesh.as_index() {
            id
        } else {
            rfw_utils::log::warn!("3d mesh id {} was invalid", mesh.0);
            return Err(SceneError::InvalidObjectIndex(mesh.0 as usize));
        };

        if self.meshes_3d.get(id).is_none() {
            rfw_utils::log::warn!("3d mesh id {} did not exist", id);
            return Err(SceneError::InvalidObjectIndex(id));
        }

        let id = self.instances_3d[id].allocate();
        rfw_utils::log::info!("allocated instance {} for 3d mesh {}", id.get_id(), mesh.0,);
        Ok(id)
    }

    pub fn add_2d_instance(&mut self, mesh: MeshId2D) -> Result<InstanceHandle2D, SceneError> {
        let id = if let Some(id) = mesh.as_index() {
            id
        } else {
            rfw_utils::log::warn!("2d mesh id {} was invalid", mesh.0);
            return Err(SceneError::InvalidObjectIndex(mesh.0 as usize));
        };

        if self.meshes_2d.get(id).is_none() {
            rfw_utils::log::warn!("2d mesh id {} did not exist", id);
            return Err(SceneError::InvalidObjectIndex(id));
        }

        let id = self.instances_2d[id].allocate();
        rfw_utils::log::info!("allocated instance {} for 2d mesh {}", id.get_id(), mesh.0,);
        Ok(id)
    }

    pub fn remove_2d_instance(&mut self, handle: InstanceHandle2D) {
        rfw_utils::log::info!("invalidated instance {}", handle.get_id());
        handle.make_invalid();
    }

    pub fn add_texture(&mut self, texture: Texture) -> usize {
        self.materials.push_texture(texture)
    }

    pub fn set_texture(&mut self, id: usize, texture: Texture) -> Result<(), SceneError> {
        if let Some(t) = self.materials.get_texture_mut(id) {
            *t = texture;
            Ok(())
        } else {
            Err(SceneError::InvalidId(id))
        }
    }

    pub fn add_point_light(&mut self, pos: Vec3, radiance: Vec3) -> PointLightRef {
        PointLightRef(
            self.lights
                .point_lights
                .push(PointLight::new(pos, radiance)),
        )
    }

    pub fn add_spot_light(
        &mut self,
        pos: Vec3,
        direction: Vec3,
        radiance: Vec3,
        inner_angle: f32,
        outer_angle: f32,
    ) -> SpotLightRef {
        SpotLightRef(self.lights.spot_lights.push(SpotLight::new(
            pos,
            direction,
            inner_angle,
            outer_angle,
            radiance,
        )))
    }

    pub fn add_directional_light(
        &mut self,
        direction: Vec3,
        radiance: Vec3,
    ) -> DirectionalLightRef {
        DirectionalLightRef(
            self.lights
                .directional_lights
                .push(DirectionalLight::new(direction, radiance)),
        )
    }

    pub fn reset_changed(&mut self) {
        self.lights.point_lights.reset_changed();
        self.lights.spot_lights.reset_changed();
        self.lights.area_lights.reset_changed();
        self.lights.directional_lights.reset_changed();
        self.materials.reset_changed();
        self.skins.reset_changed();
        self.meshes_2d.reset_changed();
        self.meshes_3d.reset_changed();
        self.instances_2d
            .iter_mut()
            .for_each(|(_, i)| i.reset_changed());
        self.instances_3d
            .iter_mut()
            .for_each(|(_, i)| i.reset_changed());
    }

    pub fn update_lights(&mut self) {
        let light_flags = self.materials.light_flags();
        if light_flags.not_any() {
            self.lights.area_lights = TrackedStorage::new();
            return;
        }

        let mut area_lights: Vec<AreaLight> = Vec::new();

        let mut triangle_light_ids: Vec<(u32, u32, u32)> = Vec::new();
        let meshes = &self.meshes_3d;
        let instances = &self.instances_3d;

        meshes.iter().for_each(|(mesh_id, m)| {
            instances[mesh_id].iter().for_each(|instance| {
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
                                mesh_id as i32,
                                instance.get_id() as i32,
                                vertex0,
                                vertex1,
                                vertex2,
                            ));
                        }
                    }
                }
            });
        });

        triangle_light_ids
            .into_iter()
            .for_each(|(mesh_id, triangle_id, id)| {
                self.meshes_3d[mesh_id as usize].triangles[triangle_id as usize].light_id =
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
        self.cameras.push(Camera3D::default())
    }

    pub fn get_cameras(&self) -> FlaggedIterator<'_, Camera3D> {
        self.cameras.iter()
    }

    pub fn get_cameras_mut(&mut self) -> FlaggedIteratorMut<'_, Camera3D> {
        self.cameras.iter_mut()
    }

    pub fn set_animation_time(&mut self, handle: &GraphHandle, time: f32) {
        self.graph.set_animation(handle, time);
    }

    pub fn set_animations_time(&mut self, time: f32) {
        self.graph.set_animations(time);
    }
}
