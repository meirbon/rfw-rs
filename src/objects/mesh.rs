use glam::*;
use rayon::prelude::*;

use crate::objects::*;
use crate::bvh::*;

pub trait ToMesh {
    fn into_mesh(self) -> Mesh;
}

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

        let timer = crate::utils::Timer::new();
        let bvh = BVH::construct(triangles.as_slice());
        println!("Building bvh took: {} ms", timer.elapsed_in_millis());

        Mesh { triangles, bvh, materials: Vec::from(material_ids) }
    }

    pub fn scale(mut self, scaling: f32) -> Self {
        let scaling = Mat4::from_scale(Vec3::new(scaling, scaling, scaling));

        self.triangles.par_iter_mut().for_each(|t| {
            let vertex0 = scaling * Vec4::new(t.vertex0.x(), t.vertex0.y(), t.vertex0.z(), 1.0);
            let vertex1 = scaling * Vec4::new(t.vertex1.x(), t.vertex1.y(), t.vertex1.z(), 1.0);
            let vertex2 = scaling * Vec4::new(t.vertex2.x(), t.vertex2.y(), t.vertex2.z(), 1.0);

            let vertex0 = Vec3::new(vertex0.x(), vertex0.y(), vertex0.z());
            let vertex1 = Vec3::new(vertex1.x(), vertex1.y(), vertex1.z());
            let vertex2 = Vec3::new(vertex2.x(), vertex2.y(), vertex2.z());

            t.vertex0 = vertex0;
            t.vertex1 = vertex1;
            t.vertex2 = vertex2;
        });

        self.bvh = BVH::construct(self.triangles.as_slice());

        self
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