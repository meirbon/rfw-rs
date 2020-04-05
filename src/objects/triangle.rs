use crate::constants::EPSILON;
use crate::objects::*;
use crate::scene::PrimID;
use glam::*;

use bvh::{Bounds, RayPacket4, AABB};
use std::ops::BitAnd;

#[derive(Copy, Clone, Debug)]
pub struct Triangle {
    pub vertex0: [f32; 3],
    pub u0: f32,

    pub vertex1: [f32; 3],
    pub u1: f32,

    pub vertex2: [f32; 3],
    pub u2: f32,

    pub normal: [f32; 3],
    pub v0: f32,

    pub n0: [f32; 3],
    pub v1: f32,

    pub n1: [f32; 3],
    pub v2: f32,

    pub n2: [f32; 3],
    pub id: i32,

    pub light_id: i32,
}

#[allow(dead_code)]
impl Triangle {
    #[inline]
    pub fn area(&self) -> f32 {
        let a = (Vec3::from(self.vertex1) - Vec3::from(self.vertex0)).length();
        let b = (Vec3::from(self.vertex2) - Vec3::from(self.vertex1)).length();
        let c = (Vec3::from(self.vertex0) - Vec3::from(self.vertex2)).length();
        let s = (a + b + c) * 0.5;
        (s * (s - a) * (s - b) * (s - c)).sqrt()
    }

    #[inline]
    pub fn normal(v0: Vec3, v1: Vec3, v2: Vec3) -> Vec3 {
        let a = v1 - v0;
        let b = v2 - v0;
        a.cross(b).normalize()
    }

    pub fn zero() -> Triangle {
        Triangle {
            vertex0: [0.0; 3],
            u0: 0.0,
            vertex1: [0.0; 3],
            u1: 0.0,
            vertex2: [0.0; 3],
            u2: 0.0,
            normal: [0.0; 3],
            v0: 0.0,
            n0: [0.0; 3],
            v1: 0.0,
            n1: [0.0; 3],
            v2: 0.0,
            n2: [0.0; 3],
            id: -1,
            light_id: -1,
        }
    }

    #[inline(always)]
    pub fn bary_centrics(
        v0: Vec3,
        v1: Vec3,
        v2: Vec3,
        edge1: Vec3,
        edge2: Vec3,
        p: Vec3,
        n: Vec3,
    ) -> (f32, f32) {
        let abc = n.dot((edge1).cross(edge2));
        let pbc = n.dot((v1 - p).cross(v2 - p));
        let pca = n.dot((v2 - p).cross(v0 - p));
        (pbc / abc, pca / abc)
    }

    // Transforms triangle using given matrix and normal_matrix (transposed of inverse of matrix)
    pub fn transform(&self, matrix: Mat4, normal_matrix: Mat3) -> Triangle {
        let vertex0 = Vec3::from(self.vertex0).extend(1.0);
        let vertex1 = Vec3::from(self.vertex1).extend(1.0);
        let vertex2 = Vec3::from(self.vertex2).extend(1.0);

        let vertex0 = matrix * vertex0;
        let vertex1 = matrix * vertex1;
        let vertex2 = matrix * vertex2;

        let n0 = normal_matrix * Vec3::from(self.n0);
        let n1 = normal_matrix * Vec3::from(self.n1);
        let n2 = normal_matrix * Vec3::from(self.n2);

        Triangle {
            vertex0: vertex0.truncate().into(),
            vertex1: vertex1.truncate().into(),
            vertex2: vertex2.truncate().into(),
            n0: n0.into(),
            n1: n1.into(),
            n2: n2.into(),
            ..(*self)
        }
    }
}

