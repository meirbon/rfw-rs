use l3d::load::MeshDescriptor;
use rayon::prelude::*;
use rfw_backend::{
    JointData, Mesh3dFlags, RTTriangle, SkinData, SkinnedMesh3D, SkinnedTriangles3D, Vertex3D,
    VertexMesh,
};
use rfw_math::*;

mod plane;
mod quad;
mod sphere;

pub use plane::*;
pub use quad::*;
use rtbvh::{Aabb, Bounds};
pub use sphere::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
pub trait SerializableObject<'a, T: Serialize + Deserialize<'a>> {
    fn serialize_object<S: AsRef<std::path::Path>>(
        &self,
        path: S,
        materials: &crate::MaterialList,
    ) -> Result<(), Box<dyn std::error::Error>>;
    fn deserialize_object<S: AsRef<std::path::Path>>(
        path: S,
        materials: &mut crate::MaterialList,
    ) -> Result<T, Box<dyn std::error::Error>>;
}

pub trait ToMesh3D {
    fn into_mesh_3d(self) -> Mesh3D;
}

impl ToMesh3D for Mesh3D {
    fn into_mesh_3d(self) -> Mesh3D {
        self
    }
}

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "serde")]
use std::collections::HashMap;

use crate::Materials;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Mesh3D {
    pub triangles: Vec<RTTriangle>,
    pub vertices: Vec<Vertex3D>,
    pub skin_data: Vec<JointData>,
    pub materials: Vec<u32>,
    pub ranges: Vec<VertexMesh>,
    pub bounds: Aabb,
    pub flags: Mesh3dFlags,
    pub name: String,
}

impl std::fmt::Display for Mesh3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mesh {{ triangles: {}, vertices: {}, joint_weights: {}, materials: {}, meshes: {}, bounds: {}, name: {} }}",
            self.triangles.len(),
            self.vertices.len(),
            self.skin_data.len(),
            self.materials.len(),
            self.ranges.len(),
            self.bounds,
            self.name.as_str()
        )
    }
}

impl Default for Mesh3D {
    fn default() -> Self {
        Mesh3D::empty()
    }
}

impl Mesh3D {
    pub fn new_indexed(
        indices: Vec<[u32; 3]>,
        original_vertices: Vec<Vec3>,
        original_normals: Vec<Vec3>,
        original_joints: Vec<Vec<[u16; 4]>>,
        original_weights: Vec<Vec<[f32; 4]>>,
        original_uvs: Vec<Vec2>,
        material_ids: Vec<u32>,
        flags: Mesh3dFlags,
        name: Option<String>,
    ) -> Mesh3D {
        let mut vertices = Vec::with_capacity(indices.len() * 3);
        let mut normals = Vec::with_capacity(indices.len() * 3);
        let mut uvs = Vec::with_capacity(indices.len() * 3);
        let mut material_indices = Vec::with_capacity(indices.len());
        let mut joints = Vec::with_capacity(original_joints.len());
        for v in original_joints.iter() {
            joints.push(Vec::with_capacity(v.len()));
        }

        let mut weights = Vec::with_capacity(original_weights.len());
        for v in original_weights.iter() {
            weights.push(Vec::with_capacity(v.len()));
        }

        indices.into_iter().enumerate().for_each(|(j, i)| {
            let i0 = i[0] as usize;
            let i1 = i[1] as usize;
            let i2 = i[2] as usize;

            vertices.push(original_vertices[i0]);
            vertices.push(original_vertices[i1]);
            vertices.push(original_vertices[i2]);

            normals.push(original_normals[i0]);
            normals.push(original_normals[i1]);
            normals.push(original_normals[i2]);

            uvs.push(original_uvs[i0]);
            uvs.push(original_uvs[i1]);
            uvs.push(original_uvs[i2]);

            joints.iter_mut().enumerate().for_each(|(i, v)| {
                v.push(original_joints[i][i0]);
                v.push(original_joints[i][i1]);
                v.push(original_joints[i][i2]);
            });

            weights.iter_mut().enumerate().for_each(|(i, v)| {
                v.push(original_weights[i][i0]);
                v.push(original_weights[i][i1]);
                v.push(original_weights[i][i2]);
            });

            material_indices.push(material_ids[j]);
        });

        debug_assert_eq!(vertices.len(), normals.len());
        debug_assert_eq!(vertices.len(), uvs.len());
        debug_assert_eq!(uvs.len(), material_ids.len() * 3);
        debug_assert_eq!(vertices.len() % 3, 0);

        Mesh3D::new(
            vertices,
            normals,
            joints,
            weights,
            uvs,
            material_indices,
            flags,
            name,
        )
    }

