use rfw_math::*;
use crate::{
    graph::{NodeDescriptor, SceneDescriptor},
    {AnimatedMesh, LoadResult, MaterialList, Mesh, ObjectLoader, ObjectRef, SceneError},
};
use std::collections::HashMap;
use std::path::PathBuf;
use rfw_utils::collections::TrackedStorage;

#[derive(Debug, Copy, Clone)]
pub struct GltfLoader {}

impl std::fmt::Display for GltfLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "gltf-loader")
    }
}

impl Default for GltfLoader {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Debug, Clone)]
enum LoadedMesh {
    Static(Mesh),
    Animated(AnimatedMesh),
}

impl ObjectLoader for GltfLoader {
    fn load(
        &self,
        path: PathBuf,
        mat_manager: &mut MaterialList,
        mesh_storage: &mut TrackedStorage<Mesh>,
        animated_mesh_storage: &mut TrackedStorage<AnimatedMesh>,
    ) -> Result<LoadResult, SceneError> {
        let loader = l3d::LoadInstance::new().with_default();
        let scene: l3d::load::SceneDescriptor = match loader.load(l3d::load::LoadOptions {
            path: path.clone(),
            ..Default::default()
        }) {
            l3d::LoadResult::Mesh(_) => return Err(SceneError::LoadError(path)),
            l3d::LoadResult::Scene(s) => s,
            l3d::LoadResult::None(_) => return Err(SceneError::LoadError(path)),
        };

        let mut mat_indices: HashMap<u32, u32> = HashMap::new();
        let mut tex_maps: HashMap<u32, u32> = HashMap::new();

        let (materials, textures) = scene.materials.take();
        for (i, texture) in textures.into_iter().enumerate() {
            tex_maps.insert(i as u32, mat_manager.push_texture(texture) as u32);
        }
        for (i, mut mat) in materials.into_iter().enumerate() {
            // Update texture ids
            if mat.diffuse_tex >= 0 {
                mat.diffuse_tex = tex_maps[&(mat.diffuse_tex as u32)] as i16;
            }
            if mat.normal_tex >= 0 {
                mat.normal_tex = tex_maps[&(mat.normal_tex as u32)] as i16;
            }
            if mat.metallic_roughness_tex >= 0 {
                mat.metallic_roughness_tex = tex_maps[&(mat.metallic_roughness_tex as u32)] as i16;
            }
            if mat.emissive_tex >= 0 {
                mat.emissive_tex = tex_maps[&(mat.emissive_tex as u32)] as i16;
            }
            if mat.sheen_tex >= 0 {
                mat.sheen_tex = tex_maps[&(mat.sheen_tex as u32)] as i16;
            }

            mat_indices.insert(i as u32, mat_manager.push(mat) as u32);
        }

        let mut meshes = Vec::with_capacity(scene.meshes.len());

        for (_, mut mesh) in scene.meshes.into_iter().enumerate() {
            mesh.material_ids.iter_mut().for_each(|i| {
                *i = mat_indices[&(*i as u32)] as i32;
            });
            if mesh.skeleton.is_some() {
                meshes.push(LoadedMesh::Animated(AnimatedMesh::from(mesh)));
            } else {
                meshes.push(LoadedMesh::Static(Mesh::from(mesh)));
            }
        }

        let meshes: Vec<ObjectRef> = meshes
            .into_iter()
            .map(|m| match m {
                LoadedMesh::Static(m) => {
                    let mesh_id = mesh_storage.push(m);
                    ObjectRef::Static(mesh_id as u32)
                }
                LoadedMesh::Animated(m) => {
                    let mesh_id = animated_mesh_storage.push(m);
                    ObjectRef::Animated(mesh_id as u32)
                }
            })
            .collect();

        let mut node_descriptors = Vec::with_capacity(scene.nodes.len());
        node_descriptors.push(load_node(&meshes, &scene.nodes[0]));

        Ok(LoadResult::Scene(SceneDescriptor {
            nodes: node_descriptors,
            animations: scene.animations,
        }))
    }
}

fn load_node(meshes: &Vec<ObjectRef>, node: &l3d::load::NodeDescriptor) -> NodeDescriptor {
    let mut node_meshes = vec![];
    for mesh in node.meshes.iter() {
        node_meshes.push(meshes[*mesh as usize]);
    }

    let mut child_nodes = vec![];
    if !node.child_nodes.is_empty() {
        child_nodes.reserve(node.child_nodes.len());
        for child in node.child_nodes.iter() {
            child_nodes.push(load_node(meshes, child));
        }
    }

    NodeDescriptor {
        name: node.name.clone(),
        child_nodes,

        translation: Vec3::from(node.translation),
        rotation: Quat::from(Vec4::from(node.rotation)),
        scale: Vec3::from(node.scale),

        meshes: node_meshes,
        skin: node.skin.clone(),
        weights: node.weights.clone(),

        id: node.id,
    }
}
