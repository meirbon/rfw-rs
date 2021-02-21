use crate::{GraphDescriptor, LoadResult, MaterialList, Mesh3D, ObjectLoader, SceneError};
use l3d::load::NodeDescriptor;
use rfw_backend::MeshId3D;
use rfw_utils::collections::TrackedStorage;
use std::collections::HashMap;
use std::path::PathBuf;

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

impl ObjectLoader for GltfLoader {
    fn load(
        &self,
        path: PathBuf,
        mat_manager: &mut MaterialList,
        mesh_storage: &mut TrackedStorage<Mesh3D>,
    ) -> Result<LoadResult, SceneError> {
        let loader = l3d::LoadInstance::new().with_default();
        let mut scene: l3d::load::SceneDescriptor = match loader.load(l3d::load::LoadOptions {
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

            meshes.push(mesh_storage.push(Mesh3D::from(mesh)) as u32);
        }

        for node in scene.nodes.iter_mut() {
            traverse_tree(node, meshes.as_slice());
        }

        let meshes = meshes.iter().map(|i| MeshId3D::from(*i as usize)).collect();
        Ok(LoadResult::Scene(GraphDescriptor {
            meshes,
            nodes: scene.nodes,
            animations: scene.animations,
        }))
    }
}

fn traverse_tree(desc: &mut NodeDescriptor, meshes: &[u32]) {
    for mesh in desc.meshes.iter_mut() {
        *mesh = meshes[*mesh as usize];
    }

    for child in desc.child_nodes.iter_mut() {
        traverse_tree(child, meshes);
    }
}
