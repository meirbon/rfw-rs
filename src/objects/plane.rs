use crate::objects::*;
use crate::scene::PrimID;
use bvh::{Bounds, RayPacket4, AABB, Ray};
use glam::*;
use crate::constants::EPSILON;

pub struct Plane {
    pos: [f32; 3],
    right: [f32; 3],
    mat_id: u32,
    up: [f32; 3],
    offset: f32,
    forward: [f32; 3],
    dims: [f32; 2],
}

impl Plane {
    pub fn new(pos: [f32; 3], up: [f32; 3], dims: [f32; 2], mat_id: u32) -> Plane {
        let pos = Vec3::from(pos);
        let up = Vec3::from(up).normalize();

        let offset = (pos - (pos - up)).length();

        let right = if up[0].abs() >= up[1].abs() {
            Vec3::new(up.z(), 0.0, -up.x()) / (up.x() * up.x() + up.z() * up.z()).sqrt()
        } else {
            Vec3::new(0.0, -up.z(), up.y()) / (up.y() * up.y() + up.z() * up.z()).sqrt()
        }.normalize();

        let forward = up.cross(right).normalize();

        Plane {
            pos: pos.into(),
            right: right.into(),
            mat_id,
            up: up.into(),
            offset,
            forward: forward.into(),
            dims,
        }
    }

    pub fn get_normal(&self) -> Vec3 { self.up.into() }

    pub fn get_uv(&self, p: Vec3) -> Vec2 {
        let center_to_hit = p - Vec3::from(self.pos);
        let dot_right = center_to_hit.dot(self.right.into());
        let dot_forward = center_to_hit.dot(self.forward.into());

        let u = dot_right % 1.0;
        let v = dot_forward % 1.0;

        let u = if u < 0.0 { 1.0 + u } else { u };
        let v = if v < 0.0 { 1.0 + v } else { v };

        Vec2::new(u, v)
    }
}

impl Intersect for Plane {
    fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool {
        let (origin, direction) = ray.into();
        let up = Vec3::from(self.up);

        let div = up.dot(direction);
        let t = -(up.dot(origin) + self.offset) / div;

        if t < t_min || t > t_max {
            return false;
        }

        true
    }

    fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let (origin, direction) = ray.into();
        let up = Vec3::from(self.up);

        let div = up.dot(direction);
        let t = -(up.dot(origin) + self.offset) / div;

        if t < t_min || t > t_max {
            return None;
        }

        let p = origin + t * direction;

        Some(HitRecord {
            normal: self.up,
            t,
            p: p.into(),
            mat_id: self.mat_id,
            g_normal: self.up,
            uv: self.get_uv(p).into(),
        })
    }

    fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32> {
        let (origin, direction) = ray.into();
        let up = Vec3::from(self.up);

        let div = up.dot(direction);
        let t = -(up.dot(origin) + self.offset) / div;

        if t < t_min || t > t_max {
            return None;
        }

        Some(t)
    }

    fn depth_test(&self, _: Ray, _: f32, _: f32) -> Option<(f32, u32)> {
        None
    }

    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[i32; 4]> {
        let (origin_x, origin_y, origin_z) = packet.origin_xyz();
        let (dir_x, dir_y, dir_z) = packet.direction_xyz();

        let up_x = Vec4::splat(self.up[0]);
        let up_y = Vec4::splat(self.up[1]);
        let up_z = Vec4::splat(self.up[2]);

        let div_x = up_x * dir_x;
        let div_y = up_y * dir_y;
        let div_z = up_z * dir_z;
        let div = div_x + div_y + div_z;

        let offset = Vec4::splat(self.offset);
        let up_dot_org_x = up_x * origin_x;
        let up_dot_org_y = up_y * origin_y;
        let up_dot_org_z = up_z * origin_z;
        let up_dot_org = up_dot_org_x + up_dot_org_y + up_dot_org_z;
        let t = -(up_dot_org + offset) / div;

        let mask = t.cmple(packet.t()) & t.cmpge(Vec4::from(*t_min));
        let mask = mask.bitmask();
        if mask == 0 {
            return None;
        }

        let x = if mask & 1 != 0 { 0 } else { -1 };
        let y = if mask & 2 != 0 { 0 } else { -1 };
        let z = if mask & 4 != 0 { 0 } else { -1 };
        let w = if mask & 8 != 0 { 0 } else { -1 };
        Some([x, y, z, w])
    }

    fn get_hit_record(&self, ray: Ray, t: f32, _: u32) -> HitRecord {
        let (origin, direction) = ray.into();
        let p = origin + direction * t;

        HitRecord {
            normal: self.up,
            t,
            p: p.into(),
            mat_id: self.mat_id,
            g_normal: self.up,
            uv: self.get_uv(p).into(),
        }
    }
}

impl Bounds for Plane {
    fn bounds(&self) -> AABB {
        let right_offset = self.dims[0] * Vec3::from(self.right);
        let forward_offset = self.dims[1] * Vec3::from(self.forward);

        let min = Vec3::from(self.pos) - right_offset - forward_offset - Vec3::splat(EPSILON);
        let max = Vec3::from(self.pos) + right_offset + forward_offset + Vec3::splat(EPSILON);

        AABB {
            min: min.into(),
            left_first: -1,
            max: max.into(),
            count: -1,
        }
    }
}
