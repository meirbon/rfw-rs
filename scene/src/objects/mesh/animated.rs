use glam::*;
use rayon::prelude::*;

use crate::objects::mesh::*;
use crate::PrimID;
use crate::{HitRecord, HitRecord4, Intersect, MaterialList, RTTriangle};
use rtbvh::{Bounds, Ray, RayPacket4, AABB, BVH, MBVH};
use std::fmt::Display;

#[cfg(feature = "object_caching")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "object_caching")]
use std::collections::HashMap;

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
pub struct AnimVertexData {
    pub joints: [u32; 4],
    pub weights: [f32; 4],
}

impl AnimVertexData {
    pub fn zero() -> Self {
        Self {
            joints: [0; 4],
            weights: [0.0; 4],
        }
    }
}

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct AnimatedMesh {
    pub triangles: Vec<RTTriangle>,
    pub vertices: Vec<VertexData>,
    pub anim_vertex_data: Vec<AnimVertexData>,
    pub joints: Vec<Vec<[u16; 4]>>,
    pub weights: Vec<Vec<Vec4>>,
    pub materials: Vec<u32>,
    pub meshes: Vec<VertexMesh>,
    pub bounds: AABB,
    pub bvh: Option<BVH>,
    pub mbvh: Option<MBVH>,
    pub name: String,
}

impl Display for AnimatedMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mesh {{ triangles: {}, vertices: {}, materials: {}, meshes: {}, bounds: {}, bvh: {}, mbvh: {}, name: {} }}",
            self.triangles.len(),
            self.vertices.len(),
            self.materials.len(),
            self.meshes.len(),
            self.bounds,
            self.bvh.is_some(),
            self.mbvh.is_some(),
            self.name.as_str()
        )
    }
}

impl Default for AnimatedMesh {
    fn default() -> Self {
        AnimatedMesh::empty()
    }
}

impl AnimatedMesh {
    pub fn new_indexed(
        indices: Vec<[u32; 3]>,
        original_vertices: Vec<Vec3A>,
        original_normals: Vec<Vec3A>,
        original_joints: Vec<Vec<[u16; 4]>>,
        original_weights: Vec<Vec<Vec4>>,
        original_uvs: Vec<Vec2>,
        material_ids: Vec<u32>,
        name: Option<String>,
    ) -> Self {
        assert_eq!(original_vertices.len(), original_normals.len());
        for vec in original_joints.iter() {
            assert_eq!(original_normals.len(), vec.len());
        }

        for vec in original_weights.iter() {
            assert_eq!(original_normals.len(), vec.len());
        }
        assert_eq!(original_normals.len(), original_uvs.len());
        assert_eq!(material_ids.len(), indices.len());

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

        Self::new(
            vertices,
            normals,
            joints,
            weights,
            uvs,
            material_indices,
            name,
        )
    }

