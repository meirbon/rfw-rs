use crate::objects::*;

use rtbvh::aabb::Bounds;
use rtbvh::{Ray, RayPacket4, AABB};

use std::fmt::Display;

#[cfg(feature = "object_caching")]
use serde::{Deserialize, Serialize};
use glam::f32::Vec3;

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ObjectRef {
    None,
    Static(u32),
    Animated(u32),
}

impl std::fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ObjectRef({})", match self {
            ObjectRef::None => String::from("None"),
            ObjectRef::Static(hit_id) => format!("Static({})", hit_id),
            ObjectRef::Animated(hit_id) => format!("Animated({})", hit_id),
        })
    }
}

/// Instance
/// Takes in a bounding box and transform and transforms to and from object local space.
#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
pub struct Instance {
    original_bounds: AABB,
    bounds: AABB,
    transform: [f32; 16],
    inverse: [f32; 16],
    normal_transform: [f32; 16],
    translation: Vec3,
    scaling: Vec3,
    rotation_x: f32,
    rotation_y: f32,
    rotation_z: f32,
    pub object_id: ObjectRef,
}

impl Display for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Instance {{ original_bounds: {}, bounds: {}, hit_id: {}, transform: {}, inverse: {}, normal_transform: {} }}",
               self.original_bounds, self.bounds, self.object_id, Mat4::from_cols_array(&self.transform), Mat4::from_cols_array(&self.inverse), Mat4::from_cols_array(&self.normal_transform)
        )
    }
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            original_bounds: AABB::empty(),
            bounds: AABB::empty(),
            transform: [0.0; 16],
            inverse: [0.0; 16],
            normal_transform: [0.0; 16],
            translation: Vec3::zero(),
            scaling: Vec3::one(),
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            object_id: ObjectRef::None,
        }
    }
}

#[allow(dead_code)]
impl Instance {
    pub fn new(object_id: ObjectRef, bounds: &AABB) -> Instance {
        let transform = glam::Mat4::identity();
        let inverse = transform.inverse();

        let normal_transform = inverse.transpose();
        let transformed_bounds = bounds.transformed(transform);

        Instance {
            original_bounds: bounds.clone(),
            bounds: transformed_bounds,
            transform: transform.to_cols_array(),
            inverse: inverse.to_cols_array(),
            normal_transform: normal_transform.to_cols_array(),
            object_id,
            ..Instance::default()
        }
    }

    pub fn set_bounds(&mut self, bounds: AABB) {
        self.original_bounds = bounds;
        self.bounds = self.original_bounds.transformed(self.get_transform());
    }

    pub fn local_bounds(&self) -> AABB {
        self.original_bounds.clone()
    }

    pub fn get_transform(&self) -> glam::Mat4 {
        glam::Mat4::from_cols_array(&self.transform)
    }

    pub fn get_inverse_transform(&self) -> glam::Mat4 {
        glam::Mat4::from_cols_array(&self.inverse)
    }

    pub fn get_normal_transform(&self) -> glam::Mat4 {
        glam::Mat4::from_cols_array(&self.normal_transform)
    }

    pub fn set_transform(&mut self, transform: glam::Mat4) {
        let inverse = transform.inverse();
        self.transform = transform.to_cols_array();
        self.inverse = inverse.to_cols_array();
        self.normal_transform = inverse.transpose().to_cols_array();
        self.bounds = self.original_bounds.transformed(transform);
    }

    #[inline(always)]
    pub fn transform_vertex(&self, vertex: glam::Vec3) -> glam::Vec3 {
        (self.get_transform() * vertex.extend(1.0)).truncate()
    }

    #[inline(always)]
    pub fn transform(&self, ray: Ray) -> (glam::Vec3, glam::Vec3) {
        let (origin, direction) = ray.into();
        let inverse = self.get_inverse_transform();
        let new_origin = inverse * origin.extend(1.0);
        let new_direction = inverse * direction.extend(0.0);
        (new_origin.truncate(), new_direction.truncate())
    }

    #[inline(always)]
    pub fn transform4(&self, packet: &RayPacket4) -> RayPacket4 {
        let origin_x = glam::Vec4::from(packet.origin_x);
        let origin_y = glam::Vec4::from(packet.origin_y);
        let origin_z = glam::Vec4::from(packet.origin_z);

        let direction_x = glam::Vec4::from(packet.direction_x);
        let direction_y = glam::Vec4::from(packet.direction_y);
        let direction_z = glam::Vec4::from(packet.direction_z);

        let matrix_cols = self.inverse;

        // Col 0
        let m0_0 = glam::Vec4::from([matrix_cols[0]; 4]);
        let m0_1 = glam::Vec4::from([matrix_cols[1]; 4]);
        let m0_2 = glam::Vec4::from([matrix_cols[2]; 4]);

        // Col 1
        let m1_0 = glam::Vec4::from([matrix_cols[4]; 4]);
        let m1_1 = glam::Vec4::from([matrix_cols[5]; 4]);
        let m1_2 = glam::Vec4::from([matrix_cols[6]; 4]);

        // Col 2
        let m2_0 = glam::Vec4::from([matrix_cols[8]; 4]);
        let m2_1 = glam::Vec4::from([matrix_cols[9]; 4]);
        let m2_2 = glam::Vec4::from([matrix_cols[10]; 4]);

        // Col 3
        let m3_0 = glam::Vec4::from([matrix_cols[12]; 4]);
        let m3_1 = glam::Vec4::from([matrix_cols[13]; 4]);
        let m3_2 = glam::Vec4::from([matrix_cols[14]; 4]);

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

        new_packet
    }