impl Intersect for Triangle {
    fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool {
        let origin = Vec3::from(ray.origin);
        let direction = Vec3::from(ray.direction);

        let vertex0 = Vec3::from(self.vertex0);
        let vertex1 = Vec3::from(self.vertex1);
        let vertex2 = Vec3::from(self.vertex2);

        let edge1 = vertex1 - vertex0;
        let edge2 = vertex2 - vertex0;

        let h = direction.cross(edge2);
        let a = edge1.dot(h);
        if a > -EPSILON && a < EPSILON {
            return false;
        }

        let f = 1.0 / a;
        let s = origin - vertex0;
        let u = f * s.dot(h);
        if u < 0.0 || u > 1.0 {
            return false;
        }

        let q = s.cross(edge1);
        let v = f * direction.dot(q);
        if v < 0.0 || (u + v) > 1.0 {
            return false;
        }

        let t = f * edge2.dot(q);
        t > t_min && t < t_max
    }

    fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let origin = Vec3::from(ray.origin);
        let direction = Vec3::from(ray.direction);

        let vertex0 = Vec3::from(self.vertex0);
        let vertex1 = Vec3::from(self.vertex1);
        let vertex2 = Vec3::from(self.vertex2);

        let edge1 = vertex1 - vertex0;
        let edge2 = vertex2 - vertex0;

        let h = direction.cross(edge2);
        let a = edge1.dot(h);
        if a > -EPSILON && a < EPSILON {
            return None;
        }

        let f = 1.0 / a;
        let s = origin - vertex0;
        let u = f * s.dot(h);
        let q = s.cross(edge1);
        let v = f * direction.dot(q);

        if u < 0.0 || u > 1.0 || v < 0.0 || (u + v) > 1.0 {
            return None;
        }

        let t = f * edge2.dot(q);
        if t <= t_min || t >= t_max {
            return None;
        }

        let p = origin + direction * t;
        let (u, v) = Self::bary_centrics(
            vertex0,
            vertex1,
            vertex2,
            edge1,
            edge2,
            p,
            Vec3::from(self.normal),
        );
        let w = 1.0 - u - v;
        let normal = Vec3::from(self.n0) * u + Vec3::from(self.n1) * v + Vec3::from(self.n2) * w;
        let uv = Vec2::new(
            self.u0 * u + self.u1 * v + self.u2 * w,
            self.v0 * u + self.v1 * v + self.v2 * w,
        );