    pub fn new<T: AsRef<str>>(
        vertices: Vec<Vec3>,
        normals: Vec<Vec3>,
        joints: Vec<Vec<[u16; 4]>>,
        weights: Vec<Vec<[f32; 4]>>,
        uvs: Vec<Vec2>,
        material_ids: Vec<u32>,
        flags: Mesh3dFlags,
        name: Option<T>,
    ) -> Mesh3D {
        debug_assert_eq!(vertices.len(), normals.len());
        debug_assert_eq!(vertices.len(), uvs.len());
        debug_assert_eq!(uvs.len(), material_ids.len() * 3);
        debug_assert_eq!(vertices.len() % 3, 0);

        let mut bounds = Aabb::new();
        let mut vertex_data = vec![Vertex3D::default(); vertices.len()];

        let normals: Vec<Vec3> = if normals[0].cmpeq(Vec3::ZERO).all() {
            let mut normals = vec![Vec3::ZERO; vertices.len()];
            for i in (0..vertices.len()).step_by(3) {
                let v0 = vertices[i];
                let v1 = vertices[i + 1];
                let v2 = vertices[i + 2];

                let e1 = v1 - v0;
                let e2 = v2 - v0;

                let normal = e1.cross(e2).normalize();

                let a = (v1 - v0).length();
                let b = (v2 - v1).length();
                let c = (v0 - v2).length();
                let s = (a + b + c) * 0.5;
                let area = (s * (s - a) * (s - b) * (s - c)).sqrt();
                let normal = normal * area;

                normals[i] += normal;
                normals[i + 1] += normal;
                normals[i + 2] += normal;
            }

            normals.par_iter_mut().for_each(|n| *n = n.normalize());
            normals
        } else {
            normals
        };

        let mut tangents: Vec<Vec4> = vec![Vec4::ZERO; vertices.len()];
        let mut bitangents: Vec<Vec3> = vec![Vec3::ZERO; vertices.len()];

        for i in (0..vertices.len()).step_by(3) {
            let v0: Vec3 = vertices[i];
            let v1: Vec3 = vertices[i + 1];
            let v2: Vec3 = vertices[i + 2];

            bounds.grow(v0);
            bounds.grow(v1);
            bounds.grow(v2);

            let e1: Vec3 = v1 - v0;
            let e2: Vec3 = v2 - v0;

            let tex0: Vec2 = uvs[i];
            let tex1: Vec2 = uvs[i + 1];
            let tex2: Vec2 = uvs[i + 2];

            let uv1: Vec2 = tex1 - tex0;
            let uv2: Vec2 = tex2 - tex0;

            let n = e1.cross(e2).normalize();

            let (t, b) = if uv1.dot(uv1) == 0.0 || uv2.dot(uv2) == 0.0 {
                let tangent: Vec3 = e1.normalize();
                let bitangent: Vec3 = n.cross(tangent).normalize();
                (tangent.extend(0.0), bitangent)
            } else {
                let r = 1.0 / (uv1.x * uv2.y - uv1.y * uv2.x);
                let tangent: Vec3 = (e1 * uv2.y - e2 * uv1.y) * r;
                let bitangent: Vec3 = (e1 * uv2.x - e2 * uv1.x) * r;
                (tangent.extend(0.0), bitangent)
            };

            tangents[i] += t;
            tangents[i + 1] += t;
            tangents[i + 2] += t;

            bitangents[i] += b;
            bitangents[i + 1] += b;
            bitangents[i + 2] += b;
        }

        let bounds = bounds;

        for i in 0..vertices.len() {
            let n: Vec3 = normals[i];
            let tangent = tangents[i].truncate().normalize();
            let bitangent = bitangents[i].normalize();

            let t: Vec3 = (tangent - (n * n.dot(tangent))).normalize();
            let c: Vec3 = n.cross(t);

            let w = c.dot(bitangent).signum();
            tangents[i] = tangent.normalize().extend(w);
        }

        vertex_data.par_iter_mut().enumerate().for_each(|(i, v)| {
            let vertex = vertices[i];
            let vertex = Vec4::new(vertex.x, vertex.y, vertex.z, 1.0);
            let normal = normals[i];

            *v = Vertex3D {
                vertex,
                normal,
                mat_id: material_ids[i / 3],
                uv: uvs[i],
                tangent: tangents[i],
                ..Default::default()
            };
        });

        let mut last_id = material_ids[0];
        let mut start = 0;
        let mut range = 0;
        let mut meshes: Vec<VertexMesh> = Vec::new();
        let mut v_bounds = Aabb::new();

        for i in 0..material_ids.len() {
            range += 1;
            for j in 0..3 {
                v_bounds.grow(vertices[i * 3 + j]);
            }

            if last_id != material_ids[i] {
                meshes.push(VertexMesh {
                    first: start * 3,
                    last: (start + range) * 3,
                    mat_id: last_id,
                    bounds: v_bounds,
                    padding: 0,
                });

                v_bounds = Aabb::new();
                last_id = material_ids[i];
                start = i as u32;
                range = 1;
            }
        }

        if meshes.is_empty() {
            // There only is 1 mesh available
            meshes.push(VertexMesh {
                first: 0,
                last: vertices.len() as u32,
                mat_id: material_ids[0],
                bounds,
                padding: 0,
            });
        } else if (start + range) != (material_ids.len() as u32 - 1) {
            // Add last mesh to list
            meshes.push(VertexMesh {
                first: start * 3,
                last: (start + range) * 3,
                mat_id: last_id,
                bounds: v_bounds,
                padding: 0,
            })
        }

        let mut triangles = vec![RTTriangle::default(); vertices.len() / 3];
        triangles.iter_mut().enumerate().for_each(|(i, triangle)| {
            let i0 = i * 3;
            let i1 = i0 + 1;
            let i2 = i0 + 2;

            let vertex0 = vertices[i0];
            let vertex1 = vertices[i1];
            let vertex2 = vertices[i2];

            let n0 = normals[i0];
            let n1 = normals[i1];
            let n2 = normals[i2];

            let uv0 = uvs[i0];
            let uv1 = uvs[i1];
            let uv2 = uvs[i2];

            let tangent0 = tangents[i0];
            let tangent1 = tangents[i1];
            let tangent2 = tangents[i1];

            let normal = RTTriangle::normal(vertex0, vertex1, vertex2);

            let ta = (1024 * 1024) as f32
                * ((uv1.x - uv0.x) * (uv2.y - uv0.y) - (uv2.x - uv0.x) * (uv1.y - uv0.y)).abs();
            let pa = (vertex1 - vertex0).cross(vertex2 - vertex0).length();
            let lod = 0.0_f32.max((0.5 * (ta / pa).log2()).sqrt());

            *triangle = RTTriangle {
                vertex0,
                u0: uv0.x,
                vertex1,
                u1: uv1.x,
                vertex2,
                u2: uv2.x,
                normal,
                v0: uv0.y,
                n0,
                v1: uv1.y,
                n1,
                v2: uv2.y,
                n2,
                id: i as i32,
                tangent0,
                tangent1,
                tangent2,
                light_id: -1,
                mat_id: material_ids[i] as i32,
                lod,
                area: RTTriangle::area(vertex0, vertex1, vertex2),
            };
        });

        let mut joints_weights = vec![JointData::default(); joints.len()];
        joints_weights.iter_mut().enumerate().for_each(|(i, v)| {
            let joints = if let Some(j) = joints.get(0) {
                *j.get(i).unwrap_or(&[0; 4])
            } else {
                [0; 4]
            };

            let mut weights = if let Some(w) = weights.get(0) {
                *w.get(i).unwrap_or(&[0.0; 4])
            } else {
                [0.25; 4]
            };

            // Ensure weights sum up to 1.0
            let total = weights[0] + weights[1] + weights[2] + weights[3];
            weights.iter_mut().for_each(|w| *w /= total);

            *v = JointData::from((joints, weights));
        });

        Mesh3D {
            triangles,
            vertices: vertex_data,
            materials: material_ids,
            skin_data: joints_weights,
            ranges: meshes,
            bounds,
            flags,
            name: if let Some(name) = name {
                String::from(name.as_ref())
            } else {
                String::new()
            },
        }
    }

