use rfw_math::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{InstanceHandle2D, InstanceHandle3D};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Flags {
    bits: bitvec::prelude::BitVec,
}

#[allow(dead_code)]
impl Flags {
    pub fn new() -> Flags {
        Self::default()
    }

    pub fn set_flag<T: Into<u8>>(&mut self, flag: T) {
        let i = flag.into() as u8 as usize;
        self.bits.set(i, true);
    }

    pub fn unset_flag<T: Into<u8>>(&mut self, flag: T) {
        let i = flag.into() as u8 as usize;
        self.bits.set(i, false);
    }

    pub fn has_flag<T: Into<u8>>(&self, flag: T) -> bool {
        match self.bits.get(flag.into() as u8 as usize) {
            None => false,
            Some(flag) => *flag,
        }
    }

    pub fn any(&self) -> bool {
        self.bits.any()
    }

    pub fn clear(&mut self) {
        self.bits.set_all(false);
    }
}

impl Default for Flags {
    fn default() -> Self {
        let mut bits = bitvec::prelude::BitVec::new();
        bits.resize(32, false);
        Self { bits }
    }
}

pub trait HasMatrix {
    fn udpate_matrix(&mut self, matrix: Mat4);
}

impl HasMatrix for InstanceHandle2D {
    fn udpate_matrix(&mut self, matrix: Mat4) {
        self.set_matrix(matrix);
    }
}

impl HasMatrix for InstanceHandle3D {
    fn udpate_matrix(&mut self, matrix: Mat4) {
        self.set_matrix(matrix);
    }
}

#[derive(Debug)]
pub struct Transform<'a, T: HasMatrix> {
    pub(crate) translation: Vec3,
    pub(crate) rotation: Quat,
    pub(crate) scale: Vec3,
    pub(crate) handle: &'a mut T,
    pub(crate) changed: bool,
}

impl<T: HasMatrix> Transform<'_, T> {
    pub fn translate_x(&mut self, offset: f32) {
        self.translation.x += offset;
        self.changed = true;
    }

    pub fn translate_y(&mut self, offset: f32) {
        self.translation.y += offset;
        self.changed = true;
    }

    pub fn translate_z(&mut self, offset: f32) {
        self.translation.z += offset;
        self.changed = true;
    }

    pub fn translate<V: Into<[f32; 3]>>(&mut self, offset: V) {
        let offset: [f32; 3] = offset.into();
        self.translation += Vec3::from(offset);
        self.changed = true;
    }

    pub fn rotate_x(&mut self, radians: f32) {
        self.rotation *= Quat::from_rotation_x(radians);
        self.changed = true;
    }

    pub fn rotate_y(&mut self, radians: f32) {
        self.rotation *= Quat::from_rotation_y(radians);
        self.changed = true;
    }

    pub fn rotate_z(&mut self, radians: f32) {
        self.rotation *= Quat::from_rotation_z(radians);
        self.changed = true;
    }

    pub fn rotate<V: Into<[f32; 3]>>(&mut self, degrees: V) {
        let degrees: [f32; 3] = degrees.into();
        self.rotation *= Quat::from_rotation_x(degrees[0].to_radians());
        self.rotation *= Quat::from_rotation_y(degrees[1].to_radians());
        self.rotation *= Quat::from_rotation_z(degrees[2].to_radians());
        self.changed = true;
    }

    pub fn scale_x(&mut self, offset: f32) {
        self.scale.x *= offset;
        self.changed = true;
    }

    pub fn scale_y(&mut self, offset: f32) {
        self.scale.y *= offset;
        self.changed = true;
    }

    pub fn scale_z(&mut self, offset: f32) {
        self.scale.z *= offset;
        self.changed = true;
    }

    pub fn scale<V: Into<[f32; 3]>>(&mut self, offset: V) {
        self.scale *= Vec3::from(offset.into());
        self.changed = true;
    }

    pub fn get_translation(&self) -> Vec3 {
        self.translation
    }

    pub fn get_rotation(&self) -> Quat {
        self.rotation
    }

    pub fn get_scale(&self) -> Vec3 {
        self.scale
    }

    pub fn set_translation(&mut self, translation: Vec3) {
        self.translation = translation;
        self.changed = true;
    }

    pub fn set_rotation(&mut self, rotation: Quat) {
        self.rotation = rotation;
        self.changed = true;
    }

    pub fn set_scale(&mut self, scale: Vec3) {
        self.scale = scale;
        self.changed = true;
    }
}

impl<T: HasMatrix> Drop for Transform<'_, T> {
    fn drop(&mut self) {
        if !self.changed {
            return;
        }

        self.handle
            .udpate_matrix(Mat4::from_scale_rotation_translation(
                self.scale,
                self.rotation,
                self.translation,
            ));
    }
}