    #[inline(always)]
    pub fn transform_hit(&self, hit: HitRecord) -> HitRecord {
        let normal_transform = self.get_normal_transform();
        let normal = normal_transform * glam::Vec3::from(hit.normal).extend(0.0);

        HitRecord {
            normal: normal.truncate().normalize().into(),
            ..hit
        }
    }

    #[inline(always)]
    pub fn transform_hit4(&self, hit: HitRecord4) -> HitRecord4 {
        let inverse = self.get_inverse_transform();
        let normal_transform = self.get_normal_transform();
        let one = Vec4::one();

        let (p_x, p_y, p_z) = {
            let matrix_cols = inverse.to_cols_array();
            // Col 0
            let m0_0 = glam::Vec4::from([matrix_cols[0]; 4]);
            let m0_1 = glam::Vec4::from([matrix_cols[1]; 4]);
            let m0_2 = glam::Vec4::from([matrix_cols[2]; 4]);

            // Col 1
            let m1_0 = glam::Vec4::from([matrix_cols[4]; 4]);
            let m1_1 = glam::Vec4::from([matrix_cols[5]; 4]);
            let m1_2 = glam::Vec4::from([matrix_cols[6]; 4]);

            // Col 2
            let m2_0 = glam::Vec4::from([matrix_cols[8]; 4]);
            let m2_1 = glam::Vec4::from([matrix_cols[9]; 4]);
            let m2_2 = glam::Vec4::from([matrix_cols[10]; 4]);

            // Col 3
            let m3_0 = glam::Vec4::from([matrix_cols[12]; 4]);
            let m3_1 = glam::Vec4::from([matrix_cols[13]; 4]);
            let m3_2 = glam::Vec4::from([matrix_cols[14]; 4]);

            let p_x = Vec4::from(hit.p_x);
            let p_y = Vec4::from(hit.p_y);
            let p_z = Vec4::from(hit.p_z);

            let mut new_p_x = m0_0 * p_x;
            let mut new_p_y = m0_1 * p_x;
            let mut new_p_z = m0_2 * p_x;

            new_p_x += m1_0 * p_y;
            new_p_y += m1_1 * p_y;
            new_p_z += m1_2 * p_y;

            new_p_x += m2_0 * p_z;
            new_p_y += m2_1 * p_z;
            new_p_z += m2_2 * p_z;

            new_p_x += m3_0 * one;
            new_p_y += m3_1 * one;
            new_p_z += m3_2 * one;

            (new_p_x, new_p_y, new_p_z)
        };

        let (n_x, n_y, n_z) = {
            let matrix_cols = normal_transform.to_cols_array();
            // Col 0
            let m0_0 = glam::Vec4::from([matrix_cols[0]; 4]);
            let m0_1 = glam::Vec4::from([matrix_cols[1]; 4]);
            let m0_2 = glam::Vec4::from([matrix_cols[2]; 4]);

            // C    ol 1
            let m1_0 = glam::Vec4::from([matrix_cols[4]; 4]);
            let m1_1 = glam::Vec4::from([matrix_cols[5]; 4]);
            let m1_2 = glam::Vec4::from([matrix_cols[6]; 4]);

            // Col 2
            let m2_0 = glam::Vec4::from([matrix_cols[8]; 4]);
            let m2_1 = glam::Vec4::from([matrix_cols[9]; 4]);
            let m2_2 = glam::Vec4::from([matrix_cols[10]; 4]);

            let n_x = Vec4::from(hit.normal_x);
            let n_y = Vec4::from(hit.normal_y);
            let n_z = Vec4::from(hit.normal_z);

            let mut new_n_x = m0_0 * n_x;
            let mut new_n_y = m0_1 * n_x;
            let mut new_n_z = m0_2 * n_x;

            new_n_x += m1_0 * n_y;
            new_n_y += m1_1 * n_y;
            new_n_z += m1_2 * n_y;

            new_n_x += m2_0 * n_z;
            new_n_y += m2_1 * n_z;
            new_n_z += m2_2 * n_z;

            (new_n_x, new_n_y, new_n_z)
        };

        HitRecord4 {
            p_x: p_x.into(),
            p_y: p_y.into(),
            p_z: p_z.into(),
            normal_x: n_x.into(),
            normal_y: n_y.into(),
            normal_z: n_z.into(),
            ..hit
        }
    }

