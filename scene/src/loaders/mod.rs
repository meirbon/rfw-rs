use crate::graph::SceneDescriptor;
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
        mat_manager: &RwLock<MaterialList>,
        mesh_storage: &RwLock<TrackedStorage<Mesh>>,
        animated_mesh_storage: &RwLock<TrackedStorage<AnimatedMesh>>,
    ) -> Result<LoadResult, crate::SceneError>;
}

/*pub struct MaterialId(u32);
pub struct MeshId(u32);
pub struct AnimationId(u32);
pub struct AnimatedMeshId(u32);
pub struct SkinId(u32);
pub struct InstanceId(u32);

pub struct AssetManager {
    mat_manager: Mutex<MaterialList>,
    mesh_storage: Mutex<TrackedStorage<Mesh>>,
    animation_storage: Mutex<TrackedStorage<Animation>>,
    animated_mesh_storage: Mutex<TrackedStorage<AnimatedMesh>>,
    skin_storage: Mutex<TrackedStorage<Skin>>,
    instances_storage: Mutex<TrackedStorage<Instance>>,
}

impl AssetManager {
    pub fn new() -> Self {
        AssetManager {
            ..Default::default()
        }
    }
}*/