        Some(HitRecord {
            g_normal: self.normal,
            normal: normal.into(),
            t,
            p: p.into(),
            mat_id: 0,
            uv: uv.into(),
        })
    }

    fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32> {
        let (origin, direction) = ray.into();

        let vertex0 = Vec3::from(self.vertex0);
        let vertex1 = Vec3::from(self.vertex1);
        let vertex2 = Vec3::from(self.vertex2);

        let edge1 = vertex1 - vertex0;
        let edge2 = vertex2 - vertex0;

        let h = direction.cross(edge2);
        let a = edge1.dot(h);
        if a > -EPSILON && a < EPSILON {
            return None;
        }

        let f = 1.0 / a;
        let s = origin - vertex0;
        let u = f * s.dot(h);
        if u < 0.0 || u > 1.0 {
            return None;
        }

        let q = s.cross(edge1);
        let v = f * direction.dot(q);
        if v < 0.0 || (u + v) > 1.0 {
            return None;
        }

        let t = f * edge2.dot(q);
        if t <= t_min || t >= t_max {
            return None;
        }

        Some(t)
    }

    fn depth_test(&self, _: Ray, _: f32, _: f32) -> Option<(f32, u32)> {
        None
    }

    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[PrimID; 4]> {
        let zero = Vec4::zero();
        let one = Vec4::one();

        let org_x = Vec4::from(packet.origin_x);
        let org_y = Vec4::from(packet.origin_y);
        let org_z = Vec4::from(packet.origin_z);

        let dir_x = Vec4::from(packet.direction_x);
        let dir_y = Vec4::from(packet.direction_y);
        let dir_z = Vec4::from(packet.direction_z);

        let p0_x = Vec4::from([self.vertex0[0]; 4]);
        let p0_y = Vec4::from([self.vertex0[1]; 4]);
        let p0_z = Vec4::from([self.vertex0[2]; 4]);

        let p1_x = Vec4::from([self.vertex1[0]; 4]);
        let p1_y = Vec4::from([self.vertex1[1]; 4]);
        let p1_z = Vec4::from([self.vertex1[2]; 4]);

        let p2_x = Vec4::from([self.vertex2[0]; 4]);
        let p2_y = Vec4::from([self.vertex2[1]; 4]);
        let p2_z = Vec4::from([self.vertex2[2]; 4]);

        let edge1_x = p1_x - p0_x;
        let edge1_y = p1_y - p0_y;
        let edge1_z = p1_z - p0_z;

        let edge2_x = p2_x - p0_x;
        let edge2_y = p2_y - p0_y;
        let edge2_z = p2_z - p0_z;

        let h_x = (dir_y * edge2_z) - (dir_z * edge2_y);
        let h_y = (dir_z * edge2_x) - (dir_x * edge2_z);
        let h_z = (dir_x * edge2_y) - (dir_y * edge2_x);

        let a = (edge1_x * h_x) + (edge1_y * h_y) + (edge1_z * h_z);
        let epsilon = Vec4::from([EPSILON; 4]);
        let mask = a.cmple(-epsilon) | a.cmpge(epsilon);
        if mask.bitmask() == 0 {
            return None;
        }

        let f = one / a;
        let s_x = org_x - p0_x;
        let s_y = org_y - p0_y;
        let s_z = org_z - p0_z;

        let u = f * ((s_x * h_x) + (s_y * h_y) + (s_z * h_z));
        let mask = mask.bitand(u.cmpge(zero) & u.cmple(one));
        if mask.bitmask() == 0 {
            return None;
        }

        let q_x = s_y * edge1_z - s_z * edge1_y;
        let q_y = s_z * edge1_x - s_x * edge1_z;
        let q_z = s_x * edge1_y - s_y * edge1_x;

        let v = f * ((dir_x * q_x) + (dir_y * q_y) + (dir_z * q_z));
        let mask = mask.bitand(v.cmpge(zero) & (u + v).cmple(one));
        if mask.bitmask() == 0 {
            return None;
        }

        let t_min = Vec4::from(*t_min);

        let t = f * ((edge2_x * q_x) + (edge2_y * q_y) + (edge2_z * q_z));
        let mask = mask.bitand(t.cmpge(t_min) & t.cmplt(packet.t.into()));
        let bitmask = mask.bitmask();
        if bitmask == 0 {
            return None;
        }
        packet.t = mask.select(t, packet.t.into()).into();

        let x = if bitmask & 1 != 0 { self.id } else { -1 };
        let y = if bitmask & 2 != 0 { self.id } else { -1 };
        let z = if bitmask & 4 != 0 { self.id } else { -1 };
        let w = if bitmask & 8 != 0 { self.id } else { -1 };
        Some([x, y, z, w])
    }

    fn get_hit_record(&self, ray: Ray, t: f32, _: u32) -> HitRecord {
        let (origin, direction) = ray.into();
        let vertex0 = Vec3::from(self.vertex0);
        let vertex1 = Vec3::from(self.vertex1);
        let vertex2 = Vec3::from(self.vertex2);
        let edge1 = vertex1 - vertex0;
        let edge2 = vertex2 - vertex0;

        let p = origin + direction * t;
        let (u, v) = Self::bary_centrics(
            vertex0,
            vertex1,
            vertex2,
            edge1,
            edge2,
            p,
            Vec3::from(self.normal),
        );
        let w = 1.0 - u - v;
        let normal = Vec3::from(self.n0) * u + Vec3::from(self.n1) * v + Vec3::from(self.n2) * w;
        let uv = Vec2::new(
            self.u0 * u + self.u1 * v + self.u2 * w,
            self.v0 * u + self.v1 * v + self.v2 * w,
        );

        HitRecord {
            g_normal: self.normal,
            normal: normal.into(),
            t,
            p: p.into(),
            mat_id: 0,
            uv: uv.into(),
        }
    }
}

impl Bounds for Triangle {
    fn bounds(&self) -> AABB {
        let mut aabb = AABB::new();
        aabb.grow(Vec3::from(self.vertex0));
        aabb.grow(Vec3::from(self.vertex1));
        aabb.grow(Vec3::from(self.vertex2));
        aabb.offset_by(crate::constants::AABB_EPSILON);
        aabb
    }
}
