use rfw_backend::*;
use rfw_math::*;
use rtbvh::AABB;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
#[repr(C)]
#[allow(dead_code)]
pub struct LightInfo {
    pub pm: Mat4,
    pub pos: [f32; 3],
    pub range: f32,
    // 80
    padding0: [Vec4; 3],
    padding1: Mat4,
    padding2: Mat4,
}

pub trait Light {
    fn set_radiance(&mut self, radiance: Vec3);
    fn get_matrix(&self, scene_bounds: &AABB) -> Mat4;
    fn get_light_info(&self, scene_bounds: &AABB) -> LightInfo;
    fn get_range(&self, scene_bounds: &AABB) -> AABB;
    fn get_radiance(&self) -> Vec3;
    fn get_energy(&self) -> f32;
}

impl Default for LightInfo {
    fn default() -> Self {
        Self {
            pm: Mat4::identity(),
            pos: [0.0; 3],
            range: 0.0,
            padding0: [Vec4::zero(); 3],
            padding1: Mat4::identity(),
            padding2: Mat4::identity(),
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
//     pub fn get_light_info(&self, _scene_bounds: &AABB) -> CubeLightInfo {
//         unimplemented!()
//     }
// }

// impl SpotLight {

// }

impl Light for AreaLight {
    fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance.into();
        self.energy = radiance.length();
    }

    fn get_matrix(&self, _: &AABB) -> Mat4 {
        let direction = Vec3::from(self.normal);
        let up = if direction.y.abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };
        let center: Vec3 = Vec3::from(self.position);
        let l = self.energy * self.area;

        let fov = 150.0_f32.to_radians();
        let projection = Mat4::perspective_rh_gl(fov, 1.0, 0.1, l);

        let view = Mat4::look_at_rh(center, center + direction, up);
        projection * view
    }

    fn get_light_info(&self, scene_bounds: &AABB) -> LightInfo {
        LightInfo {
            pm: self.get_matrix(scene_bounds),
            pos: self.position,
            range: self.energy * self.area,
            ..LightInfo::default()
        }
    }

    fn get_range(&self, _: &AABB) -> AABB {
        let pos = Vec3::from(self.position);
        let normal = Vec3::from(self.normal);

        let up = if normal.y.abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };

        let right = normal.cross(up).normalize();
        let up = normal.cross(right).normalize();
        let l = self.energy * self.area;

        let range_x = Vec3::new(l, 0.0, 0.0) * right;
        let range_y = Vec3::new(0.0, l, 0.0) * normal;
        let range_z = Vec3::new(0.0, 0.0, l) * up;

        AABB::from_points(&[
            pos.into(),
            (pos + range_x).into(),
            (pos + range_y).into(),
            (pos + range_z).into(),
        ])
    }

    fn get_radiance(&self) -> Vec3 {
        self.radiance.into()
    }

    fn get_energy(&self) -> f32 {
        self.energy
    }
}

impl Light for SpotLight {
    fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance.into();
        self.energy = radiance.length();
    }

    fn get_matrix(&self, _: &AABB) -> Mat4 {
        let direction = Vec3::from(self.direction);
        let up = if direction.y.abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };
        let fov = self.cos_outer.acos() * 2.0;

        let direction = Vec3::from(self.direction);
        let center: Vec3 = Vec3::from(self.position);
        let projection = Mat4::perspective_rh_gl(fov, 1.0, 0.1, self.energy * 2.0);
        let view = Mat4::look_at_rh(center.into(), (center + direction).into(), up.into());
        projection * view
    }

    fn get_light_info(&self, scene_bounds: &AABB) -> LightInfo {
        LightInfo {
            pm: self.get_matrix(scene_bounds),
            pos: self.position,
            range: self.energy * 2.0,
            ..LightInfo::default()
        }
    }

    fn get_range(&self, _: &AABB) -> AABB {
        let pos: Vec3 = self.position.into();
        let direction: Vec3 = self.direction.into();
        let up = if direction.y.abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };

        let right = direction.cross(up).normalize();
        let up = right.cross(direction).normalize();

        let angle = self.cos_outer.acos();
        let length = self.energy;
        let width = length * angle.tan();
        let extent = pos + direction * length;
        let width: Vec3 = right * width;
        let height: Vec3 = up * width;

        AABB::from_points(&[
            pos.into(),
            (extent).into(),
            (extent + width).into(),
            (extent - width).into(),
            (extent + height).into(),
            (extent - height).into(),
        ])
    }

    fn get_radiance(&self) -> Vec3 {
        self.radiance.into()
    }

    fn get_energy(&self) -> f32 {
        self.energy
    }
}

impl Light for DirectionalLight {
    fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance.into();
        self.energy = radiance.length();
    }

    fn get_matrix(&self, scene_bounds: &AABB) -> Mat4 {
        let direction = Vec3::from(self.direction);
        let up = if direction.y.abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };

        let lengths: Vec3 = scene_bounds.lengths::<Vec3>();
        let dims: Vec3 = lengths * direction;
        let l = dims.length() * 1.5;
        let center = scene_bounds.center::<Vec3>() - Vec3::splat(0.5 * l) * direction;

        let h = (up * l).length();
        let w = (direction.cross(up).normalize() * l).length();

        let projection = Mat4::orthographic_rh(-w, w, -h, h, 0.1, l);
        let view = Mat4::look_at_rh(center.into(), (center + direction).into(), up.into());
        projection * view
    }

    fn get_light_info(&self, scene_bounds: &AABB) -> LightInfo {
        let direction = Vec3::from(self.direction);
        let lengths: Vec3 = scene_bounds.lengths::<Vec3>();
        let dims: Vec3 = lengths * direction;
        let l = dims.length() * 1.5;
        let center = scene_bounds.center::<Vec3>() - Vec3::splat(0.5 * l) * direction;

        LightInfo {
            pm: self.get_matrix(scene_bounds),
            pos: center.into(),
            range: l,
            ..LightInfo::default()
        }
    }

    fn get_range(&self, scene_bounds: &AABB) -> AABB {
        let direction: Vec3 = self.direction.into();
        let up = if direction.y.abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };

        let lengths: Vec3 = scene_bounds.lengths::<Vec3>();
        let dims: Vec3 = lengths * direction;
        let l = dims.length() * 1.5;
        let center = scene_bounds.center::<Vec3>() - Vec3::splat(0.5 * l) * direction;

        let h = (up * l).length();
        let w = (direction.cross(up).normalize() * l).length();

        let right = direction.cross(up).normalize();
        let up = right.cross(direction).normalize();

        AABB::from_points(&[
            center.into(),
            (center + w * right).into(),
            (center - w * right).into(),
            (center + h * up).into(),
            (center - h * up).into(),
            (center + l * direction).into(),
        ])
    }

    fn get_radiance(&self) -> Vec3 {
        self.radiance.into()
    }

    fn get_energy(&self) -> f32 {
        self.energy
    }
}
