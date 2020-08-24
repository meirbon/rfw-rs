use crate::graph::{NodeGraph, Skin};
use crate::utils::TrackedStorage;
use crate::{AnimatedMesh, MaterialList, Mesh, ObjectRef};
use std::path::PathBuf;
use std::sync::RwLock;

pub mod gltf;
pub mod obj;

#[derive(Debug, Clone)]
pub enum LoadResult {
    /// Reference to single mesh
    Object(ObjectRef),
    /// Indices of root nodes of scene
    Scene(NodeGraph),
}

impl LoadResult {
    pub fn object(self) -> Result<ObjectRef, ()> {
        match self {
            LoadResult::Object(obj) => Ok(obj),
            LoadResult::Scene(_) => Err(()),
        }
    }

    pub fn scene(self) -> Result<NodeGraph, ()> {
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
        mat_manager: &RwLock<MaterialList>,
        mesh_storage: &RwLock<TrackedStorage<Mesh>>,
        animated_mesh_storage: &RwLock<TrackedStorage<AnimatedMesh>>,
        skin_storage: &RwLock<TrackedStorage<Skin>>,
    ) -> Result<LoadResult, crate::SceneError>;
}
