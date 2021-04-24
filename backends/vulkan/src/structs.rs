use rfw::backend::{CameraView2D, CameraView3D};
use rfw::math::*;

#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct VkCamera {
    pub view_2d: CameraView2D,
    pub view_3d: CameraView3D,
    pub view: Mat4,
    pub projection: Mat4,
    pub view_projection: Mat4,
    pub light_count: UVec4,
}

#[derive(Debug, Copy, Clone, Default)]
#[repr(C)]
pub struct InstanceTransform {
    pub matrix: Mat4,
    pub inverse: Mat4,
}

impl InstanceTransform {
    pub fn new(matrix: Mat4) -> Self {
        Self {
            matrix,
            inverse: matrix.inverse(),
        }
    }
}