    #[inline(always)]
    pub fn transform_ray(&self, ray: Ray) -> Ray {
        let inverse = self.get_inverse_transform();

        let (origin, direction) = ray.into();
        let new_origin: glam::Vec4 = inverse * origin.extend(1.0);
        let new_direction: glam::Vec4 = inverse * direction.extend(0.0);
        (new_origin.truncate(), new_direction.truncate()).into()
    }

    pub fn set_translation<T: Into<[f32; 3]>>(&mut self, t: T) {
        self.translation = Vec3::from(t.into());
    }

    pub fn translate_x(&mut self, offset: f32) {
        self.translation += Vec3::new(offset, 0.0, 0.0);
    }

    pub fn translate_y(&mut self, offset: f32) {
        self.translation += Vec3::new(0.0, offset, 0.0);
    }

    pub fn translate_z(&mut self, offset: f32) {
        self.translation += Vec3::new(0.0, 0.0, offset);
    }

    pub fn set_rotation<T: Into<[f32; 3]>>(&mut self, r: T) {
        let r: [f32; 3] = r.into();
        self.rotation_x = r[0];
        self.rotation_y = r[1];
        self.rotation_z = r[2];
    }

    pub fn rotate_x(&mut self, degrees: f32) {
        self.rotation_x = (self.rotation_x + degrees.to_radians()) % (std::f32::consts::PI * 2.0);
    }

    pub fn rotate_y(&mut self, degrees: f32) {
        self.rotation_y = (self.rotation_y + degrees.to_radians()) % (std::f32::consts::PI * 2.0);
    }

    pub fn rotate_z(&mut self, degrees: f32) {
        self.rotation_z = (self.rotation_z + degrees.to_radians()) % (std::f32::consts::PI * 2.0);
    }

    pub fn set_scale<T: Into<[f32; 3]>>(&mut self, scale: T) {
        self.scaling = Vec3::from(scale.into());
    }

    pub fn scale<T: Into<[f32; 3]>>(&mut self, scale: T) {
        let scale: [f32; 3] = scale.into();
        let scale: Vec3 = Vec3::from(scale).max(Vec3::splat(0.001));
        self.scaling *= scale;
    }

    pub fn scale_x(&mut self, scale: f32) {
        let scale = scale.max(0.001);
        self.scaling[0] *= scale;
    }

    pub fn scale_y(&mut self, scale: f32) {
        let scale = scale.max(0.001);
        self.scaling[1] *= scale;
    }

    pub fn scale_z(&mut self, scale: f32) {
        let scale = scale.max(0.001);
        self.scaling[2] *= scale;
    }

    /// Returns translation in [x, y, z]
    pub fn get_translation(&self) -> [f32; 3] {
        self.translation.into()
    }

    /// Returns scale in [x, y, z]
    pub fn get_scale(&self) -> [f32; 3] {
        self.scaling.into()
    }

    /// Returns rotation as quaternion in [x, y, z, w]
    pub fn get_rotation(&self) -> [f32; 4] {
        let mut quat = glam::Quat::identity();
        if self.rotation_x.abs() > 0.0001 {
            quat *= glam::Quat::from_axis_angle(Vec3::new(1.0, 0.0, 0.0), self.rotation_x);
        }
        if self.rotation_y.abs() > 0.0001 {
            quat *= glam::Quat::from_axis_angle(Vec3::new(0.0, 1.0, 0.0), self.rotation_y);
        }
        if self.rotation_z.abs() > 0.0001 {
            quat *= glam::Quat::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), self.rotation_z);
        }

        quat.into()
    }

    /// Returns rotation as radian euler angles in [x, y, z]
    pub fn get_euler_angles(&self) -> [f32; 3] {
        [self.rotation_x, self.rotation_y, self.rotation_z]
    }

    pub fn update_transform(&mut self) {
        let rotation_x = glam::Quat::from_axis_angle(Vec3::unit_x(), self.rotation_x);
        let rotation_y = glam::Quat::from_axis_angle(Vec3::unit_y(), self.rotation_y);
        let rotation_z = glam::Quat::from_axis_angle(Vec3::unit_z(), self.rotation_z);

        self.set_transform(glam::Mat4::from_scale_rotation_translation(
            self.scaling.into(),
            rotation_x * rotation_y * rotation_z,
            self.translation.into(),
        ));
    }
}

impl Bounds for Instance {
    fn bounds(&self) -> AABB {
        self.original_bounds.transformed(self.get_transform())
    }
}

#[cfg(feature = "object_caching")]
impl<'a> SerializableObject<'a, Instance> for Instance {
    fn serialize_object<S: AsRef<std::path::Path>>(
        &self,
        path: S,
        _: &crate::MaterialList,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        let encoded: Vec<u8> = bincode::serialize(self)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(encoded.as_ref())?;
        Ok(())
    }

    fn deserialize_object<S: AsRef<std::path::Path>>(
        path: S,
        _: &mut crate::MaterialList,
    ) -> Result<Instance, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let object: Self = bincode::deserialize_from(reader)?;
        Ok(object)
    }
}
