use glam::*;
use crate::objects::*;
use crate::bvh::AABB;
use std::sync::Arc;

pub struct Instance {
    bounds: AABB,
    transform: Mat4,
    inverse: Mat4,
    normal_transform: Mat4,
    object: Arc<Box<dyn Intersect>>,
}

impl Instance {
    pub fn new(object: Arc<Box<dyn Intersect>>, transform: Mat4) -> Instance {
        let inverse = transform.inverse();
        let normal_transform = inverse.transpose();
        let bounds = object.bounds().transformed(transform);

        Instance {
            bounds,
            transform,
            inverse,
            normal_transform,
            object,
        }
    }
}

impl Intersect for Instance {
    fn occludes(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> bool {
        self.object.occludes(origin, direction, t_min, t_max)
    }

    fn intersect(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<HitRecord> {
        if self.bounds.intersect(origin,  Vec3::one() / direction, t_max).is_none() {
            return None;
        }

        let new_origin = self.inverse * Vec4::new(origin.x(), origin.y(), origin.z(), 1.0);
        let new_direction = self.inverse * Vec4::new(direction.x(), direction.y(), direction.z(), 0.0);
        let new_origin = Vec3::new(new_origin.x(), new_origin.y(), new_origin.z());
        let new_direction = Vec3::new(new_direction.x(), new_direction.y(), new_direction.z());

        if let Some(mut hit) = self.object.intersect(new_origin, new_direction, t_min, t_max) {
            let p = self.inverse * Vec4::new(hit.p.x(), hit.p.y(), hit.p.z(), 1.0);
            let p = Vec3::new(p.x(), p.y(), p.z());

            let normal = self.normal_transform * Vec4::new(hit.normal.x(), hit.normal.y(), hit.normal.z(), 1.0);
            let normal = Vec3::new(normal.x(), normal.y(), normal.z());

            hit.p = p;
            hit.normal = normal;
            return Some(hit);
        }

        None
    }

    fn intersect_t(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<f32> {
        self.object.intersect_t(origin, direction, t_min, t_max)
    }

    fn bounds(&self) -> AABB {
        self.bounds.clone()
    }
}