    pub fn scale(&self, scaling: f32) -> Self {
        let mut new_self = self.clone();

        let scaling = Mat4::from_scale(Vec3::splat(scaling));
        new_self.triangles.par_iter_mut().for_each(|t| {
            let vertex0 = scaling * Vec4::new(t.vertex0[0], t.vertex0[1], t.vertex0[2], 1.0);
            let vertex1 = scaling * Vec4::new(t.vertex1[0], t.vertex1[1], t.vertex1[2], 1.0);
            let vertex2 = scaling * Vec4::new(t.vertex2[0], t.vertex2[1], t.vertex2[2], 1.0);

            t.vertex0 = vertex0.truncate();
            t.vertex1 = vertex1.truncate();
            t.vertex2 = vertex2.truncate();
        });

        new_self.vertices.iter_mut().for_each(|v| {
            v.vertex = scaling * Vec4::new(v.vertex[0], v.vertex[1], v.vertex[2], 1.0);
        });

        new_self
    }

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    pub fn empty() -> Mesh3D {
        Mesh3D {
            triangles: Default::default(),
            vertices: Default::default(),
            skin_data: Default::default(),
            materials: Default::default(),
            ranges: Default::default(),
            bounds: Aabb::new(),
            flags: Mesh3dFlags::all(),
            name: String::new(),
        }
    }

