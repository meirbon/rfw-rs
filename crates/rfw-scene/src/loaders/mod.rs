use crate::{GraphDescriptor, MaterialList, Mesh3D, SceneError};
use rfw_backend::MeshId3D;
use rfw_utils::collections::TrackedStorage;
use std::path::PathBuf;

pub mod gltf;
pub mod obj;

#[derive(Debug, Clone)]
pub enum LoadResult {
    /// Reference to single mesh
    Object(MeshId3D),
    /// Indices of root nodes of scene
    Scene(GraphDescriptor),
}

#[derive(Debug, Clone)]
pub enum Error {
    ResultIsScene,
    ResultIsMesh,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Error({})",
            match self {
                Error::ResultIsScene => "Result is a scene, not a mesh",
                Error::ResultIsMesh => "Result is a mesh, not a scene",
            }
        )
    }
}

impl std::error::Error for Error {}

impl LoadResult {
    pub fn object(self) -> Result<MeshId3D, Error> {
        match self {
            LoadResult::Object(obj) => Ok(obj),
            LoadResult::Scene(_) => Err(Error::ResultIsScene),
        }
    }

    pub fn scene(self) -> Result<GraphDescriptor, Error> {
        match self {
            LoadResult::Scene(scene) => Ok(scene),
            LoadResult::Object(_) => Err(Error::ResultIsMesh),
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
