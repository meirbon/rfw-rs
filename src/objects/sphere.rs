use glam::*;
use crate::objects::*;
use crate::bvh::AABB;

pub struct Sphere {
    pos: Vec3,
    radius2: f32,
    pub mat_id: u32,
}

#[allow(dead_code)]
impl Sphere {
    pub fn new(pos: Vec3, radius: f32, mat_id: u32) -> Sphere {
        Sphere { pos, radius2: radius * radius, mat_id }
    }

    pub fn normal(&self, p: Vec3) -> Vec3 {
        (p - self.pos).normalize()
    }

    pub fn get_uv(&self, n: Vec3) -> Vec2 {
        let u = n.x().atan2(n.z()) * (1.0 / (2.0 * std::f32::consts::PI)) + 0.5;
        let v = n.y() * 0.5 + 0.5;
        vec2(u, v)
    }
}

impl Intersect for Sphere {
    fn occludes(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> bool {
        let a = direction.dot(direction);
        let r_pos = origin - self.pos;

        let b = (direction * 2.0).dot(r_pos);
        let r_pos2 = r_pos.dot(r_pos);
        let c = r_pos2 - self.radius2;

        let d: f32 = (b * b) - (4.0 * a * c);

        if d < 0.0 {
            return false;
        }

        let div_2a = 1.0 / (2.0 * a);
        let sqrt_d = if d > 0.0 { d.sqrt() } else { 0.0 };

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;
        let t = if t1 > t_min && t1 < t2 { t1 } else { t2 };

        t < t_max
    }

    fn intersect(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let a = direction.dot(direction);
        let r_pos = origin - self.pos;

        let b = (direction * 2.0).dot(r_pos);
        let r_pos2 = r_pos.dot(r_pos);
        let c = r_pos2 - self.radius2;

        let d: f32 = (b * b) - (4.0 * a * c);

        if d < 0.0 {
            return None;
        }

        let div_2a = 1.0 / (2.0 * a);

        let sqrt_d = if d > 0.0 { d.sqrt() } else { 0.0 };

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;

        let t = if t1 > t_min && t1 < t2 { t1 } else { t2 };
        if t <= t_min || t >= t_max {
            return None;
        }

        let p = origin + direction * t;
        let normal = self.normal(p);
        let uv = self.get_uv(normal);

        Some(HitRecord { normal, t, p, mat_id: self.mat_id, uv })
    }

    fn intersect_t(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<f32> {
        let a = direction.dot(direction);
        let r_pos = origin - self.pos;

        let b = (direction * 2.0).dot(r_pos);
        let r_pos2 = r_pos.dot(r_pos);
        let c = r_pos2 - self.radius2;

        let d: f32 = (b * b) - (4.0 * a * c);

        if d < 0.0 {
            return None;
        }

        let div_2a = 1.0 / (2.0 * a);

        let sqrt_d = if d > 0.0 { d.sqrt() } else { 0.0 };

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;

        let t = if t1 > t_min && t1 < t2 { t1 } else { t2 };

        if t <= t_min || t >= t_max { None } else { Some(t) }
    }

    fn bounds(&self) -> AABB {
        let radius = [self.radius2.sqrt() + crate::constants::AABB_EPSILON; 3].into();
        let min = self.pos - radius;
        let max = self.pos + radius;
        AABB {
            min,
            left_first: -1,
            max,
            count: -1,
        }
    }
}