    pub fn buffer_size(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<Vertex3D>()
    }

    pub fn as_slice(&self) -> &[Vertex3D] {
        self.vertices.as_slice()
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.vertices.as_ptr() as *const u8, self.buffer_size())
        }
    }

    pub fn apply_skin(&self, skin: &SkinData) -> SkinnedMesh3D {
        SkinnedMesh3D::apply(
            self.vertices.as_slice(),
            self.skin_data.as_slice(),
            self.ranges.as_slice(),
            skin.joint_matrices,
        )
    }

    pub fn apply_skin_triangles(&self, skin: &SkinData) -> SkinnedTriangles3D {
        SkinnedTriangles3D::apply(
            self.triangles.as_slice(),
            self.skin_data.as_slice(),
            skin.joint_matrices,
        )
    }

    pub fn with_flags(mut self, flags: Mesh3dFlags) -> Self {
        self.flags |= flags;
        self
    }

    pub fn without_flags(mut self, flags: Mesh3dFlags) -> Self {
        self.flags.remove(flags);
        self
    }
}

impl Bounds for Mesh3D {
    fn bounds(&self) -> Aabb {
        self.bounds
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
struct SerializedMesh {
    pub mesh: Mesh3D,
    pub materials: Materials,
}

#[cfg(feature = "serde")]
impl<'a> crate::SerializableObject<'a, Mesh3D> for Mesh3D {
    fn serialize_object<S: AsRef<std::path::Path>>(
        &self,
        path: S,
        materials: &MaterialList,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create a new local material list
        let mut local_mat_list = MaterialList::empty();

        // Gather all material indices
        use std::collections::BTreeSet;
        let mut material_indices: BTreeSet<u32> = BTreeSet::new();
        self.ranges.iter().for_each(|mesh| {
            material_indices.insert(mesh.mat_id);
        });
        let material_indices: Vec<u32> = material_indices.iter().map(|i| *i).collect();

        // Initialize mappings for materials and textures to local space
        let mut material_mapping: HashMap<u32, u32> = HashMap::new();
        let mut texture_mapping: HashMap<i16, i16> = HashMap::new();
        texture_mapping.insert(-1, -1);

        // Iterate over all materials
        material_indices.iter().for_each(|index| {
            // Get original material
            let material = &materials[*index as usize];

            let mut add_texture_index = |index: usize| {
                // Add texture to index if necessary
                if texture_mapping.get(&(index as i16)).is_none() {
                    // Push texture to material list
                    texture_mapping.insert(
                        index as i16,
                        local_mat_list.push_texture(materials.get_texture(index).unwrap().clone())
                            as i16,
                    );
                }
            };

            if material.diffuse_tex >= 0 {
                add_texture_index(material.diffuse_tex as usize);
            }

            if material.normal_tex >= 0 {
                add_texture_index(material.normal_tex as usize);
            }
        });

        for index in material_indices.iter() {
            let index = *index as usize;
            let mut material = materials[index].clone();

            let d_key = texture_mapping.get(&material.diffuse_tex);
            let n_key = texture_mapping.get(&material.normal_tex);

            assert!(
                d_key.is_some(),
                "diffuse texture {} was not in mapping",
                material.diffuse_tex
            );
            assert!(
                n_key.is_some(),
                "normal texture {} was not in mapping",
                material.normal_tex
            );

            material.diffuse_tex = *d_key.unwrap();
            material.normal_tex = *n_key.unwrap();

            material_mapping.insert(index as u32, local_mat_list.push(material) as u32);
        }

        assert_eq!(material_indices.len(), local_mat_list.len());

        // Create a clone of original mesh so that we can overwrite its material indices
        let mut mesh = self.clone();
        mesh.materials.par_iter_mut().for_each(|id| {
            *id = *material_mapping
                .get(id)
                .expect(format!("Mat with id {} does not exist", id).as_str())
        });
        mesh.ranges
            .par_iter_mut()
            .for_each(|m| m.mat_id = *material_mapping.get(&m.mat_id).unwrap());

        let serialized_mesh = SerializedMesh {
            mesh,
            materials: local_mat_list,
        };

        let mut file = std::fs::File::create(path)?;
        use std::io::Write;
        let encoded: Vec<u8> = bincode::serialize(&serialized_mesh)?;
        file.write_all(encoded.as_ref())?;
        Ok(())
    }

    fn deserialize_object<S: AsRef<std::path::Path>>(
        path: S,
        materials: &mut MaterialList,
    ) -> Result<Mesh3D, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let object: SerializedMesh = bincode::deserialize_from(reader)?;

        // Initialize mapping
        let mut material_mapping: HashMap<u32, u32> = HashMap::new();
        let mut texture_mapping: HashMap<i16, i16> = HashMap::new();
        texture_mapping.insert(-1, -1);

        let serialized_list = object.materials;
        let mut mesh = object.mesh;

        // Add all textures from serialized mesh
        for i in 0..serialized_list.len_textures() {
            let texture = serialized_list.get_texture(i).unwrap();
            let new_index = materials.push_texture(texture.clone());

            // Add to mapping
            texture_mapping.insert(i as i16, new_index as i16);
        }

        for i in 0..serialized_list.len() {
            let mut material = serialized_list.get(i).unwrap().clone();

            // Overwrite diffuse texture index
            if material.diffuse_tex >= 0 {
                let key = *texture_mapping.get(&material.diffuse_tex).unwrap();
                material.diffuse_tex = key;
            }

            // Overwrite normal texture index
            if material.normal_tex >= 0 {
                let key = *texture_mapping.get(&material.normal_tex).unwrap();
                material.normal_tex = key;
            }

            // Add new index to mapping
            let new_index = materials.push(material);
            material_mapping.insert(i as u32, new_index as u32);
        }

        mesh.materials.par_iter_mut().for_each(|m| {
            *m = *material_mapping.get(m).unwrap() as u32;
        });
        mesh.ranges.par_iter_mut().for_each(|m| {
            m.mat_id = *material_mapping.get(&m.mat_id).unwrap() as u32;
        });

        Ok(mesh)
    }
}

impl From<MeshDescriptor> for Mesh3D {
    fn from(desc: MeshDescriptor) -> Self {
        let mut bounds = Aabb::new();
        let mut vertex_data = vec![Vertex3D::default(); desc.vertices.len()];

        let material_ids: Vec<u32> = desc.material_ids.chunks(3).map(|c| c[0] as u32).collect();

        let normals: Vec<Vec3> = if Vec3::from(desc.normals[0]).cmpeq(Vec3::ZERO).all() {
            let mut normals = vec![Vec3::ZERO; desc.vertices.len()];
            for i in (0..desc.vertices.len()).step_by(3) {
                let v0: Vec3 = Vec4::from(desc.vertices[i]).truncate();
                let v1: Vec3 = Vec4::from(desc.vertices[i + 1]).truncate();
                let v2: Vec3 = Vec4::from(desc.vertices[i + 2]).truncate();

                let e1 = v1 - v0;
                let e2 = v2 - v0;

                let normal = e1.cross(e2).normalize();

                let a = (v1 - v0).length();
                let b = (v2 - v1).length();
                let c = (v0 - v2).length();
                let s = (a + b + c) * 0.5;
                let area = (s * (s - a) * (s - b) * (s - c)).sqrt();
                let normal = normal * area;

                normals[i] += normal;
                normals[i + 1] += normal;
                normals[i + 2] += normal;
            }

            normals.par_iter_mut().for_each(|n| *n = n.normalize());
            normals
        } else {
            desc.normals
                .iter()
                .map(|n| Vec3::from(*n))
                .collect::<Vec<Vec3>>()
        };

        for i in (0..desc.vertices.len()).step_by(3) {
            let v0: Vec3 = Vec4::from(desc.vertices[i]).truncate();
            let v1: Vec3 = Vec4::from(desc.vertices[i + 1]).truncate();
            let v2: Vec3 = Vec4::from(desc.vertices[i + 2]).truncate();

            bounds.grow(v0);
            bounds.grow(v1);
            bounds.grow(v2);
        }

        vertex_data.par_iter_mut().enumerate().for_each(|(i, v)| {
            *v = Vertex3D {
                vertex: Vec4::from(desc.vertices[i]),
                normal: normals[i],
                mat_id: material_ids[i / 3],
                uv: Vec2::from(desc.uvs[i]),
                tangent: Vec4::from(desc.tangents[i]),
                ..Default::default()
            };
        });

        let mut last_id = material_ids[0];
        let mut start = 0;
        let mut range = 0;
        let mut meshes: Vec<VertexMesh> = Vec::new();
        let mut v_bounds = Aabb::new();

        (0..material_ids.len()).into_iter().for_each(|i| {
            range += 1;
            for j in 0..3 {
                v_bounds.grow(vec3(
                    desc.vertices[i * 3 + j][0],
                    desc.vertices[i * 3 + j][1],
                    desc.vertices[i * 3 + j][2],
                ));
            }

            if last_id != material_ids[i] {
                meshes.push(VertexMesh {
                    first: start * 3,
                    last: (start + range) * 3,
                    mat_id: last_id as _,
                    bounds: v_bounds,
                    padding: 0,
                });

                v_bounds = Aabb::new();
                last_id = material_ids[i];
                start = i as u32;
                range = 1;
            }
        });

        if meshes.is_empty() {
            // There only is 1 mesh available
            meshes.push(VertexMesh {
                first: 0,
                last: desc.vertices.len() as u32,
                mat_id: material_ids[0],
                bounds,
                padding: 0,
            });
        } else if (start + range) != (material_ids.len() as u32 - 1) {
            // Add last mesh to list
            meshes.push(VertexMesh {
                first: start * 3,
                last: (start + range) * 3,
                mat_id: last_id,
                bounds: v_bounds,
                padding: 0,
            })
        }

        let mut triangles = vec![RTTriangle::default(); desc.vertices.len() / 3];
        triangles.iter_mut().enumerate().for_each(|(i, triangle)| {
            let i0 = i * 3;
            let i1 = i0 + 1;
            let i2 = i0 + 2;

            let vertex0 = Vec3::new(
                desc.vertices[i0][0],
                desc.vertices[i0][1],
                desc.vertices[i0][2],
            );
            let vertex1 = Vec3::new(
                desc.vertices[i1][0],
                desc.vertices[i1][1],
                desc.vertices[i1][2],
            );
            let vertex2 = Vec3::new(
                desc.vertices[i2][0],
                desc.vertices[i2][1],
                desc.vertices[i2][2],
            );

            let n0 = normals[i0];
            let n1 = normals[i1];
            let n2 = normals[i2];

            let uv0 = Vec2::from(desc.uvs[i0]);
            let uv1 = Vec2::from(desc.uvs[i1]);
            let uv2 = Vec2::from(desc.uvs[i2]);

            let tangent0 = Vec4::from(desc.tangents[i0]);
            let tangent1 = Vec4::from(desc.tangents[i1]);
            let tangent2 = Vec4::from(desc.tangents[i1]);

            let normal = RTTriangle::normal(vertex0, vertex1, vertex2);

            let ta = (1024 * 1024) as f32
                * ((uv1.x - uv0.x) * (uv2.y - uv0.y) - (uv2.x - uv0.x) * (uv1.y - uv0.y)).abs();
            let pa = (vertex1 - vertex0).cross(vertex2 - vertex0).length();
            let lod = 0.0_f32.max((0.5 * (ta / pa).log2()).sqrt());

            *triangle = RTTriangle {
                vertex0,
                u0: uv0.x,
                vertex1,
                u1: uv1.x,
                vertex2,
                u2: uv2.x,
                normal,
                v0: uv0.y,
                n0,
                v1: uv1.y,
                n1,
                v2: uv2.y,
                n2,
                id: i as i32,
                tangent0,
                tangent1,
                tangent2,
                light_id: -1,
                mat_id: material_ids[i] as i32,
                lod,
                area: RTTriangle::area(vertex0, vertex1, vertex2),
            };
        });

        let (joints, weights) = if let Some(s) = desc.skeleton {
            (s.joints, s.weights)
        } else {
            (Vec::new(), Vec::new())
        };

        let joints_weights = if !joints.is_empty() && !weights.is_empty() {
            let mut joints_weights = vec![JointData::default(); vertex_data.len()];
            joints_weights.iter_mut().enumerate().for_each(|(i, v)| {
                let joints = if let Some(j) = joints.get(0) {
                    *j.get(i).unwrap_or(&[0; 4])
                } else {
                    [0; 4]
                };

                let mut weights = if let Some(w) = weights.get(0) {
                    *w.get(i).unwrap_or(&[0.0; 4])
                } else {
                    [0.25; 4]
                };

                // Ensure weights sum up to 1.0
                let total = weights[0] + weights[1] + weights[2] + weights[3];
                weights.iter_mut().for_each(|w| *w /= total);

                *v = JointData::from((joints, weights));
            });
            joints_weights
        } else {
            Vec::new()
        };

        Mesh3D {
            triangles,
            vertices: vertex_data,
            skin_data: joints_weights,
            materials: material_ids,
            ranges: meshes,
            bounds,
            flags: Mesh3dFlags::default(),
            name: desc.name,
        }
    }
}
