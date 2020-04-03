use glam::*;
use crate::objects::*;
use bvh::aabb::Bounds;
use bvh::AABB;

pub struct Instance {
    bounds: AABB,
    transform: Mat4,
    inverse: Mat4,
    normal_transform: Mat4,
}

#[allow(dead_code)]
impl Instance {
    pub fn new(hit_id: isize, bounds: &AABB, transform: Mat4) -> Instance {
        let inverse = transform.inverse();
        let normal_transform = inverse.transpose();
        let mut bounds = bounds.transformed(transform);

        bounds.left_first = hit_id as i32;

        Instance {
            bounds,
            transform,
            inverse,
            normal_transform,
        }
    }

    pub fn get_transform(&self) -> Mat4 { self.transform }

    pub fn set_transform(&mut self, transform: Mat4) {
        self.inverse = transform.inverse();
        let new_transform = transform * self.inverse;
        self.bounds = self.bounds.transformed(new_transform);
    }

    #[inline(always)]
    pub fn intersects(&self, origin: Vec3, direction: Vec3, t_max: f32) -> Option<(Vec3, Vec3)> {
        if self.bounds.intersect(origin, Vec3::one() / direction, t_max).is_none() {
            return None;
        }

        let new_origin = self.inverse * origin.extend(1.0);
        let new_direction = self.inverse * direction.extend(0.0);
        Some((new_origin.truncate(), new_direction.truncate()))
    }

    #[inline(always)]
    pub fn transform_hit(&self, hit: HitRecord) -> HitRecord {
        let p = self.inverse * Vec3::from(hit.p).extend(1.0);
        let normal = self.normal_transform * Vec3::from(hit.normal).extend(0.0);

        HitRecord {
            p: p.truncate().into(),
            normal: normal.truncate().into(),
            ..hit
        }
    }

    #[inline(always)]
    pub fn get_hit_id(&self) -> usize {
        self.bounds.left_first as usize
    }
}

impl Bounds for Instance {
    fn bounds(&self) -> AABB {
        self.bounds.clone()
    }
}