    pub fn new<T: AsRef<str>>(
        vertices: Vec<Vec3A>,
        normals: Vec<Vec3A>,
        joints: Vec<Vec<[u16; 4]>>,
        weights: Vec<Vec<Vec4>>,
        uvs: Vec<Vec2>,
        material_ids: Vec<u32>,
        name: Option<T>,
    ) -> Self {
        debug_assert_eq!(vertices.len(), normals.len());
        debug_assert_eq!(vertices.len(), uvs.len());
        debug_assert_eq!(uvs.len(), material_ids.len() * 3);
        debug_assert_eq!(vertices.len() % 3, 0);

        let mut bounds = AABB::new();
        let mut vertex_data = vec![VertexData::zero(); vertices.len()];
        let mut anim_vertex_data = vec![AnimVertexData::zero(); vertices.len()];

        let normals: Vec<Vec3A> = if normals[0].cmpeq(Vec3A::zero()).all() {
            let mut normals = vec![Vec3A::zero(); vertices.len()];
            for i in (0..vertices.len()).step_by(3) {
                let v0 = vertices[i + 0];
                let v1 = vertices[i + 1];
                let v2 = vertices[i + 2];

                let e1 = v1 - v0;
                let e2 = v2 - v0;

                let n = e1.cross(e2).normalize();

                let a = (v1 - v0).length();
                let b = (v2 - v1).length();
                let c = (v0 - v2).length();
                let s = (a + b + c) * 0.5;
                let area = (s * (s - a) * (s - b) * (s - c)).sqrt();
                let n = n * area;

                normals[i + 0] += n;
                normals[i + 1] += n;
                normals[i + 2] += n;
            }

            normals.par_iter_mut().for_each(|n| *n = n.normalize());
            normals
        } else {
            Vec::from(normals)
        };

        let mut tangents: Vec<Vec4> = vec![Vec4::zero(); vertices.len()];
        let mut bitangents: Vec<Vec3A> = vec![Vec3A::zero(); vertices.len()];

        for i in (0..vertices.len()).step_by(3) {
            let v0: Vec3A = vertices[i];
            let v1: Vec3A = vertices[i + 1];
            let v2: Vec3A = vertices[i + 2];

            bounds.grow(v0);
            bounds.grow(v1);
            bounds.grow(v2);

            let e1: Vec3A = v1 - v0;
            let e2: Vec3A = v2 - v0;

            let tex0: Vec2 = uvs[i];
            let tex1: Vec2 = uvs[i + 1];
            let tex2: Vec2 = uvs[i + 2];

            let uv1: Vec2 = tex1 - tex0;
            let uv2: Vec2 = tex2 - tex0;

            let n = e1.cross(e2).normalize();

            let (t, b) = if uv1.dot(uv1) == 0.0 || uv2.dot(uv2) == 0.0 {
                let tangent: Vec3A = e1.normalize();
                let bitangent: Vec3A = n.cross(tangent).normalize();
                (tangent.extend(0.0), bitangent)
            } else {
                let r = 1.0 / (uv1.x() * uv2.y() - uv1.y() * uv2.x());
                let tangent: Vec3A = (e1 * uv2.y() - e2 * uv1.y()) * r;
                let bitangent: Vec3A = (e1 * uv2.x() - e2 * uv1.x()) * r;
                (tangent.extend(0.0), bitangent)
            };

            tangents[i + 0] += t;
            tangents[i + 1] += t;
            tangents[i + 2] += t;

            bitangents[i + 0] += b;
            bitangents[i + 1] += b;
            bitangents[i + 2] += b;
        }

        let bounds = bounds;

        for i in 0..vertices.len() {
            let n: Vec3A = normals[i];
            let tangent = tangents[i].truncate().normalize();
            let bitangent = bitangents[i].normalize();

            let t: Vec3A = (tangent - (n * n.dot(tangent))).normalize();
            let c: Vec3A = n.cross(t);

            let w = c.dot(bitangent).signum();
            tangents[i] = tangent.normalize().extend(w);
        }

        vertex_data
            .iter_mut()
            .enumerate()
            .zip(anim_vertex_data.iter_mut())
            .par_bridge()
            .for_each(|((i, v), anim_v)| {
                let vertex: [f32; 3] = vertices[i].into();
                let vertex = [vertex[0], vertex[1], vertex[2], 1.0];
                let normal = normals[i].into();

                *v = VertexData {
                    vertex,
                    normal,
                    mat_id: material_ids[i / 3],
                    uv: uvs[i].into(),
                    tangent: tangents[i].into(),
                };

                let joints: [u32; 4] = if let Some(j) = joints.get(0) {
                    if let Some(joints) = j.get(i) {
                        [
                            joints[0] as u32,
                            joints[1] as u32,
                            joints[2] as u32,
                            joints[3] as u32,
                        ]
                    } else {
                        [0; 4]
                    }
                } else {
                    [0; 4]
                };
                let mut weights: [f32; 4] = if let Some(w) = weights.get(0) {
                    if let Some(weights) = w.get(i) {
                        *weights.as_ref()
                    } else {
                        [0.0; 4]
                    }
                } else {
                    [0.25; 4]
                };

                // Ensure weights sum up to 1.0
                let total = weights[0] + weights[1] + weights[2] + weights[3];
                for i in 0..4 {
                    weights[i] = weights[i] / total;
                }

                *anim_v = AnimVertexData { joints, weights };
            });

        let mut last_id = material_ids[0];
        let mut start = 0;
        let mut range = 0;
        let mut meshes: Vec<VertexMesh> = Vec::new();
        let mut v_bounds = AABB::new();

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
                    bounds: v_bounds.clone(),
                });

                v_bounds = AABB::new();
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
                bounds: bounds.clone(),
            });
        } else if (start + range) != (material_ids.len() as u32 - 1) {
            // Add last mesh to list
            meshes.push(VertexMesh {
                first: start * 3,
                last: (start + range) * 3,
                mat_id: last_id,
                bounds: v_bounds,
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

            let tangent0: Vec4 = tangents[i0];
            let tangent1: Vec4 = tangents[i1];
            let tangent2: Vec4 = tangents[i1];

            let normal = RTTriangle::normal(vertex0, vertex1, vertex2);

            let ta = (1024 * 1024) as f32
                * ((uv1.x() - uv0.x()) * (uv2.y() - uv0.y())
                    - (uv2.x() - uv0.x()) * (uv1.y() - uv0.y()))
                .abs();
            let pa: f32 = (vertex1 - vertex0).cross(vertex2 - vertex0).length();
            let lod = 0.0_f32.max((0.5 * (ta / pa).log2()).sqrt());

            *triangle = RTTriangle {
                vertex0: vertex0.into(),
                u0: uv0.x(),
                vertex1: vertex1.into(),
                u1: uv1.x(),
                vertex2: vertex2.into(),
                u2: uv2.x(),
                normal: normal.into(),
                v0: uv0.y(),
                n0: n0.into(),
                v1: uv1.y(),
                n1: n1.into(),
                v2: uv2.y(),
                n2: n2.into(),
                id: i as i32,
                tangent0: tangent0.into(),
                tangent1: tangent1.into(),
                tangent2: tangent2.into(),
                light_id: -1,
                mat_id: material_ids[i] as i32,
                lod,
                area: RTTriangle::area(vertex0, vertex1, vertex2),
            };
        });

        Self {
            triangles,
            vertices: vertex_data,
            anim_vertex_data,
            joints,
            weights,
            materials: Vec::from(material_ids),
            meshes,
            bounds,
            bvh: None,
            mbvh: None,
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

            t.vertex0 = vertex0.truncate().into();
            t.vertex1 = vertex1.truncate().into();
            t.vertex2 = vertex2.truncate().into();
        });

        new_self.vertices.iter_mut().for_each(|v| {
            v.vertex = (scaling * Vec4::new(v.vertex[0], v.vertex[1], v.vertex[2], 1.0)).into();
        });

        new_self
    }

    pub fn construct_bvh(&mut self) {
        let aabbs: Vec<AABB> = self.triangles.par_iter().map(|t| t.bounds()).collect();
        let centers: Vec<Vec3A> = self.triangles.par_iter().map(|t| t.center()).collect();

        self.bvh = Some(BVH::construct_spatial(
            aabbs.as_slice(),
            centers.as_slice(),
            self.triangles.as_slice(),
        ));
        self.mbvh = Some(MBVH::construct(self.bvh.as_ref().unwrap()));
    }

    pub fn refit_bvh(&mut self) {
        if let Some(bvh) = self.bvh.as_mut() {
            let aabbs: Vec<AABB> = self.triangles.par_iter().map(|t| t.bounds()).collect();
            bvh.refit(aabbs.as_slice());
        }
    }

    pub fn len(&self) -> usize {
        self.vertices.len()
    }

    pub fn empty() -> Self {
        AnimatedMesh {
            triangles: Vec::new(),
            vertices: Vec::new(),
            anim_vertex_data: Vec::new(),
            joints: Vec::new(),
            weights: Vec::new(),
            materials: Vec::new(),
            meshes: Vec::new(),
            bounds: AABB::new(),
            bvh: None,
            mbvh: None,
            name: String::new(),
        }
    }

    pub fn buffer_size(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<VertexData>()
    }

    pub fn anim_buffer_size(&self) -> usize {
        self.anim_vertex_data.len() * std::mem::size_of::<AnimVertexData>()
    }

    pub fn as_slice(&self) -> &[VertexData] {
        self.vertices.as_slice()
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.vertices.as_ptr() as *const u8, self.buffer_size())
        }
    }

    pub fn get_hit_record4(&self, ray: &RayPacket4, t: [f32; 4], hit_data: [u32; 4]) -> HitRecord4 {
        let (org_x, org_y, org_z) = ray.origin_xyz::<Vec4>();
        let (dir_x, dir_y, dir_z) = ray.direction_xyz::<Vec4>();
        let t = Vec4::from(t);

        let vertex0_x = Vec4::new(
            self.triangles[hit_data[0] as usize].vertex0[0],
            self.triangles[hit_data[1] as usize].vertex0[0],
            self.triangles[hit_data[2] as usize].vertex0[0],
            self.triangles[hit_data[3] as usize].vertex0[0],
        );
        let vertex0_y = Vec4::new(
            self.triangles[hit_data[0] as usize].vertex0[1],
            self.triangles[hit_data[1] as usize].vertex0[1],
            self.triangles[hit_data[2] as usize].vertex0[1],
            self.triangles[hit_data[3] as usize].vertex0[1],
        );
        let vertex0_z = Vec4::new(
            self.triangles[hit_data[0] as usize].vertex0[2],
            self.triangles[hit_data[1] as usize].vertex0[2],
            self.triangles[hit_data[2] as usize].vertex0[2],
            self.triangles[hit_data[3] as usize].vertex0[2],
        );
        let vertex1_x = Vec4::new(
            self.triangles[hit_data[0] as usize].vertex1[0],
            self.triangles[hit_data[1] as usize].vertex1[0],
            self.triangles[hit_data[2] as usize].vertex1[0],
            self.triangles[hit_data[3] as usize].vertex1[0],
        );
        let vertex1_y = Vec4::new(
            self.triangles[hit_data[0] as usize].vertex1[1],
            self.triangles[hit_data[1] as usize].vertex1[1],
            self.triangles[hit_data[2] as usize].vertex1[1],
            self.triangles[hit_data[3] as usize].vertex1[1],
        );
        let vertex1_z = Vec4::new(
            self.triangles[hit_data[0] as usize].vertex1[2],
            self.triangles[hit_data[1] as usize].vertex1[2],
            self.triangles[hit_data[2] as usize].vertex1[2],
            self.triangles[hit_data[3] as usize].vertex1[2],
        );
        let vertex2_x = Vec4::new(
            self.triangles[hit_data[0] as usize].vertex2[0],
            self.triangles[hit_data[1] as usize].vertex2[0],
            self.triangles[hit_data[2] as usize].vertex2[0],
            self.triangles[hit_data[3] as usize].vertex2[0],
        );
        let vertex2_y = Vec4::new(
            self.triangles[hit_data[0] as usize].vertex2[1],
            self.triangles[hit_data[1] as usize].vertex2[1],
            self.triangles[hit_data[2] as usize].vertex2[1],
            self.triangles[hit_data[3] as usize].vertex2[1],
        );
        let vertex2_z = Vec4::new(
            self.triangles[hit_data[0] as usize].vertex2[2],
            self.triangles[hit_data[1] as usize].vertex2[2],
            self.triangles[hit_data[2] as usize].vertex2[2],
            self.triangles[hit_data[3] as usize].vertex2[2],
        );

        let edge1_x = vertex1_x - vertex0_x;
        let edge1_y = vertex1_y - vertex0_y;
        let edge1_z = vertex1_z - vertex0_z;

        let edge2_x = vertex2_x - vertex0_x;
        let edge2_y = vertex2_y - vertex0_y;
        let edge2_z = vertex2_z - vertex0_z;

        let p_x = org_x + dir_x * t;
        let p_y = org_y + dir_y * t;
        let p_z = org_z + dir_z * t;

        let n_x = Vec4::new(
            self.triangles[hit_data[0] as usize].normal[0],
            self.triangles[hit_data[1] as usize].normal[0],
            self.triangles[hit_data[2] as usize].normal[0],
            self.triangles[hit_data[3] as usize].normal[0],
        );
        let n_y = Vec4::new(
            self.triangles[hit_data[0] as usize].normal[1],
            self.triangles[hit_data[1] as usize].normal[1],
            self.triangles[hit_data[2] as usize].normal[1],
            self.triangles[hit_data[3] as usize].normal[1],
        );
        let n_z = Vec4::new(
            self.triangles[hit_data[0] as usize].normal[2],
            self.triangles[hit_data[1] as usize].normal[2],
            self.triangles[hit_data[2] as usize].normal[2],
            self.triangles[hit_data[3] as usize].normal[2],
        );

        let abc_x: Vec4 = edge1_y * edge2_z - edge1_z * edge2_y;
        let abc_y: Vec4 = edge1_z * edge2_x - edge1_z * edge2_z;
        let abc_z: Vec4 = edge1_x * edge2_y - edge1_z * edge2_x;
        let abc = n_x * abc_x + n_y * abc_y + n_z * abc_z;

        let v0_p_x: Vec4 = vertex0_x - p_x;
        let v0_p_y: Vec4 = vertex0_y - p_y;
        let v0_p_z: Vec4 = vertex0_z - p_z;

        let v1_p_x: Vec4 = vertex1_x - p_x;
        let v1_p_y: Vec4 = vertex1_y - p_y;
        let v1_p_z: Vec4 = vertex1_z - p_z;

        let v2_p_x: Vec4 = vertex2_x - p_x;
        let v2_p_y: Vec4 = vertex2_y - p_y;
        let v2_p_z: Vec4 = vertex2_z - p_z;

        let pbc_x: Vec4 = v1_p_y * v2_p_z - v1_p_z * v2_p_y;
        let pbc_y: Vec4 = v1_p_z * v2_p_x - v1_p_z * v2_p_z;
        let pbc_z: Vec4 = v1_p_x * v2_p_y - v1_p_z * v2_p_x;
        let pbc = n_x * pbc_x + n_y * pbc_y + n_z * pbc_z;

        let pca_x: Vec4 = v2_p_y * v0_p_z - v2_p_z * v0_p_y;
        let pca_y: Vec4 = v2_p_z * v0_p_x - v2_p_z * v0_p_z;
        let pca_z: Vec4 = v2_p_x * v0_p_y - v2_p_z * v0_p_x;
        let pca = n_x * pca_x + n_y * pca_y + n_z * pca_z;

        let u: Vec4 = pbc / abc;
        let v: Vec4 = pca / abc;

        let w = Vec4::one() - u - v;

        let n0_x = Vec4::new(
            self.triangles[hit_data[0] as usize].n0[0],
            self.triangles[hit_data[1] as usize].n0[0],
            self.triangles[hit_data[2] as usize].n0[0],
            self.triangles[hit_data[3] as usize].n0[0],
        );
        let n0_y = Vec4::new(
            self.triangles[hit_data[0] as usize].n0[1],
            self.triangles[hit_data[1] as usize].n0[1],
            self.triangles[hit_data[2] as usize].n0[1],
            self.triangles[hit_data[3] as usize].n0[1],
        );
        let n0_z = Vec4::new(
            self.triangles[hit_data[0] as usize].n0[2],
            self.triangles[hit_data[1] as usize].n0[2],
            self.triangles[hit_data[2] as usize].n0[2],
            self.triangles[hit_data[3] as usize].n0[2],
        );
        let n1_x = Vec4::new(
            self.triangles[hit_data[0] as usize].n1[0],
            self.triangles[hit_data[1] as usize].n1[0],
            self.triangles[hit_data[2] as usize].n1[0],
            self.triangles[hit_data[3] as usize].n1[0],
        );
        let n1_y = Vec4::new(
            self.triangles[hit_data[0] as usize].n1[1],
            self.triangles[hit_data[1] as usize].n1[1],
            self.triangles[hit_data[2] as usize].n1[1],
            self.triangles[hit_data[3] as usize].n1[1],
        );
        let n1_z = Vec4::new(
            self.triangles[hit_data[0] as usize].n1[2],
            self.triangles[hit_data[1] as usize].n1[2],
            self.triangles[hit_data[2] as usize].n1[2],
            self.triangles[hit_data[3] as usize].n1[2],
        );
        let n2_x = Vec4::new(
            self.triangles[hit_data[0] as usize].n2[0],
            self.triangles[hit_data[1] as usize].n2[0],
            self.triangles[hit_data[2] as usize].n2[0],
            self.triangles[hit_data[3] as usize].n2[0],
        );
        let n2_y = Vec4::new(
            self.triangles[hit_data[0] as usize].n2[1],
            self.triangles[hit_data[1] as usize].n2[1],
            self.triangles[hit_data[2] as usize].n2[1],
            self.triangles[hit_data[3] as usize].n2[1],
        );
        let n2_z = Vec4::new(
            self.triangles[hit_data[0] as usize].n2[2],
            self.triangles[hit_data[1] as usize].n2[2],
            self.triangles[hit_data[2] as usize].n2[2],
            self.triangles[hit_data[3] as usize].n2[2],
        );

        let vn_x = u * n0_x + v * n1_x + w * n2_x;
        let vn_y = u * n0_y + v * n1_y + w * n2_y;
        let vn_z = u * n0_z + v * n1_z + w * n2_z;

        let t_u0 = Vec4::new(
            self.triangles[hit_data[0] as usize].u0,
            self.triangles[hit_data[1] as usize].u0,
            self.triangles[hit_data[2] as usize].u0,
            self.triangles[hit_data[3] as usize].u0,
        );
        let t_u1 = Vec4::new(
            self.triangles[hit_data[0] as usize].u1,
            self.triangles[hit_data[1] as usize].u1,
            self.triangles[hit_data[2] as usize].u1,
            self.triangles[hit_data[3] as usize].u1,
        );
        let t_u2 = Vec4::new(
            self.triangles[hit_data[0] as usize].u2,
            self.triangles[hit_data[1] as usize].u2,
            self.triangles[hit_data[2] as usize].u2,
            self.triangles[hit_data[3] as usize].u2,
        );

        let t_v0 = Vec4::new(
            self.triangles[hit_data[0] as usize].v0,
            self.triangles[hit_data[1] as usize].v0,
            self.triangles[hit_data[2] as usize].v0,
            self.triangles[hit_data[3] as usize].v0,
        );
        let t_v1 = Vec4::new(
            self.triangles[hit_data[0] as usize].v1,
            self.triangles[hit_data[1] as usize].v1,
            self.triangles[hit_data[2] as usize].v1,
            self.triangles[hit_data[3] as usize].v1,
        );
        let t_v2 = Vec4::new(
            self.triangles[hit_data[0] as usize].v2,
            self.triangles[hit_data[1] as usize].v2,
            self.triangles[hit_data[2] as usize].v2,
            self.triangles[hit_data[3] as usize].v2,
        );

        let t_u: Vec4 = t_u0 * u + t_u1 * v + t_u2 * w;
        let t_v: Vec4 = t_v0 * u + t_v1 * v + t_v2 * w;

        HitRecord4 {
            normal_x: vn_x.into(),
            normal_y: vn_y.into(),
            normal_z: vn_z.into(),
            t: t.into(),
            p_x: p_x.into(),
            p_y: p_y.into(),
            p_z: p_z.into(),
            mat_id: [
                self.materials[hit_data[0] as usize],
                self.materials[hit_data[1] as usize],
                self.materials[hit_data[2] as usize],
                self.materials[hit_data[3] as usize],
            ],
            g_normal_x: n_x.into(),
            g_normal_y: n_y.into(),
            g_normal_z: n_z.into(),
            u: t_u.into(),
            v: t_v.into(),
        }
    }
}

