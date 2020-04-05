use crate::objects::*;
use crate::scene::PrimID;
use bvh::{Bounds, RayPacket4, AABB, Ray};
use glam::*;

#[derive(Debug, Copy, Clone)]
pub struct Sphere {
    pos: [f32; 3],
    radius2: f32,
    pub mat_id: u32,
}

#[allow(dead_code)]
impl Sphere {
    pub fn new(pos: [f32; 3], radius: f32, mat_id: usize) -> Sphere {
        Sphere {
            pos,
            radius2: radius * radius,
            mat_id: mat_id as u32,
        }
    }

    pub fn normal(&self, p: Vec3) -> Vec3 {
        (p - self.pos.into()).normalize()
    }

    pub fn get_uv(&self, n: Vec3) -> Vec2 {
        let u = n.x().atan2(n.z()) * (1.0 / (2.0 * std::f32::consts::PI)) + 0.5;
        let v = n.y() * 0.5 + 0.5;
        vec2(u, v)
    }
}

impl Intersect for Sphere {
    fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool {
        let (origin, direction) = ray.into();

        let a = direction.dot(direction);
        let r_pos = origin - self.pos.into();

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

    fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let (origin, direction) = ray.into();

        let a = direction.dot(direction);
        let r_pos = origin - self.pos.into();

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

        Some(HitRecord {
            g_normal: normal.into(),
            normal: normal.into(),
            t,
            p: p.into(),
            mat_id: self.mat_id,
            uv: uv.into(),
        })
    }

    fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32> {
        let (origin, direction) = ray.into();

        let a = direction.dot(direction);
        let r_pos = origin - self.pos.into();

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
            None
        } else {
            Some(t)
        }
    }

    fn depth_test(&self, _: Ray, _: f32, _: f32) -> Option<(f32, u32)> {
        None
    }

    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[PrimID; 4]> {
        let origin_x = Vec4::from(packet.origin_x);
        let origin_y = Vec4::from(packet.origin_y);
        let origin_z = Vec4::from(packet.origin_z);

        let direction_x = Vec4::from(packet.direction_x);
        let direction_y = Vec4::from(packet.direction_y);
        let direction_z = Vec4::from(packet.direction_z);

        let a_x: Vec4 = direction_x * direction_x;
        let a_y: Vec4 = direction_y * direction_y;
        let a_z: Vec4 = direction_z * direction_z;
        let a: Vec4 = a_x + a_y + a_z;

        let r_pos_x: Vec4 = origin_x - Vec4::from([self.pos[0]; 4]);
        let r_pos_y: Vec4 = origin_y - Vec4::from([self.pos[1]; 4]);
        let r_pos_z: Vec4 = origin_z - Vec4::from([self.pos[2]; 4]);

        let b_x: Vec4 = direction_x * 2.0 * r_pos_x;
        let b_y: Vec4 = direction_y * 2.0 * r_pos_y;
        let b_z: Vec4 = direction_z * 2.0 * r_pos_z;
        let b: Vec4 = b_x + b_y + b_z;

        let r_pos2_x: Vec4 = r_pos_x * r_pos_x;
        let r_pos2_y: Vec4 = r_pos_y * r_pos_y;
        let r_pos2_z: Vec4 = r_pos_z * r_pos_z;
        let r_pos2: Vec4 = r_pos2_x + r_pos2_y + r_pos2_z;

        let radius: Vec4 = Vec4::from([self.radius2; 4]);
        let c: Vec4 = r_pos2 - radius;
        let d: Vec4 = b * b - 4.0 * a * c;

        let t_min = Vec4::from(*t_min);

        let mask = d.cmpge(Vec4::zero());
        // No hits
        if mask.bitmask() == 0 {
            return None;
        }

        let div_2a = Vec4::one() / (2.0 * a);
        let sqrt_d = unsafe {
            use std::arch::x86_64::_mm_sqrt_ps;
            Vec4::from(_mm_sqrt_ps(d.into())).max(Vec4::zero())
        };

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;
        let pick_t1 = t1.cmpgt(t_min) & t1.cmplt(t2);
        let t = pick_t1.select(t1, t2);
        let mask = mask & (t.cmpgt(t_min) & t.cmplt(packet.t.into()));
        let bitmask = mask.bitmask();
        if bitmask == 0 {
            return None;
        }
        packet.t = mask.select(t, packet.t.into()).into();

        let x = if bitmask & 1 != 0 { 0 } else { -1 };
        let y = if bitmask & 2 != 0 { 0 } else { -1 };
        let z = if bitmask & 4 != 0 { 0 } else { -1 };
        let w = if bitmask & 8 != 0 { 0 } else { -1 };
        Some([x, y, z, w])
    }

    fn get_hit_record(&self, ray: Ray, t: f32, _: u32) -> HitRecord {
        let (origin, direction) = ray.into();

        let p = origin + direction * t;
        let normal = self.normal(p);
        let uv = self.get_uv(normal);

        HitRecord {
            g_normal: normal.into(),
            normal: normal.into(),
            t,
            p: p.into(),
            mat_id: self.mat_id,
            uv: uv.into(),
        }
    }
}

impl Bounds for Sphere {
    fn bounds(&self) -> AABB {
        let radius = self.radius2.sqrt() + crate::constants::AABB_EPSILON;
        let min: [f32; 3] = [
            self.pos[0] - radius,
            self.pos[1] - radius,
            self.pos[2] - radius,
        ];
        let max: [f32; 3] = [
            self.pos[0] + radius,
            self.pos[1] + radius,
            self.pos[2] + radius,
        ];
        AABB {
            min,
            left_first: -1,
            max,
            count: -1,
        }
    }
}
