use glam::*;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
pub struct AABB {
    pub min: [f32; 3],
    pub left_first: i32,
    pub max: [f32; 3],
    pub count: i32,
}

impl Display for AABB {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let min = Vec3::from(self.min);
        let max = Vec3::from(self.max);

        write!(f, "(min: ({}, {}, {}), left_first: {}, max: ({}, {}, {}), count: {})",
               min.x(), min.y(), min.z(), self.left_first, max.x(), max.y(), max.z(), self.count)
    }
}

#[allow(dead_code)]
impl AABB {
    pub fn new() -> AABB {
        AABB {
            min: [1e34; 3],
            left_first: -1,
            max: [-1e34; 3],
            count: -1,
        }
    }

    pub fn intersect(&self, origin: Vec3, dir_inverse: Vec3, t: f32) -> Option<(f32, f32)> {
        let min = Vec3::from(self.min);
        let max = Vec3::from(self.max);

        let t1 = (min - origin) * dir_inverse;
        let t2 = (max - origin) * dir_inverse;

        let t_min = t1.min(t2);
        let t_max = t1.max(t2);

        let t_min = t_min.x().max(t_min.y().max(t_min.z()));
        let t_max = t_max.x().min(t_max.y().min(t_max.z()));

        if t_max > t_min && t_min < t {
            return Some((t_min, t_max));
        }

        None
    }

    pub fn grow(&mut self, pos: Vec3) {
        let min = Vec3::from(self.min);
        let max = Vec3::from(self.max);
        let min = min.min(pos);
        let max = max.max(pos);

        for i in 0..3 {
            self.min[i] = min[i];
            self.max[i] = max[i];
        }
    }

    pub fn grow_bb(&mut self, aabb: &AABB) {
        for i in 0..3 {
            self.min[i] = self.min[i].min(aabb.min[i]);
            self.max[i] = self.max[i].max(aabb.max[i]);
        }
    }

    pub fn offset_by(&mut self, delta: f32) {
        let delta = Vec3::from([delta; 3]);

        let min = Vec3::from(self.min);
        let max = Vec3::from(self.max);
        let min = min - delta;
        let max = max + delta;

        for i in 0..3 {
            self.min[i] = min[i];
            self.max[i] = max[i];
        }
    }

    pub fn union_of(&self, bb: &AABB) -> AABB {
        let min = Vec3::from(self.min).min(bb.min.into());
        let max = Vec3::from(self.max).max(bb.max.into());

        let mut new_min = [0.0; 3];
        let mut new_max = [0.0; 3];

        for i in 0..3 {
            new_min[i] = min[i];
            new_max[i] = max[i];
        }

        AABB {
            min: new_min,
            left_first: -1,
            max: new_max,
            count: -1,
        }
    }

    pub fn intersection(&self, bb: &AABB) -> AABB {
        let min = Vec3::from(self.min).max(Vec3::from(bb.min));
        let max = Vec3::from(self.max).min(Vec3::from(bb.max));

        let mut new_min = [0.0; 3];
        let mut new_max = [0.0; 3];

        for i in 0..3 {
            new_min[i] = min[i].max(bb.min[i]);
            new_max[i] = max[i].min(bb.max[i]);
        }

        AABB {
            min: new_min,
            left_first: -1,
            max: new_max,
            count: -1,
        }
    }

    pub fn volume(&self) -> f32 {
        let length = Vec3::from(self.max) - Vec3::from(self.min);
        return length.x() * length.y() * length.z();
    }

    pub fn center(&self) -> Vec3 {
        (Vec3::from(self.min) + Vec3::from(self.max)) * 0.5
    }

    pub fn area(&self) -> f32 {
        let e = Vec3::from(self.max) - Vec3::from(self.min);
        let value: f32 = e.x() * e.y() + e.x() * e.z() + e.y() * e.z();

        0.0_f32.max(value)
    }

    pub fn lengths(&self) -> Vec3 {
        Vec3::from(self.max) - Vec3::from(self.min)
    }

    pub fn longest_axis(&self) -> usize {
        let mut a: usize = 0;
        if self.extend(1) > self.extend(0) {
            a = 1;
        }
        if self.extend(2) > self.extend(a) {
            a = 2
        }
        a
    }

    pub fn extend(&self, axis: usize) -> f32 {
        self.max[axis] - self.min[axis]
    }

    pub fn transform(&mut self, transform: Mat4) {
        let p1 = transform * Vec4::new(self.min[0], self.min[1], self.min[2], 1.0);
        let p5 = transform * Vec4::new(self.max[0], self.max[1], self.max[2], 1.0);
        let p2 = transform * Vec4::new(self.max[0], self.min[1], self.min[2], 1.0);
        let p3 = transform * Vec4::new(self.min[0], self.max[1], self.max[2], 1.0);
        let p4 = transform * Vec4::new(self.min[0], self.min[1], self.max[2], 1.0);
        let p6 = transform * Vec4::new(self.max[0], self.max[1], self.min[2], 1.0);
        let p7 = transform * Vec4::new(self.min[0], self.max[1], self.min[2], 1.0);
        let p8 = transform * Vec4::new(self.max[0], self.min[1], self.max[2], 1.0);

        let p1 = Vec3::new(p1.x(), p1.y(), p1.z());
        let p5 = Vec3::new(p5.x(), p5.y(), p5.z());
        let p2 = Vec3::new(p2.x(), p2.y(), p2.z());
        let p3 = Vec3::new(p3.x(), p3.y(), p3.z());
        let p4 = Vec3::new(p4.x(), p4.y(), p4.z());
        let p6 = Vec3::new(p6.x(), p6.y(), p6.z());
        let p7 = Vec3::new(p7.x(), p7.y(), p7.z());
        let p8 = Vec3::new(p8.x(), p8.y(), p8.z());

        let mut transformed = AABB::new();
        transformed.grow(p1);
        transformed.grow(p2);
        transformed.grow(p3);
        transformed.grow(p4);
        transformed.grow(p5);
        transformed.grow(p6);
        transformed.grow(p7);
        transformed.grow(p8);

        transformed.offset_by(crate::constants::EPSILON);
        *self = AABB {
            min: transformed.min,
            max: transformed.max,
            ..(*self).clone()
        }
    }

    pub fn transformed(&self, transform: Mat4) -> AABB {
        let p1 = transform * Vec3::new(self.min[0], self.min[1], self.min[2]).extend(1.0);
        let p5 = transform * Vec3::new(self.max[0], self.max[1], self.max[2]).extend(1.0);
        let p2 = transform * Vec3::new(self.max[0], self.min[1], self.min[2]).extend(1.0);
        let p3 = transform * Vec3::new(self.min[0], self.max[1], self.max[2]).extend(1.0);
        let p4 = transform * Vec3::new(self.min[0], self.min[1], self.max[2]).extend(1.0);
        let p6 = transform * Vec3::new(self.max[0], self.max[1], self.min[2]).extend(1.0);
        let p7 = transform * Vec3::new(self.min[0], self.max[1], self.min[2]).extend(1.0);
        let p8 = transform * Vec3::new(self.max[0], self.min[1], self.max[2]).extend(1.0);

        let mut transformed = AABB::new();
        transformed.grow(p1.truncate());
        transformed.grow(p2.truncate());
        transformed.grow(p3.truncate());
        transformed.grow(p4.truncate());
        transformed.grow(p5.truncate());
        transformed.grow(p6.truncate());
        transformed.grow(p7.truncate());
        transformed.grow(p8.truncate());

        transformed.offset_by(crate::constants::EPSILON);

        transformed
    }
}