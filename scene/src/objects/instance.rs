use crate::objects::*;
use bvh::aabb::Bounds;
use bvh::{RayPacket4, AABB};

/// Instance
/// Takes in a bounding box and transform and transforms to and from object local space.
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

    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    pub fn set_transform(&mut self, transform: Mat4) {
        self.inverse = transform.inverse();
        let new_transform = transform * self.inverse;
        self.bounds = self.bounds.transformed(new_transform);
    }

    #[inline(always)]
    pub fn intersects(&self, ray: Ray, t_max: f32) -> Option<(Vec3, Vec3)> {
        let (origin, direction) = ray.into();
        if self
            .bounds
            .intersect(origin, Vec3::one() / direction, t_max)
            .is_none()
        {
            return None;
        }

        let new_origin = self.inverse * origin.extend(1.0);
        let new_direction = self.inverse * direction.extend(0.0);
        Some((new_origin.truncate(), new_direction.truncate()))
    }

    #[inline(always)]
    pub fn intersects4(&self, packet: &RayPacket4) -> Option<RayPacket4> {
        let one = Vec4::one();
        if self
            .bounds
            .intersect4(
                packet,
                one / Vec4::from(packet.direction_x),
                one / Vec4::from(packet.direction_y),
                one / Vec4::from(packet.direction_z),
            )
            .is_none()
        {
            return None;
        }

        let origin_x = Vec4::from(packet.origin_x);
        let origin_y = Vec4::from(packet.origin_y);
        let origin_z = Vec4::from(packet.origin_z);

        let direction_x = Vec4::from(packet.direction_x);
        let direction_y = Vec4::from(packet.direction_y);
        let direction_z = Vec4::from(packet.direction_z);

        let matrix_cols = self.inverse.to_cols_array();

        // Col 0
        let m0_0 = Vec4::from([matrix_cols[0]; 4]);
        let m0_1 = Vec4::from([matrix_cols[1]; 4]);
        let m0_2 = Vec4::from([matrix_cols[2]; 4]);

        // Col 1
        let m1_0 = Vec4::from([matrix_cols[4]; 4]);
        let m1_1 = Vec4::from([matrix_cols[5]; 4]);
        let m1_2 = Vec4::from([matrix_cols[6]; 4]);

        // Col 2
        let m2_0 = Vec4::from([matrix_cols[8]; 4]);
        let m2_1 = Vec4::from([matrix_cols[9]; 4]);
        let m2_2 = Vec4::from([matrix_cols[10]; 4]);

        // Col 3
        let m3_0 = Vec4::from([matrix_cols[12]; 4]);
        let m3_1 = Vec4::from([matrix_cols[13]; 4]);
        let m3_2 = Vec4::from([matrix_cols[14]; 4]);

        let mut new_origin_x = m0_0 * origin_x;
        let mut new_origin_y = m0_1 * origin_x;
        let mut new_origin_z = m0_2 * origin_x;

        let mut new_direction_x = m0_0 * direction_x;
        let mut new_direction_y = m0_1 * direction_x;
        let mut new_direction_z = m0_2 * direction_x;

        new_origin_x += m1_0 * origin_y;
        new_origin_y += m1_1 * origin_y;
        new_origin_z += m1_2 * origin_y;

        new_direction_x += m1_0 * direction_y;
        new_direction_y += m1_1 * direction_y;
        new_direction_z += m1_2 * direction_y;

        new_origin_x += m2_0 * origin_z;
        new_origin_y += m2_1 * origin_z;
        new_origin_z += m2_2 * origin_z;

        new_direction_x += m2_0 * direction_z;
        new_direction_y += m2_1 * direction_z;
        new_direction_z += m2_2 * direction_z;

        // Only origin needs to be translated
        new_origin_x += m3_0;
        new_origin_y += m3_1;
        new_origin_z += m3_2;

        let new_packet = RayPacket4 {
            origin_x: new_origin_x.into(),
            origin_y: new_origin_y.into(),
            origin_z: new_origin_z.into(),
            direction_x: new_direction_x.into(),
            direction_y: new_direction_y.into(),
            direction_z: new_direction_z.into(),
            ..(*packet).clone()
        };

        Some(new_packet)
    }

    #[inline(always)]
    pub fn transform_hit(&self, hit: HitRecord) -> HitRecord {
        let p = self.inverse * Vec3::from(hit.p).extend(1.0);
        let normal = self.normal_transform * Vec3::from(hit.normal).extend(0.0);

        HitRecord {
            p: p.truncate().into(),
            normal: normal.truncate().normalize().into(),
            ..hit
        }
    }

    pub fn transform_ray(&self, ray: Ray) -> Ray {
        let (origin, direction) = ray.into();
        let new_origin: Vec4 = self.inverse * origin.extend(1.0);
        let new_direction: Vec4 = self.inverse * direction.extend(0.0);
        (new_origin.truncate(), new_direction.truncate()).into()
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
