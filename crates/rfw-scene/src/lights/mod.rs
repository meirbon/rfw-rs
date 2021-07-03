use rfw_backend::*;
use rfw_math::*;
use rtbvh::Aabb;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
#[repr(C)]
pub struct LightInfo {
    pub pm: Mat4,
    pub pos: Vec3,
    pub range: f32,
    // 80
    _padding0: [Vec4; 3],
    _padding1: Mat4,
    _padding2: Mat4,
}

pub trait Light {
    fn set_radiance(&mut self, radiance: Vec3);
    fn get_matrix(&self, scene_bounds: &Aabb) -> Mat4;
    fn get_light_info(&self, scene_bounds: &Aabb) -> LightInfo;
    fn get_range(&self, scene_bounds: &Aabb) -> Aabb;
    fn get_radiance(&self) -> Vec3;
    fn get_energy(&self) -> f32;
}

impl Default for LightInfo {
    fn default() -> Self {
        Self {
            pm: Mat4::IDENTITY,
            pos: Vec3::ZERO,
            range: 0.0,
            _padding0: [Vec4::ZERO; 3],
            _padding1: Mat4::IDENTITY,
            _padding2: Mat4::IDENTITY,
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
#[allow(dead_code)]
pub struct CubeLightInfo {
    pm: [Mat4; 6],
    pos: [f32; 3],
    range: f32,
}

// impl PointLight {
//     pub fn get_light_info(&self, _scene_bounds: &Aabb) -> CubeLightInfo {
//         unimplemented!()
//     }
// }

// impl SpotLight {

// }

impl Light for AreaLight {
    fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance;
        self.energy = radiance.length();
    }

    fn get_matrix(&self, _: &Aabb) -> Mat4 {
        let direction = self.normal;
        let up = if direction.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let center: Vec3 = self.position;
        let l = self.energy * self.area;

        let fov = 150.0_f32.to_radians();
        let projection = Mat4::perspective_rh_gl(fov, 1.0, 0.1, l);

        let view = Mat4::look_at_rh(center, center + direction, up);
        projection * view
    }

    fn get_light_info(&self, scene_bounds: &Aabb) -> LightInfo {
        LightInfo {
            pm: self.get_matrix(scene_bounds),
            pos: self.position,
            range: self.energy * self.area,
            ..LightInfo::default()
        }
    }

    fn get_range(&self, _: &Aabb) -> Aabb {
        let pos = self.position;
        let normal = self.normal;

        let up = if normal.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };

        let right = normal.cross(up).normalize();
        let up = normal.cross(right).normalize();
        let l = self.energy * self.area;

        let range_x = Vec3::new(l, 0.0, 0.0) * right;
        let range_y = Vec3::new(0.0, l, 0.0) * normal;
        let range_z = Vec3::new(0.0, 0.0, l) * up;

        let mut aabb = Aabb::new();
        aabb.grow(pos);
        aabb.grow(pos + range_x);
        aabb.grow(pos + range_y);
        aabb.grow(pos + range_z);
        aabb
    }

    fn get_radiance(&self) -> Vec3 {
        self.radiance
    }

    fn get_energy(&self) -> f32 {
        self.energy
    }
}

impl Light for SpotLight {
    fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance;
        self.energy = radiance.length();
    }

    fn get_matrix(&self, _: &Aabb) -> Mat4 {
        let direction = self.direction;
        let up = if direction.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let fov = self.cos_outer.acos() * 2.0;

        let direction = self.direction;
        let center: Vec3 = self.position;
        let projection = Mat4::perspective_rh_gl(fov, 1.0, 0.1, self.energy * 2.0);
        let view = Mat4::look_at_rh(center, center + direction, up);
        projection * view
    }

    fn get_light_info(&self, scene_bounds: &Aabb) -> LightInfo {
        LightInfo {
            pm: self.get_matrix(scene_bounds),
            pos: self.position,
            range: self.energy * 2.0,
            ..LightInfo::default()
        }
    }

    fn get_range(&self, _: &Aabb) -> Aabb {
        let pos: Vec3 = self.position;
        let direction: Vec3 = self.direction;
        let up = if direction.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };

        let right = direction.cross(up).normalize();
        let up = right.cross(direction).normalize();

        let angle = self.cos_outer.acos();
        let length = self.energy;
        let width = length * angle.tan();
        let extent = pos + direction * length;
        let width: Vec3 = right * width;
        let height: Vec3 = up * width;

        let mut aabb = Aabb::new();
        aabb.grow(pos);
        aabb.grow(extent);
        aabb.grow(extent + width);
        aabb.grow(extent - width);
        aabb.grow(extent + height);
        aabb.grow(extent - height);
        aabb
    }

    fn get_radiance(&self) -> Vec3 {
        self.radiance
    }

    fn get_energy(&self) -> f32 {
        self.energy
    }
}

impl Light for DirectionalLight {
    fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance;
        self.energy = radiance.length();
    }

    fn get_matrix(&self, scene_bounds: &Aabb) -> Mat4 {
        let direction = self.direction;
        let up = if direction.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };

        let lengths: Vec3 = scene_bounds.lengths();
        let dims: Vec3 = lengths * direction;
        let l = dims.length() * 1.5;
        let center = scene_bounds.center() - Vec3::splat(0.5 * l) * direction;

        let h = (up * l).length();
        let w = (direction.cross(up).normalize() * l).length();

        let projection = Mat4::orthographic_rh(-w, w, -h, h, 0.1, l);
        let view = Mat4::look_at_rh(center, center + direction, up);
        projection * view
    }

    fn get_light_info(&self, scene_bounds: &Aabb) -> LightInfo {
        let direction = self.direction;
        let lengths: Vec3 = scene_bounds.lengths();
        let dims: Vec3 = lengths * direction;
        let l = dims.length() * 1.5;
        let center = scene_bounds.center() - Vec3::splat(0.5 * l) * direction;

        LightInfo {
            pm: self.get_matrix(scene_bounds),
            pos: center,
            range: l,
            ..LightInfo::default()
        }
    }

    fn get_range(&self, scene_bounds: &Aabb) -> Aabb {
        let direction: Vec3 = self.direction;
        let up = if direction.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };

        let lengths: Vec3 = scene_bounds.lengths();
        let dims: Vec3 = lengths * direction;
        let l = dims.length() * 1.5;
        let center = scene_bounds.center() - Vec3::splat(0.5 * l) * direction;

        let h = (up * l).length();
        let w = (direction.cross(up).normalize() * l).length();

        let right = direction.cross(up).normalize();
        let up = right.cross(direction).normalize();

        let mut aabb = Aabb::new();
        aabb.grow(center);
        aabb.grow(center + w * right);
        aabb.grow(center - w * right);
        aabb.grow(center + h * up);
        aabb.grow(center - h * up);
        aabb.grow(center + l * direction);
        aabb
    }

    fn get_radiance(&self) -> Vec3 {
        self.radiance
    }

    fn get_energy(&self) -> f32 {
        self.energy
    }
}

#[cfg(test)]
mod tests {
    use crate::LightInfo;

    #[test]
    fn is_aligned() {
        assert!(std::mem::size_of::<LightInfo>() == 256);
    }
}