impl Intersect for AnimatedMesh {
    fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool {
        if let Some(mbvh) = self.mbvh.as_ref() {
            let (origin, direction) = ray.get_vectors::<Vec3A>();

            let intersection_test = |i, t_min, t_max| {
                let triangle: &RTTriangle = unsafe { self.triangles.get_unchecked(i) };
                triangle.occludes(ray, t_min, t_max)
            };

            mbvh.occludes(
                origin.as_ref(),
                direction.as_ref(),
                t_min,
                t_max,
                intersection_test,
            )
        } else {
            false
        }
    }

    fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        if let Some(mbvh) = self.mbvh.as_ref() {
            let (origin, direction) = ray.get_vectors::<Vec3A>();

            let intersection_test = |i, t_min, t_max| {
                let triangle: &RTTriangle = &self.triangles[i];
                if let Some(mut hit) = triangle.intersect(ray, t_min, t_max) {
                    hit.mat_id = self.materials[i];
                    Some((hit.t, hit))
                } else {
                    None
                }
            };

            mbvh.traverse(
                origin.as_ref(),
                direction.as_ref(),
                t_min,
                t_max,
                intersection_test,
            )
        } else {
            None
        }
    }

    fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32> {
        if let Some(mbvh) = self.mbvh.as_ref() {
            let (origin, direction) = ray.get_vectors::<Vec3A>();

            let intersection_test = |i, t_min, t_max| {
                let triangle: &RTTriangle = unsafe { self.triangles.get_unchecked(i) };
                if let Some(t) = triangle.intersect_t(ray, t_min, t_max) {
                    return Some(t);
                }
                None
            };

            mbvh.traverse_t(
                origin.as_ref(),
                direction.as_ref(),
                t_min,
                t_max,
                intersection_test,
            )
        } else {
            None
        }
    }

    fn depth_test(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<(f32, u32)> {
        if let Some(mbvh) = self.mbvh.as_ref() {
            let (origin, direction) = ray.get_vectors::<Vec3A>();

            let intersection_test = |i, t_min, t_max| -> Option<(f32, u32)> {
                let triangle: &RTTriangle = unsafe { self.triangles.get_unchecked(i) };
                triangle.depth_test(ray, t_min, t_max)
            };

            let hit = mbvh.depth_test(
                origin.as_ref(),
                direction.as_ref(),
                t_min,
                t_max,
                intersection_test,
            );

            Some(hit)
        } else {
            None
        }
    }

    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[PrimID; 4]> {
        if let Some(mbvh) = self.mbvh.as_ref() {
            let mut prim_id = [-1 as PrimID; 4];
            let mut valid = false;
            let intersection_test = |i: usize, packet: &mut RayPacket4| {
                let triangle: &RTTriangle = unsafe { self.triangles.get_unchecked(i) };
                if let Some(hit) = triangle.intersect4(packet, t_min) {
                    valid = true;
                    for i in 0..4 {
                        if hit[i] >= 0 {
                            prim_id[i] = hit[i];
                        }
                    }
                }
            };

            mbvh.traverse4(packet, intersection_test);

            if valid {
                Some(prim_id)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn get_hit_record(&self, ray: Ray, t: f32, hit_data: u32) -> HitRecord {
        let mut hit_record = self.triangles[hit_data as usize].get_hit_record(ray, t, hit_data);
        hit_record.mat_id = self.materials[hit_data as usize];
        hit_record
    }

    fn get_mat_id(&self, prim_id: PrimID) -> u32 {
        self.materials[prim_id as usize]
    }
}

impl Bounds for AnimatedMesh {
    fn bounds(&self) -> AABB {
        self.bounds.clone()
    }
}

impl AnimatedMesh {
    pub fn to_static_mesh(&self, skin: &crate::graph::Skin) -> Mesh {
        let mut vertices = vec![VertexData::zero(); self.vertices.len()];
        let mut triangles = vec![RTTriangle::default(); self.triangles.len()];

        vertices.iter_mut().enumerate().for_each(|(i, v)| {
            let original_v = &self.vertices[i];
            let anim_data = &self.anim_vertex_data[i];

            let vertex = Vec4::from(original_v.vertex);
            let normal = Vec3A::from(original_v.normal).extend(0.0);
            let tangent = Vec4::from(original_v.tangent);

            let skin_matrix = skin.joint_matrices[anim_data.joints[0] as usize]
                * anim_data.weights[0]
                + skin.joint_matrices[anim_data.joints[1] as usize] * anim_data.weights[1]
                + skin.joint_matrices[anim_data.joints[2] as usize] * anim_data.weights[2]
                + skin.joint_matrices[anim_data.joints[3] as usize] * anim_data.weights[3];

            let normal_matrix: Mat4 = skin_matrix.inverse().transpose();

            *v = VertexData {
                vertex: (skin_matrix * vertex).into(),
                normal: (normal_matrix * normal).truncate().into(),
                tangent: (normal_matrix * tangent).into(),
                ..original_v.clone()
            }
        });

        triangles.iter_mut().enumerate().for_each(|(i, t)| {
            let original_t = &self.triangles[i];

            let v0 = i * 3;
            let v1 = v0 + 1;
            let v2 = v0 + 2;

            let v0 = &vertices[v0];
            let v1 = &vertices[v1];
            let v2 = &vertices[v2];

            *t = RTTriangle {
                normal: RTTriangle::normal(
                    Vec4::from(v0.vertex).truncate(),
                    Vec4::from(v1.vertex).truncate(),
                    Vec4::from(v2.vertex).truncate(),
                )
                .into(),
                n0: v0.normal,
                n1: v1.normal,
                n2: v2.normal,
                tangent0: v0.tangent,
                tangent1: v1.tangent,
                tangent2: v2.tangent,
                vertex0: [v0.vertex[0], v0.vertex[1], v0.vertex[2]],
                vertex1: [v1.vertex[0], v1.vertex[1], v1.vertex[2]],
                vertex2: [v2.vertex[0], v2.vertex[1], v2.vertex[2]],
                ..original_t.clone()
            }
        });

        Mesh {
            vertices,
            triangles,
            name: self.name.clone(),
            meshes: self.meshes.clone(),
            bounds: self.bounds,
            materials: self.materials.clone(),
            bvh: None,
            mbvh: None,
        }
    }
}

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
struct SerializedMesh {
    pub mesh: AnimatedMesh,
    pub materials: MaterialList,
}

#[cfg(feature = "object_caching")]
impl<'a> crate::objects::SerializableObject<'a, AnimatedMesh> for AnimatedMesh {
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
        self.meshes.iter().for_each(|mesh| {
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
        mesh.meshes
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
    ) -> Result<AnimatedMesh, Box<dyn std::error::Error>> {
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
        mesh.meshes.par_iter_mut().for_each(|m| {
            m.mat_id = *material_mapping.get(&m.mat_id).unwrap() as u32;
        });

        Ok(mesh)
    }
}
