use glam::*;
use crate::objects::*;
use bvh::AABB;
use bvh::Bounds;
use crate::camera::RayPacket4;
use std::ops::BitAnd;
use crate::scene::PrimID;

use std::arch::x86_64::*;

#[derive(Debug, Copy, Clone)]
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

        Some(HitRecord { normal: normal.into(), t, p: p.into(), mat_id: self.mat_id, uv: uv.into() })
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

    fn depth_test(&self, _: Vec3, _: Vec3, _: f32, _: f32) -> Option<(f32, u32)> {
        None
    }

    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> [PrimID; 4] {
        let origin_x = Vec4::from(packet.origin_x);
        let origin_y = Vec4::from(packet.origin_y);
        let origin_z = Vec4::from(packet.origin_z);

        let direction_x = Vec4::from(packet.direction_x);
        let direction_y = Vec4::from(packet.direction_y);
        let direction_z = Vec4::from(packet.direction_z);

        let a_x = direction_x * direction_x;
        let a_y = direction_y * direction_y;
        let a_z = direction_z * direction_z;
        let a = a_x + a_y + a_z;

        let r_pos_x = origin_x - Vec4::from([self.pos.x(); 4]);
        let r_pos_y = origin_y - Vec4::from([self.pos.y(); 4]);
        let r_pos_z = origin_z - Vec4::from([self.pos.z(); 4]);

        let b_x = direction_x * 2.0 * r_pos_x;
        let b_y = direction_y * 2.0 * r_pos_y;
        let b_z = direction_z * 2.0 * r_pos_z;
        let b = b_x + b_y + b_z;

        let r_pos2_x = r_pos_x * r_pos_x;
        let r_pos2_y = r_pos_y * r_pos_y;
        let r_pos2_z = r_pos_z * r_pos_z;
        let r_pos2 = r_pos2_x + r_pos2_y + r_pos2_z;

        let radius = Vec4::from([self.radius2; 4]);
        let c = r_pos2 - radius;
        let d = b * b - 4.0 * a * c;
        let t_min = Vec4::from(*t_min);

        let mask = d.cmplt(t_min);
        if mask.bitmask() == 0 { return [-1; 4]; }

        let div_2a = Vec4::one() / (2.0 * a);
        let sqrt_d = unsafe {
            Vec4::from(_mm_sqrt_ps(d.into())).max(Vec4::zero())
        };

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;
        let pick_t1 = t1.cmpgt(t_min).bitand(t1.cmplt(t2));
        let t = pick_t1.select(t1, t2);
        let mask = mask.bitand(t.cmpgt(t_min).bitand(t.cmplt(packet.t.into())));
        let bitmask = mask.bitmask();
        if bitmask == 0 { return [-1; 4]; }
        packet.t = mask.select(t, packet.t.into()).into();

        let x = if bitmask & 1 != 0 { 0 } else { -1 };
        let y = if bitmask & 2 != 0 { 0 } else { -1 };
        let z = if bitmask & 4 != 0 { 0 } else { -1 };
        let w = if bitmask & 8 != 0 { 0 } else { -1 };
        [x, y, z, w]
    }
}

impl Bounds for Sphere {
    fn bounds(&self) -> AABB {
        let radius = self.radius2.sqrt() + crate::constants::AABB_EPSILON;
        let min: [f32; 3] = [self.pos[0] - radius, self.pos[1] - radius, self.pos[2] - radius];
        let max: [f32; 3] = [self.pos[0] + radius, self.pos[1] + radius, self.pos[2] + radius];
        AABB {
            min,
            left_first: -1,
            max,
            count: -1,
        }
    }
}