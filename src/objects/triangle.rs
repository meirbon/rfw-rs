use glam::*;
use crate::objects::*;
use bvh::aabb::Bounds;
use bvh::AABB;

#[derive(Copy, Clone, Debug)]
pub struct Triangle {
    pub vertex0: Vec3,
    pub vertex1: Vec3,
    pub vertex2: Vec3,
    pub normal: Vec3,
    pub n0: Vec3,
    pub n1: Vec3,
    pub n2: Vec3,
    pub uv0: Vec2,
    pub uv1: Vec2,
    pub uv2: Vec2,
}

#[allow(dead_code)]
impl Triangle {
    #[inline]
    pub fn area(&self) -> f32 {
        let a = (self.vertex1 - self.vertex0).length();
        let b = (self.vertex2 - self.vertex1).length();
        let c = (self.vertex0 - self.vertex2).length();
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
            vertex0: Vec3::zero(),
            vertex1: Vec3::zero(),
            vertex2: Vec3::zero(),
            normal: Vec3::zero(),
            n0: Vec3::zero(),
            n1: Vec3::zero(),
            n2: Vec3::zero(),
            uv0: Vec2::zero(),
            uv1: Vec2::zero(),
            uv2: Vec2::zero(),
        }
    }

    #[inline]
    pub fn bary_centrics(v0: Vec3, v1: Vec3, v2: Vec3, p: Vec3, n: Vec3) -> (f32, f32) {
        let abc = n.dot((v1 - v0).cross(v2 - v0));
        let pbc = n.dot((v1 - p).cross(v2 - p));
        let pca = n.dot((v2 - p).cross(v0 - p));
        (pbc / abc, pca / abc)
    }

    // Transforms triangle using given matrix and normal_matrix (transposed of inverse of matrix)
    pub fn transform(&self, matrix: Mat4, normal_matrix: Mat3) -> Triangle {
        let vertex0 = Vec4::new(self.vertex0.x(), self.vertex0.y(), self.vertex0.z(), 1.0);
        let vertex1 = Vec4::new(self.vertex1.x(), self.vertex1.y(), self.vertex1.z(), 1.0);
        let vertex2 = Vec4::new(self.vertex2.x(), self.vertex2.y(), self.vertex2.z(), 1.0);

        let vertex0 = matrix * vertex0;
        let vertex1 = matrix * vertex1;
        let vertex2 = matrix * vertex2;

        let vertex0 = Vec3::new(vertex0.x(), vertex0.y(), vertex0.z());
        let vertex1 = Vec3::new(vertex1.x(), vertex1.y(), vertex1.z());
        let vertex2 = Vec3::new(vertex2.x(), vertex2.y(), vertex2.z());

        let n0 = normal_matrix * self.n0;
        let n1 = normal_matrix * self.n1;
        let n2 = normal_matrix * self.n2;

        Triangle {
            vertex0,
            vertex1,
            vertex2,
            n0,
            n1,
            n2,
            ..(*self)
        }
    }
}

impl Intersect for Triangle {
    fn occludes(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> bool {
        let edge1 = self.vertex1 - self.vertex0;
        let edge2 = self.vertex2 - self.vertex0;

        let h = direction.cross(edge2);
        let a = edge1.dot(h);
        if a > -crate::constants::EPSILON && a < crate::constants::EPSILON {
            return false;
        }

        let f = 1.0 / a;
        let s = origin - self.vertex0;
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

    fn intersect(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let edge1 = self.vertex1 - self.vertex0;
        let edge2 = self.vertex2 - self.vertex0;

        let h = direction.cross(edge2);
        let a = edge1.dot(h);
        if a > -crate::constants::EPSILON && a < crate::constants::EPSILON {
            return None;
        }

        let f = 1.0 / a;
        let s = origin - self.vertex0;
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
        let (u, v) = Self::bary_centrics(self.vertex0, self.vertex1, self.vertex2, p, self.normal);
        let w = 1.0 - u - v;
        let normal = self.n0 * u + self.n1 * v + self.n2 * w;
        let uv = self.uv0 * u + self.uv1 * v + self.uv2 * w;

        Some(HitRecord {
            normal,
            t,
            p,
            mat_id: 0,
            uv,
        })
    }

    fn intersect_t(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<f32> {
        let edge1 = self.vertex1 - self.vertex0;
        let edge2 = self.vertex2 - self.vertex0;

        let h = direction.cross(edge2);
        let a = edge1.dot(h);
        if a > -crate::constants::EPSILON && a < crate::constants::EPSILON {
            return None;
        }

        let f = 1.0 / a;
        let s = origin - self.vertex0;
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
}

impl Bounds for Triangle {
    fn bounds(&self) -> AABB {
        let mut aabb = AABB::new();
        aabb.grow(self.vertex0);
        aabb.grow(self.vertex1);
        aabb.grow(self.vertex2);
        aabb.offset_by(crate::constants::AABB_EPSILON);
        aabb
    }
}