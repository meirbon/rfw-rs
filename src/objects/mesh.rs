use glam::*;
use rayon::prelude::*;

use crate::objects::*;
use crate::bvh::*;

pub struct Mesh {
    triangles: Vec<Triangle>,
    materials: Vec<u32>,
    bvh: BVH,
}

impl Mesh {
    pub fn new(vertices: &[Vec3], normals: &[Vec3], uvs: &[Vec2], material_ids: &[u32]) -> Mesh {
        assert_eq!(vertices.len(), normals.len());
        assert_eq!(vertices.len(), uvs.len());
        assert_eq!(uvs.len(), material_ids.len() * 3);
        assert_eq!(vertices.len() % 3, 0);

        let mut triangles = vec![Triangle::zero(); vertices.len() / 3];
        triangles.iter_mut().enumerate().for_each(|(i, triangle)| {
            let i0 = i * 3;
            let i1 = i0 + 1;
            let i2 = i0 + 2;

            let vertex0 = unsafe { *vertices.get_unchecked(i0) };
            let vertex1 = unsafe { *vertices.get_unchecked(i1) };
            let vertex2 = unsafe { *vertices.get_unchecked(i2) };

            let n0 = unsafe { *normals.get_unchecked(i0) };
            let n1 = unsafe { *normals.get_unchecked(i1) };
            let n2 = unsafe { *normals.get_unchecked(i2) };

            let uv0 = unsafe { *uvs.get_unchecked(i0) };
            let uv1 = unsafe { *uvs.get_unchecked(i1) };
            let uv2 = unsafe { *uvs.get_unchecked(i2) };

            let normal = Triangle::normal(vertex0, vertex1, vertex2);

            *triangle = Triangle {
                vertex0,
                vertex1,
                vertex2,
                normal,
                n0,
                n1,
                n2,
                uv0,
                uv1,
                uv2,
            };
        });

        let bvh = BVH::construct(triangles.as_slice());

        Mesh { triangles, bvh, materials: Vec::from(material_ids) }
    }
}

impl Intersect for Mesh {
    fn occludes(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> bool {
        self.bvh.occludes(origin, direction, t_min, t_max, |i, t_min, t_max| {
            let triangle = unsafe { self.triangles.get_unchecked(i) };
            triangle.occludes(origin, direction, t_min, t_max)
        })
    }

    fn intersect(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<HitRecord> {
        self.bvh.traverse(origin, direction, t_min, t_max, |i, t_min, t_max| {
            let triangle = unsafe { self.triangles.get_unchecked(i) };
            if let Some(mut hit) = triangle.intersect(origin, direction, t_min, t_max) {
                hit.mat_id = self.materials[i];
                return Some((hit.t, hit));
            }
            None
        })
    }

    fn intersect_t(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<f32> {
        self.bvh.traverse_t(origin, direction, t_min, t_max, |i, t_min, t_max| {
            let triangle = unsafe { self.triangles.get_unchecked(i) };
            if let Some(t) = triangle.intersect_t(origin, direction, t_min, t_max) {
                return Some(t);
            }
            None
        })
    }

    fn bounds(&self) -> AABB {
        self.bvh.nodes[0].bounds.clone()
    }
}