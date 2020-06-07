use glam::*;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crate::material::*;
use crate::objects::mesh::ToMesh;
use crate::triangle_scene::SceneError;
use crate::utils::*;
use crate::Mesh;

enum ObjFlags {
    HasNormals = 1,
    HasUvs = 2,
}

impl Into<u8> for ObjFlags {
    fn into(self) -> u8 {
        self as u8
    }
}

pub struct Obj {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub material_ids: Vec<u32>,
    pub light_ids: Vec<i32>,
    pub flags: Flags,
    pub name: String,
}

impl Obj {
    pub fn new<T: AsRef<Path>>(
        path: T,
        mat_manager: Arc<Mutex<MaterialList>>,
    ) -> Result<Obj, SceneError> {
        let path = path.as_ref();

        let object = tobj::load_obj(path, true);
        if let Err(_) = object {
            return Err(SceneError::LoadError(path.to_path_buf()));
        }
        let (models, materials) = object.unwrap();
        let mut material_indices = vec![0; materials.len()];

        {
            let mut mat_manager = mat_manager.lock().unwrap();
            for (i, material) in materials.iter().enumerate() {
                let mut color = Vec3::new(
                    material.diffuse[0],
                    material.diffuse[1],
                    material.diffuse[2],
                );
                let specular = Vec3::new(
                    material.specular[0],
                    material.specular[1],
                    material.specular[2],
                );

                let roughness = (1.0 - material.shininess.log10() / 1000.0)
                    .max(0.0)
                    .min(1.0);
                let opacity = 1.0 - material.dissolve;

                let parent = if let Some(p) = path.parent() {
                    p.to_path_buf()
                } else {
                    PathBuf::new()
                };

                let d_path: PathBuf = parent.join(material.diffuse_texture.as_str()).to_path_buf();
                let mut n_path: PathBuf =
                    parent.join(material.normal_texture.as_str()).to_path_buf();

                let mut roughness_map: Option<PathBuf> = None;
                let mut metallic_map: Option<PathBuf> = None;
                let mut emissive_map: Option<PathBuf> = None;
                let mut sheen_map: Option<PathBuf> = None;

                // TODO: Alpha and specular maps
                material.unknown_param.iter().for_each(|(name, value)| {
                    let key = name.to_lowercase();
                    match key.as_str() {
                        "ke" => {
                            // Emissive
                            let values = value.split(" ");
                            let mut f_values = [0.0 as f32; 3];
                            let mut i = 0;
                            for value in values {
                                assert!(i <= 2);
                                let value: f32 = value.parse().unwrap();
                                f_values[i] = value;
                                i += 1;
                            }

                            let mut value: Vec3 = Vec3::from(f_values);
                            if value.cmple(Vec3::one()).all() {
                                value = value * Vec3::splat(10.0);
                            }

                            color = value.max(color);
                        }
                        "map_pr" => {
                            roughness_map = Some(parent.join(value.as_str()).to_path_buf());
                        }
                        "map_ke" => {
                            emissive_map = Some(parent.join(value.as_str()).to_path_buf());
                        }
                        "ps" | "map_ps" => {
                            sheen_map = Some(parent.join(value.as_str()).to_path_buf());
                        }
                        "pm" | "map_pm" => {
                            metallic_map = Some(parent.join(value.as_str()).to_path_buf());
                        }
                        "norm" => {
                            n_path = parent.join(value.as_str()).to_path_buf();
                        }
                        _ => {}
                    }
                });

                material_indices[i] = mat_manager.add_with_maps(
                    color,
                    roughness,
                    specular,
                    opacity,
                    Some(d_path),
                    Some(n_path),
                    roughness_map,
                    metallic_map,
                    emissive_map,
                    sheen_map,
                );
            }

            if material_indices.is_empty() {
                material_indices.push(mat_manager.add(
                    Vec3::new(1.0, 0.0, 0.0),
                    1.0,
                    Vec3::zero(),
                    1.0,
                ));
            }
        }

        let mut flags = Flags::new();
        let num_vertices: usize = models.iter().map(|m| m.mesh.indices.len()).sum();

        let mut vertices = Vec::with_capacity(num_vertices);
        let mut normals = Vec::with_capacity(num_vertices);
        let mut uvs = Vec::with_capacity(num_vertices);
        let mut material_ids = Vec::with_capacity(num_vertices);
        let light_ids = vec![-1; num_vertices];

        for m in models.iter() {
            let mesh = &m.mesh;

            let mut i = 0;
            for idx in &mesh.indices {
                let idx = *idx as usize;
                let i0 = 3 * idx;
                let i1 = i0 + 1;
                let i2 = i0 + 2;

                let pos = [mesh.positions[i0], mesh.positions[i1], mesh.positions[i2]];

                let normal = if !mesh.normals.is_empty() {
                    flags.set_flag(ObjFlags::HasNormals);
                    [mesh.normals[i0], mesh.normals[i1], mesh.normals[i2]]
                } else {
                    [0.0; 3]
                };

                let uv = if !mesh.texcoords.is_empty() {
                    flags.set_flag(ObjFlags::HasUvs);
                    [mesh.texcoords[idx * 2], mesh.texcoords[idx * 2 + 1]]
                } else {
                    [0.0; 2]
                };

                vertices.push(pos.into());
                normals.push(normal.into());
                uvs.push(uv.into());

                if i % 3 == 0 {
                    let material_id = if mesh.material_id.is_some() {
                        material_indices[mesh.material_id.unwrap()]
                    } else {
                        material_indices[0]
                    };

                    material_ids.push(material_id as u32);
                }

                i = i + 1;
            }
        }

        Ok(Obj {
            vertices,
            normals,
            uvs,
            material_ids,
            light_ids,
            flags,
            name: String::from(path.to_str().unwrap()),
        })
    }
}

impl ToMesh for Obj {
    fn into_mesh(self) -> Mesh {
        Mesh::new(
            self.vertices.as_slice(),
            self.normals.as_slice(),
            self.uvs.as_slice(),
            self.material_ids.as_slice(),
            Some(self.name),
        )
    }
}