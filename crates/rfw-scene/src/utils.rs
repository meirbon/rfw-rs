use rfw_math::*;

use crate::{InstanceFlags2D, InstanceFlags3D, InstanceHandle2D, InstanceHandle3D};

impl HasTranslation for InstanceHandle2D {
    fn get_translation(&self) -> Vec3 {
        let matrix = unsafe { (*self.ptr.get()).matrices[self.index] };
        let (_, _, translation) = matrix.to_scale_rotation_translation();
        translation
    }
}
impl HasTranslation for InstanceHandle3D {
    fn get_translation(&self) -> Vec3 {
        let matrix = unsafe { (*self.ptr.get()).matrices[self.index] };
        let (_, _, translation) = matrix.to_scale_rotation_translation();
        translation
    }
}

impl HasRotation for InstanceHandle2D {
    fn get_rotation(&self) -> Quat {
        let matrix = unsafe { (*self.ptr.get()).matrices[self.index] };
        let (_, rotation, _) = matrix.to_scale_rotation_translation();
        rotation
    }
}
impl HasRotation for InstanceHandle3D {
    fn get_rotation(&self) -> Quat {
        let matrix = unsafe { (*self.ptr.get()).matrices[self.index] };
        let (_, rotation, _) = matrix.to_scale_rotation_translation();
        rotation
    }
}

impl HasScale for InstanceHandle2D {
    fn get_scale(&self) -> Vec3 {
        let matrix = unsafe { (*self.ptr.get()).matrices[self.index] };
        let (scale, _, _) = matrix.to_scale_rotation_translation();
        scale
    }
}
impl HasScale for InstanceHandle3D {
    fn get_scale(&self) -> Vec3 {
        let matrix = unsafe { (*self.ptr.get()).matrices[self.index] };
        let (scale, _, _) = matrix.to_scale_rotation_translation();
        scale
    }
}

impl HasMatrix for InstanceHandle2D {
    fn update(&mut self, t: Vec3, r: Quat, s: Vec3) {
        self.set_matrix(Mat4::from_scale_rotation_translation(s, r, t));
    }

    fn set_matrix(&mut self, matrix: Mat4) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.matrices[self.index] = matrix;
        list.flags[self.index] |= InstanceFlags2D::TRANSFORMED;
    }
}

impl HasMatrix for InstanceHandle3D {
    fn update(&mut self, t: Vec3, r: Quat, s: Vec3) {
        self.set_matrix(Mat4::from_scale_rotation_translation(s, r, t));
    }

    fn set_matrix(&mut self, matrix: Mat4) {
        let list = unsafe { self.ptr.get().as_mut().unwrap() };
        list.matrices[self.index] = matrix;
        list.flags[self.index] |= InstanceFlags3D::TRANSFORMED;
    }
}
