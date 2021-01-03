use crate::graph::SceneDescriptor;
use crate::{MaterialList, Mesh3D, ObjectRef, SceneError};
use rfw_utils::collections::TrackedStorage;
use std::path::PathBuf;

pub mod gltf;
pub mod obj;

#[derive(Debug, Clone)]
pub enum LoadResult {
    /// Reference to single mesh
    Object(ObjectRef),
    /// Indices of root nodes of scene
    Scene(SceneDescriptor),
}

impl LoadResult {
    pub fn object(self) -> Result<ObjectRef, ()> {
        match self {
            LoadResult::Object(obj) => Ok(obj),
            LoadResult::Scene(_) => Err(()),
        }
    }

    pub fn scene(self) -> Result<SceneDescriptor, ()> {
        match self {
            LoadResult::Object(_) => Err(()),
            LoadResult::Scene(scene) => Ok(scene),
        }
    }
}

pub trait ObjectLoader: std::fmt::Display + std::fmt::Debug {
    fn load(
        &self,
        path: PathBuf,
        mat_manager: &mut MaterialList,
        mesh_storage: &mut TrackedStorage<Mesh3D>,
    ) -> Result<LoadResult, SceneError>;
}
