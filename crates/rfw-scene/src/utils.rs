use rfw_math::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{InstanceHandle2D, InstanceHandle3D};

pub trait HasMatrix {
    fn update(&mut self, t: Vec3, r: Quat, s: Vec3);
}

pub trait HasTranslation: HasMatrix {}
impl HasTranslation for InstanceHandle2D {}
impl HasTranslation for InstanceHandle3D {}

pub trait HasRotation: HasMatrix {}
impl HasRotation for InstanceHandle2D {}
impl HasRotation for InstanceHandle3D {}

pub trait HasScale: HasMatrix {}
impl HasScale for InstanceHandle2D {}
impl HasScale for InstanceHandle3D {}

impl HasMatrix for InstanceHandle2D {
    fn update(&mut self, t: Vec3, r: Quat, s: Vec3) {
        self.set_matrix(Mat4::from_scale_rotation_translation(s, r, t));
    }
}

impl HasMatrix for InstanceHandle3D {
    fn update(&mut self, t: Vec3, r: Quat, s: Vec3) {
        self.set_matrix(Mat4::from_scale_rotation_translation(s, r, t));
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
    pub fn translate_x(&mut self, offset: f32) -> &mut Self
    where
        T: HasTranslation,
    {
        self.translation.x += offset;
        self.changed = true;
        self
    }

    pub fn translate_y(&mut self, offset: f32) -> &mut Self
    where
        T: HasTranslation,
    {
        self.translation.y += offset;
        self.changed = true;
        self
    }

    pub fn translate_z(&mut self, offset: f32) -> &mut Self
    where
        T: HasTranslation,
    {
        self.translation.z += offset;
        self.changed = true;
        self
    }

    pub fn translate<V: Into<[f32; 3]>>(&mut self, offset: V) -> &mut Self
    where
        T: HasTranslation,
    {
        let offset: [f32; 3] = offset.into();
        self.translation += Vec3::from(offset);
        self.changed = true;
        self
    }

    pub fn rotate_x(&mut self, radians: f32) -> &mut Self
    where
        T: HasRotation,
    {
        self.rotation *= Quat::from_rotation_x(radians);
        self.changed = true;
        self
    }

    pub fn rotate_y(&mut self, radians: f32) -> &mut Self
    where
        T: HasRotation,
    {
        self.rotation *= Quat::from_rotation_y(radians);
        self.changed = true;
        self
    }

    pub fn rotate_z(&mut self, radians: f32) -> &mut Self
    where
        T: HasRotation,
    {
        self.rotation *= Quat::from_rotation_z(radians);
        self.changed = true;
        self
    }

    pub fn rotate<V: Into<[f32; 3]>>(&mut self, degrees: V) -> &mut Self
    where
        T: HasRotation,
    {
        let degrees: [f32; 3] = degrees.into();
        self.rotation *= Quat::from_rotation_x(degrees[0].to_radians());
        self.rotation *= Quat::from_rotation_y(degrees[1].to_radians());
        self.rotation *= Quat::from_rotation_z(degrees[2].to_radians());
        self.changed = true;
        self
    }

    pub fn scale_x(&mut self, offset: f32) -> &mut Self
    where
        T: HasScale,
    {
        self.scale.x *= offset;
        self.changed = true;
        self
    }

    pub fn scale_y(&mut self, offset: f32) -> &mut Self
    where
        T: HasScale,
    {
        self.scale.y *= offset;
        self.changed = true;
        self
    }

    pub fn scale_z(&mut self, offset: f32) -> &mut Self
    where
        T: HasScale,
    {
        self.scale.z *= offset;
        self.changed = true;
        self
    }

    pub fn scale<V: Into<[f32; 3]>>(&mut self, offset: V) -> &mut Self
    where
        T: HasScale,
    {
        self.scale *= Vec3::from(offset.into());
        self.changed = true;
        self
    }

    pub fn get_translation(&self) -> Vec3
    where
        T: HasTranslation,
    {
        self.translation
    }

    pub fn get_rotation(&self) -> Quat
    where
        T: HasRotation,
    {
        self.rotation
    }

    pub fn get_scale(&self) -> Vec3
    where
        T: HasScale,
    {
        self.scale
    }

    pub fn set_translation(&mut self, translation: Vec3) -> &mut Self
    where
        T: HasTranslation,
    {
        self.translation = translation;
        self.changed = true;
        self
    }

    pub fn set_rotation(&mut self, rotation: Quat) -> &mut Self
    where
        T: HasRotation,
    {
        self.rotation = rotation;
        self.changed = true;
        self
    }

    pub fn set_scale(&mut self, scale: Vec3) -> &mut Self
    where
        T: HasScale,
    {
        self.scale = scale;
        self.changed = true;
        self
    }

    pub fn set_matrix(&mut self, matrix: Mat4) -> &mut Self
    where
        T: HasTranslation + HasRotation + HasScale,
    {
        let (s, r, t) = matrix.to_scale_rotation_translation();
        self.scale = s;
        self.rotation = r;
        self.translation = t;
        self.changed = true;
        self
    }
}

impl<T: HasMatrix> Drop for Transform<'_, T> {
    fn drop(&mut self) {
        if !self.changed {
            return;
        }

        self.handle
            .update(self.translation, self.rotation, self.scale);
    }
}
