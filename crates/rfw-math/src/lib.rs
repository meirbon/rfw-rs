pub use glam::*;

pub trait HasMatrix: std::fmt::Debug {
    fn update(&mut self, t: Vec3, r: Quat, s: Vec3);
    fn set_matrix(&mut self, matrix: Mat4);
}

pub trait HasTranslation: HasMatrix {
    fn get_translation(&self) -> Vec3;
}
pub trait HasRotation: HasMatrix {
    fn get_rotation(&self) -> Quat;
}
pub trait HasScale: HasMatrix {
    fn get_scale(&self) -> Vec3;
}

#[derive(Debug)]
pub struct Transform<'a, T: HasMatrix> {
    pub(crate) translation: Vec3,
    pub(crate) rotation: Quat,
    pub(crate) scale: Vec3,
    pub(crate) handle: &'a mut T,
    pub(crate) changed: bool,
}

pub trait HasTransform<T: HasMatrix> {
    fn get_transform(&mut self) -> Transform<'_, T>;
}

impl<T: HasTranslation + HasRotation + HasScale> HasTransform<T> for T {
    fn get_transform(&mut self) -> Transform<'_, T> {
        Transform::from_trs(
            self.get_translation(),
            self.get_rotation(),
            self.get_scale(),
            self,
        )
    }
}

impl<'a, T: HasMatrix> Transform<'a, T> {
    pub fn from_matrix(matrix: Mat4, handle: &'a mut T) -> Self {
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        Self {
            translation,
            rotation,
            scale,
            handle,
            changed: false,
        }
    }
    pub fn from_trs(translation: Vec3, rotation: Quat, scale: Vec3, handle: &'a mut T) -> Self {
        Self {
            translation,
            rotation,
            scale,
            handle,
            changed: false,
        }
    }

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

    pub fn set_matrix(&mut self, matrix: Mat4) -> &mut Self {
        let (s, r, t) = matrix.to_scale_rotation_translation();
        self.translation = t;
        self.rotation = r;
        self.scale = s;
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

#[inline(always)]
pub fn vec4_sqrt(vec: Vec4) -> Vec4 {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    unsafe {
        use std::arch::x86_64::_mm_sqrt_ps;
        _mm_sqrt_ps(vec.into()).into()
    }
    #[cfg(any(
        all(not(target_arch = "x86_64"), not(target_arch = "x86")),
        target_arch = "wasm32-unknown-unknown"
    ))]
    {
        Vec4::new(vec[0].sqrt(), vec[1].sqrt(), vec[2].sqrt(), vec[3].sqrt())
    }
}

#[inline(always)]
pub fn vec4_rsqrt(vec: Vec4) -> Vec4 {
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    unsafe {
        use std::arch::x86_64::_mm_rsqrt_ps;
        _mm_rsqrt_ps(vec.into()).into()
    }
    #[cfg(any(
        all(not(target_arch = "x86_64"), not(target_arch = "x86")),
        target_arch = "wasm32-unknown-unknown"
    ))]
    {
        Vec4::new(vec[0].sqrt(), vec[1].sqrt(), vec[2].sqrt(), vec[3].sqrt())
    }
}
