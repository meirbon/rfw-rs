use crate::graph::animation::Animation;
use crate::graph::{NodeGraph, Skin};
use crate::utils::TrackedStorage;
use crate::{AnimatedMesh, Instance, MaterialList, Mesh, ObjectRef};
use std::path::PathBuf;
use std::sync::Mutex;

pub mod gltf;
pub mod obj;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum LoadResult {
    /// Reference to single mesh
    Object(ObjectRef),
    /// Indices of root nodes of scene
    Scene(Vec<u32>),
}

impl LoadResult {
    pub fn object(self) -> Result<ObjectRef, ()> {
        match self {
            LoadResult::Object(obj) => Ok(obj),
            LoadResult::Scene(_) => Err(()),
        }
    }

    pub fn scene(self) -> Result<Vec<u32>, ()> {
        match self {
            LoadResult::Object(_) => Err(()),
            LoadResult::Scene(nodes) => Ok(nodes),
        }
    }
}

pub trait ObjectLoader: std::fmt::Display + std::fmt::Debug {
    fn load(
        &self,
        path: PathBuf,
        mat_manager: &Mutex<MaterialList>,
        mesh_storage: &Mutex<TrackedStorage<Mesh>>,
        animation_storage: &Mutex<TrackedStorage<Animation>>,
        animated_mesh_storage: &Mutex<TrackedStorage<AnimatedMesh>>,
        node_storage: &Mutex<NodeGraph>,
        skin_storage: &Mutex<TrackedStorage<Skin>>,
        instances_storage: &Mutex<TrackedStorage<Instance>>,
    ) -> Result<LoadResult, crate::SceneError>;
}
