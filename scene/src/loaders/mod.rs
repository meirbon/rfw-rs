use crate::graph::{Node, Skin};
use crate::utils::{FlaggedStorage, TrackedStorage};
use crate::{AnimatedMesh, Instance, MaterialList, Mesh, ObjectRef};
use std::path::PathBuf;
use std::sync::Mutex;

pub mod gltf;
pub mod obj;

pub trait ObjectLoader: std::fmt::Display + std::fmt::Debug {
    fn load(
        &self,
        path: PathBuf,
        mat_manager: &Mutex<MaterialList>,
        mesh_storage: &Mutex<TrackedStorage<Mesh>>,
        animated_mesh_storage: &Mutex<TrackedStorage<AnimatedMesh>>,
        node_storage: &Mutex<TrackedStorage<Node>>,
        skin_storage: &Mutex<FlaggedStorage<Skin>>,
        instances: &Mutex<TrackedStorage<Instance>>,
    ) -> Result<Option<ObjectRef>, crate::triangle_scene::SceneError>;
}
