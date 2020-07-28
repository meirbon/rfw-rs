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

pub trait ObjectLoader: std::fmt::Display + std::fmt::Debug {
    fn load(
        &self,
        path: PathBuf,
        mat_manager: &Mutex<MaterialList>,
        mesh_storage: &Mutex<TrackedStorage<Mesh>>,
        animated_mesh_storage: &Mutex<TrackedStorage<AnimatedMesh>>,
        node_storage: &Mutex<NodeGraph>,
        skin_storage: &Mutex<TrackedStorage<Skin>>,
        instances_storage: &Mutex<TrackedStorage<Instance>>,
    ) -> Result<LoadResult, crate::triangle_scene::SceneError>;
}
