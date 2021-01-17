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

use rfw_utils::collections::{FlaggedIterator, FlaggedIteratorMut, FlaggedStorage, TrackedStorage};
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
    pub instances_2d: FlaggedStorage<InstanceList2D>,
    pub instances_3d: FlaggedStorage<InstanceList3D>,
    pub meshes_3d: TrackedStorage<Mesh3D>,
    pub meshes_2d: TrackedStorage<Mesh2D>,
    pub graph: graph::SceneGraph,
    pub skins: TrackedStorage<graph::Skin>,
}

impl Default for Objects {
    fn default() -> Self {
        Self {
            instances_2d: Default::default(),
            instances_3d: Default::default(),
            meshes_3d: Default::default(),
            meshes_2d: Default::default(),
            graph: Default::default(),
            skins: Default::default(),
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
    lights: SceneLights,
    materials: MaterialList,
    cameras: TrackedStorage<Camera>,
}

impl From<&Scene> for SerializableScene {
    fn from(scene: &Scene) -> Self {
        Self {
            instances_3d: scene.objects.instances_3d.clone(),
            instances_2d: scene.objects.instances_2d.clone(),
            meshes_3d: scene.objects.meshes_3d.clone(),
            meshes_2d: scene.objects.meshes_2d.clone(),
            graph: scene.objects.graph.clone(),
            skins: scene.objects.skins.clone(),
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
            objects: Objects {
                instances_2d: s.instances_2d,
                instances_3d: s.instances_3d,
                meshes_3d: s.meshes_3d,
                meshes_2d: s.meshes_2d,
                graph: s.graph,
                skins: s.skins,
            },
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
            objects: Objects::default(),
            lights: SceneLights::default(),
            materials: MaterialList::new(),
            ..Default::default()
        }
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
            let result = loader.load(
                path.to_path_buf(),
                &mut self.materials,
                &mut self.objects.meshes_3d,
            );
            if let Ok(r) = result.as_ref() {
                match r {
                    LoadResult::Object(i) => {
                        self.objects
                            .instances_3d
                            .overwrite_val(i.as_index().unwrap(), InstanceList3D::default());
                    }
                    LoadResult::Scene(s) => s.meshes.iter().for_each(|i| {
                        self.objects
                            .instances_3d
                            .overwrite_val(i.as_index().unwrap(), InstanceList3D::default());
                    }),
                }
            }

            return result;
        }

        Err(SceneError::NoFileLoader(extension))
    }

    pub fn add_3d_scene<T: ToScene>(&mut self, scene: &T) -> GraphHandle {
        self.objects.graph.add_graph(scene.into_scene(
            &mut self.objects.meshes_3d,
            &mut self.objects.instances_3d,
            &mut self.objects.skins,
        ))
    }

    pub fn remove_3d_scene(&mut self, scene: GraphHandle) {
        self.objects.graph.remove_graph(
            scene,
            &mut self.objects.meshes_3d,
            &mut self.objects.instances_3d,
            &mut self.objects.skins,
        );
    }

    pub fn add_3d_object<T: ToMesh>(&mut self, object: T) -> MeshID {
        let id = self.objects.meshes_3d.push(object.into_mesh());
        self.objects
            .instances_3d
            .overwrite_val(id, InstanceList3D::default());
        MeshID(id as _)
    }

    pub fn get_3d_object(&self, id: MeshID) -> Option<&Mesh3D> {
        if let Some(index) = id.as_index() {
            self.objects.meshes_3d.get(index)
        } else {
            None
        }
    }

    pub fn get_3d_object_mut(&mut self, id: MeshID) -> Option<&mut Mesh3D> {
        if let Some(index) = id.as_index() {
            self.objects.meshes_3d.get_mut(index)
        } else {
            None
        }
    }

    pub fn add_2d_object(&mut self, object: Mesh2D) -> MeshID {
        let id = self.objects.meshes_2d.push(object);
        self.objects
            .instances_2d
            .overwrite_val(id, InstanceList2D::default());
        MeshID(id as _)
    }

    pub fn get_2d_object(&self, id: MeshID) -> Option<&Mesh2D> {
        if let Some(index) = id.as_index() {
            self.objects.meshes_2d.get(index)
        } else {
            None
        }
    }

    pub fn get_2d_object_mut(&mut self, id: MeshID) -> Option<&mut Mesh2D> {
        if let Some(index) = id.as_index() {
            self.objects.meshes_2d.get_mut(index)
        } else {
            None
        }
    }

    /// Sets an index to the given object.
    /// This removes all instances that contained this object.
    pub fn set_3d_object(&mut self, index: MeshID, object: Mesh3D) -> Result<(), SceneError> {
        let index = if let Some(index) = index.as_index() {
            index
        } else {
            return Err(SceneError::InvalidObjectIndex(index.into()));
        };

        if self.objects.meshes_3d.get(index).is_none() {
            return Err(SceneError::InvalidObjectIndex(index));
        }

        self.objects.meshes_3d[index] = object;
        self.objects.instances_3d[index].set_all_flags(InstanceFlags3D::all());
        Ok(())
    }

    /// Sets an index to the given object.
    /// This removes all instances that contained this object.
    pub fn set_2d_object(&mut self, index: MeshID, object: Mesh2D) -> Result<(), SceneError> {
        let index = if let Some(index) = index.as_index() {
            index
        } else {
            return Err(SceneError::InvalidObjectIndex(index.into()));
        };

        if self.objects.meshes_2d.get(index).is_none() {
            Err(SceneError::InvalidObjectIndex(index))
        } else {
            self.objects.meshes_2d[index] = object;
            self.objects.instances_2d[index].set_all_flags(InstanceFlags2D::all());
            Ok(())
        }
    }

    pub fn remove_3d_object(&mut self, index: MeshID) -> Result<(), SceneError> {
        let index = if let Some(index) = index.as_index() {
            index
        } else {
            return Err(SceneError::InvalidObjectIndex(index.into()));
        };

        match self.objects.meshes_3d.erase(index as usize) {
            Ok(_) => {
                self.objects.instances_3d.erase(index).unwrap();
                Ok(())
            }
            Err(_) => Err(SceneError::InvalidObjectIndex(index as _)),
        }
    }

    pub fn remove_2d_object(&mut self, index: usize) -> Result<(), SceneError> {
        // TODO: Remove 2d instances that contained this object
        match self.objects.meshes_2d.erase(index) {
            Ok(_) => {
                self.objects.instances_2d.erase(index).unwrap();
                Ok(())
            }
            Err(_) => Err(SceneError::InvalidObjectIndex(index)),
        }
    }

    pub fn add_3d_instance(&mut self, mesh: MeshID) -> Result<InstanceHandle3D, SceneError> {
        let id = if let Some(id) = mesh.as_index() {
            id
        } else {
            return Err(SceneError::InvalidObjectIndex(mesh.0 as usize));
        };

        if self.objects.meshes_3d.get(id).is_none() {
            return Err(SceneError::InvalidObjectIndex(id));
        }

        Ok(self.objects.instances_3d[id].allocate())
    }

    pub fn add_2d_instance(&mut self, mesh: MeshID) -> Result<InstanceHandle2D, SceneError> {
        let id = if let Some(id) = mesh.as_index() {
            id
        } else {
            return Err(SceneError::InvalidObjectIndex(mesh.0 as usize));
        };

        if self.objects.meshes_2d.get(id).is_none() {
            return Err(SceneError::InvalidObjectIndex(id));
        }

        Ok(self.objects.instances_2d[id].allocate())
    }

    pub fn remove_2d_instance(&mut self, handle: InstanceHandle2D) {
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
        object.meshes_3d.trigger_changed_all();
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
        let meshes = &self.objects.meshes_3d;
        let instances = &self.objects.instances_3d;

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
