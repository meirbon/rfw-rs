use glam::*;
use std::path::Path;
use std::error::Error;

use crate::utils::*;
use crate::material::*;
use crate::objects::RTMesh;
use crate::objects::mesh::ToMesh;
use crate::RastMesh;

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
}

impl Obj {
    pub fn new<T: AsRef<Path>>(path: T, mat_manager: &mut MaterialList) -> Result<Obj, Box<dyn Error>> {
        let path = path.as_ref();
        let object = tobj::load_obj(path);
        if let Err(e) = object { return Err(Box::new(e)); }
        let (models, materials) = object.unwrap();

        let mut material_indices = vec![0; materials.len()];

        for (i, material) in materials.iter().enumerate() {
            let color = vec3(material.diffuse[0], material.diffuse[1], material.diffuse[2]);
            let specular = vec3(material.specular[0], material.specular[1], material.specular[2]);
            let roughness = (material.shininess.log10() / 1000.0).max(0.0).min(1.0);
            let opacity = 1.0 - material.dissolve;

            let mat = Material::new(color, roughness, specular, opacity);
            material_indices[i] = mat_manager.push(mat);
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

                let pos = [
                    mesh.positions[i0],
                    mesh.positions[i1],
                    mesh.positions[i2],
                ];

                let normal = if !mesh.normals.is_empty() {
                    flags.set_flag(ObjFlags::HasNormals);
                    [mesh.normals[i0], mesh.normals[i1], mesh.normals[i2]]
                } else { [0.0; 3] };

                let uv = if !mesh.texcoords.is_empty() {
                    flags.set_flag(ObjFlags::HasUvs);
                    [mesh.texcoords[idx * 2], mesh.texcoords[idx * 2 + 1]]
                } else { [0.0; 2] };

                vertices.push(pos.into());
                normals.push(normal.into());
                uvs.push(uv.into());

                if i % 3 == 0 {
                    let material_id = if mesh.material_id.is_some() { material_indices[mesh.material_id.unwrap()] } else { mat_manager.get_default() };
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
        })
    }
}

impl ToMesh for Obj {
    fn into_rt_mesh(self) -> RTMesh {
        RTMesh::new(
            self.vertices.as_slice(),
            self.normals.as_slice(),
            self.uvs.as_slice(),
            self.material_ids.as_slice(),
        )
    }

    fn into_mesh(self) -> RastMesh {
        RastMesh::new(
            self.vertices.as_slice(),
            self.normals.as_slice(),
            self.uvs.as_slice(),
            self.material_ids.as_slice(),
        )
    }
}