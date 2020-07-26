use std::path::PathBuf;
use crate::utils::{TrackedStorage, FlaggedStorage};
use crate::{Mesh, AnimatedMesh, MaterialList, Instance, ObjectRef};
use std::sync::Mutex;
use crate::graph::{Node, Skin};

pub mod obj;
pub mod gltf;

pub trait ObjectLoader : std::fmt::Display + std::fmt::Debug {
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