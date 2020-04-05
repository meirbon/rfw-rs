use glam::*;
use rayon::prelude::*;

use crate::objects::*;
use crate::scene::{PrimID, USE_MBVH};
use bvh::{Bounds, RayPacket4, AABB, BVH, MBVH};

pub trait ToMesh {
    fn into_mesh(self) -> Mesh;
}

#[derive(Debug, Clone)]
pub struct Mesh {
    triangles: Vec<Triangle>,
    materials: Vec<u32>,
    bvh: BVH,
    mbvh: MBVH,
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
                light_id: -1,
            };
        });

        let timer = crate::utils::Timer::new();
        let aabbs = triangles
            .par_iter()
            .map(|t| t.bounds())
            .collect::<Vec<AABB>>();
        let bvh = BVH::construct(aabbs.as_slice());
        let mbvh = MBVH::construct(&bvh);
        println!("Building bvh took: {} ms", timer.elapsed_in_millis());

        Mesh {
            triangles,
            bvh,
            mbvh,
            materials: Vec::from(material_ids),
        }
    }

    pub fn scale(mut self, scaling: f32) -> Self {
        let scaling = Mat4::from_scale(Vec3::new(scaling, scaling, scaling));

        self.triangles.par_iter_mut().for_each(|t| {
            let vertex0 = scaling * Vec4::new(t.vertex0[0], t.vertex0[1], t.vertex0[2], 1.0);
            let vertex1 = scaling * Vec4::new(t.vertex1[0], t.vertex1[1], t.vertex1[2], 1.0);
            let vertex2 = scaling * Vec4::new(t.vertex2[0], t.vertex2[1], t.vertex2[2], 1.0);

            t.vertex0 = vertex0.truncate().into();
            t.vertex1 = vertex1.truncate().into();
            t.vertex2 = vertex2.truncate().into();
        });

        let aabbs = self
            .triangles
            .par_iter()
            .map(|t| t.bounds())
            .collect::<Vec<AABB>>();
        self.bvh = BVH::construct(aabbs.as_slice());
        self.mbvh = MBVH::construct(&self.bvh);

        self
    }
}

impl Intersect for Mesh {
    fn occludes(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> bool {
        let intersection_test = |i, t_min, t_max| {
            let triangle: &Triangle = unsafe { self.triangles.get_unchecked(i) };
            triangle.occludes(origin, direction, t_min, t_max)
        };

        unsafe {
            match USE_MBVH {
                true => self.mbvh.occludes(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
                _ => self.bvh.occludes(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
            }
        }
    }

    fn intersect(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
    ) -> Option<HitRecord> {
        let intersection_test = |i, t_min, t_max| {
            let triangle: &Triangle = unsafe { self.triangles.get_unchecked(i) };
            if let Some(mut hit) = triangle.intersect(origin, direction, t_min, t_max) {
                hit.mat_id = self.materials[i];
                return Some((hit.t, hit));
            }
            None
        };

        unsafe {
            match USE_MBVH {
                true => self.mbvh.traverse(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
                _ => self.bvh.traverse(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
            }
        }
    }

    fn intersect_t(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<f32> {
        let intersection_test = |i, t_min, t_max| {
            let triangle: &Triangle = unsafe { self.triangles.get_unchecked(i) };
            if let Some(t) = triangle.intersect_t(origin, direction, t_min, t_max) {
                return Some(t);
            }
            None
        };

        unsafe {
            match USE_MBVH {
                true => self.mbvh.traverse_t(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
                _ => self.bvh.traverse_t(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
            }
        }
    }

    fn depth_test(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
    ) -> Option<(f32, u32)> {
        let intersection_test = |i, t_min, t_max| -> Option<(f32, u32)> {
            let triangle: &Triangle = unsafe { self.triangles.get_unchecked(i) };
            triangle.depth_test(origin, direction, t_min, t_max)
        };

        let hit = unsafe {
            match USE_MBVH {
                true => self.mbvh.depth_test(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
                _ => self.bvh.depth_test(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
            }
        };

        Some(hit)
    }

    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[PrimID; 4]> {
        let mut prim_id = [-1 as PrimID; 4];
        let mut valid = false;
        let intersection_test = |i: usize, packet: &mut RayPacket4| {
            let triangle: &Triangle = unsafe { self.triangles.get_unchecked(i) };
            if let Some(hit) = triangle.intersect4(packet, t_min) {
                valid = true;
                for i in 0..4 {
                    if hit[i] >= 0 {
                        prim_id[i] = hit[i];
                    }
                }
            }
        };

        unsafe {
            match USE_MBVH {
                true => self.mbvh.traverse4(packet, intersection_test),
                _ => self.bvh.traverse4(packet, intersection_test),
            }
        };

        if valid {
            Some(prim_id)
        } else {
            None
        }
    }
}

impl Bounds for Mesh {
    fn bounds(&self) -> AABB {
        self.bvh.nodes[0].bounds.clone()
    }
}
