use glam::*;
use rtbvh::AABB;
use serde::{Deserialize, Serialize};
use std::fmt::Display;

pub trait Light {
    fn set_radiance(&mut self, radiance: Vec3);
    fn get_matrix(&self) -> Mat4;
    fn get_light_info(&self) -> LightInfo;
    fn get_range(&self) -> AABB;
}

#[derive(Debug, Copy, Clone)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct AreaLight {
    pub position: [f32; 3],
    energy: f32,
    pub normal: [f32; 3],
    pub tri_idx: i32,
    pub vertex0: [f32; 3],
    pub inst_idx: i32,
    pub vertex1: [f32; 3],
    radiance: [f32; 3],
    pub vertex2: [f32; 3],
}

impl Display for AreaLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AreaLight {{ position: {}, energy: {}, normal: {}, tri_idx: {}, vertex0: {}, inst_idx: {}, vertex1: {}, radiance: {}, vertex2: {} }}",
            Vec3::from(self.position),
            self.energy,
            Vec3::from(self.normal),
            self.tri_idx,
            Vec3::from(self.vertex0),
            self.inst_idx,
            Vec3::from(self.vertex1),
            Vec3::from(self.radiance),
            Vec3::from(self.vertex2),
        )
    }
}

impl AreaLight {
    pub fn new(
        pos: Vec3,
        radiance: Vec3,
        normal: Vec3,
        tri_id: i32,
        inst_id: i32,
        vertex0: Vec3,
        vertex1: Vec3,
        vertex2: Vec3,
    ) -> AreaLight {
        let radiance = radiance.abs();
        let energy = radiance.length();
        Self {
            position: pos.into(),
            energy,
            normal: normal.into(),
            tri_idx: tri_id,
            vertex0: vertex0.into(),
            inst_idx: inst_id,
            vertex1: vertex1.into(),
            radiance: radiance.into(),
            vertex2: vertex2.into(),
        }
    }
}

impl Light for AreaLight {
    fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance.into();
        self.energy = radiance.length();
    }

    fn get_matrix(&self) -> Mat4 {
        let direction = Vec3::from(self.normal);
        let up = if direction.y().abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };

        let center: Vec3 = Vec3::from(self.position);
        let projection = Mat4::perspective_rh_gl(120.0, 1.0, 0.1, self.energy);
        let view = Mat4::look_at_rh(center, center + direction, up);
        projection * view
    }

    fn get_light_info(&self) -> LightInfo {
        LightInfo {
            pm: self.get_matrix(),
            pos: self.position,
            range: self.energy,
            ..LightInfo::default()
        }
    }

    fn get_range(&self) -> AABB {
        let pos = Vec3::from(self.position);
        let normal = Vec3::from(self.normal);

        let up = if normal.y().abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };

        let right = normal.cross(up).normalize();
        let up = normal.cross(right).normalize();

        let range_x = Vec3::new(self.energy, 0.0, 0.0) * right;
        let range_y = Vec3::new(0.0, self.energy, 0.0) * normal;
        let range_z = Vec3::new(0.0, 0.0, self.energy) * up;

        AABB::from_points(&[pos, pos + range_x, pos + range_y, pos + range_z])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct PointLight {
    pub position: [f32; 3],
    pub energy: f32,
    radiance: [f32; 3],
    _dummy: i32,
}

impl Display for PointLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PointLight {{ position: {}, energy: {}, radiance: {} }}",
            Vec3::from(self.position),
            self.energy,
            Vec3::from(self.radiance)
        )
    }
}

impl PointLight {
    pub fn new(position: Vec3, radiance: Vec3) -> PointLight {
        Self {
            position: position.into(),
            energy: radiance.length(),
            radiance: radiance.into(),
            _dummy: 0,
        }
    }

    pub fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance.into();
        self.energy = radiance.length();
    }

    pub fn get_matrix(&self, _: &AABB) -> [Mat4; 6] {
        let fov = (90.0 as f32).to_radians();
        let projection = Mat4::perspective_rh_gl(fov, 1.0, 0.1, 1e3);
        let center: Vec3 = Vec3::from(self.position);

        [
            projection
                * Mat4::look_at_rh(
                    center,
                    center + Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(0.0, -1.0, 0.0),
                ),
            projection
                * Mat4::look_at_rh(
                    center,
                    center + Vec3::new(-1.0, 0.0, 0.0),
                    Vec3::new(0.0, -1.0, 0.0),
                ),
            projection
                * Mat4::look_at_rh(
                    center,
                    center + Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(0.0, 0.0, 1.0),
                ),
            projection
                * Mat4::look_at_rh(
                    center,
                    center + Vec3::new(0.0, -1.0, 0.0),
                    Vec3::new(0.0, 0.0, -1.0),
                ),
            projection
                * Mat4::look_at_rh(
                    center,
                    center + Vec3::new(0.0, 0.0, 1.0),
                    Vec3::new(0.0, -1.0, 0.0),
                ),
            projection
                * Mat4::look_at_rh(
                    center,
                    center + Vec3::new(0.0, 0.0, -1.0),
                    Vec3::new(0.0, -1.0, 0.0),
                ),
        ]
    }

    pub fn get_light_info(&self, _scene_bounds: &AABB) -> CubeLightInfo {
        unimplemented!()
    }

    pub fn get_range(&self, _scene_bounds: &AABB) -> AABB {
        unimplemented!()
    }
} // 32 Bytes

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct SpotLight {
    pub position: [f32; 3],
    pub cos_inner: f32,
    radiance: [f32; 3],
    pub cos_outer: f32,
    pub direction: [f32; 3],
    energy: f32,
} // 48 Bytes

impl Display for SpotLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SpotLight {{ position: {}, cos_inner: {}, radiance: {}, cos_outer: {}, direction: {}, energy: {} }}",
            Vec3::from(self.position),
            self.cos_inner,
            Vec3::from(self.radiance),
            self.cos_outer,
            Vec3::from(self.direction),
            self.energy
        )
    }
}

impl SpotLight {
    pub fn new(
        position: Vec3,
        direction: Vec3,
        inner_angle: f32,
        outer_angle: f32,
        radiance: Vec3,
    ) -> SpotLight {
        debug_assert!(outer_angle > inner_angle);
        let inner_angle = inner_angle.to_radians();
        let outer_angle = outer_angle.to_radians();
        let radiance = radiance.abs();

        Self {
            position: position.into(),
            cos_inner: inner_angle.cos(),
            radiance: radiance.into(),
            cos_outer: outer_angle.cos(),
            direction: direction.normalize().into(),
            energy: radiance.length(),
        }
    }
}

impl Light for SpotLight {
    fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance.into();
        self.energy = radiance.length();
    }

    fn get_matrix(&self) -> Mat4 {
        let direction = Vec3::from(self.direction);
        let up = if direction.y().abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };
        let fov = self.cos_outer.acos() * 2.0;

        let direction = Vec3::from(self.direction);
        let center: Vec3 = Vec3::from(self.position);
        let projection = Mat4::perspective_rh_gl(fov, 1.0, 0.1, self.energy);
        let view = Mat4::look_at_rh(center, center + direction, up);
        projection * view
    }

    fn get_light_info(&self) -> LightInfo {
        LightInfo {
            pm: self.get_matrix(),
            pos: self.position,
            range: self.energy,
            ..LightInfo::default()
        }
    }

    fn get_range(&self) -> AABB {
        let pos: Vec3 = self.position.into();
        let direction: Vec3 = self.direction.into();
        let up = if direction.y().abs() > 0.99 {
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
            pos,
            extent,
            extent + width,
            extent - width,
            extent + height,
            extent - height,
        ])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct DirectionalLight {
    pub direction: [f32; 3],
    energy: f32,
    radiance: [f32; 3],
    _dummy: i32,
} // 32 Bytes

impl Display for DirectionalLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DirectionalLight {{ direction: {}, energy: {}, radiance: {} }}",
            Vec3::from(self.direction),
            self.energy,
            Vec3::from(self.radiance),
        )
    }
}

impl DirectionalLight {
    pub fn new(direction: Vec3, radiance: Vec3) -> DirectionalLight {
        let radiance = radiance.abs();
        Self {
            direction: direction.normalize().into(),
            energy: radiance.length(),
            radiance: radiance.into(),
            _dummy: 0,
        }
    }
}

impl Light for DirectionalLight {
    fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance.into();
        self.energy = radiance.length();
    }

    fn get_matrix(&self) -> Mat4 {
        let direction = Vec3::from(self.direction);
        let up = if direction.y().abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };

        let l = 1e4;

        let h = (up * l).length() / 2.0;
        let w = (direction.cross(up).normalize() * l).length() / 2.0;

        let center = Vec3::zero() - 1e3 * direction;
        let projection = Mat4::orthographic_rh(-w, w, -h, h, 0.1, l);
        let view = Mat4::look_at_rh(center, center + direction, up);
        projection * view
    }

    fn get_light_info(&self) -> LightInfo {
        let direction = Vec3::from(self.direction);
        let l = 1e3;
        let center = Vec3::zero() - l * direction;

        LightInfo {
            pm: self.get_matrix(),
            pos: center.into(),
            range: l,
            ..LightInfo::default()
        }
    }

    fn get_range(&self) -> AABB {
        let direction: Vec3 = self.direction.into();
        let l = 1e4;
        let up = if direction.y().abs() > 0.99 {
            Vec3::unit_z()
        } else {
            Vec3::unit_y()
        };

        let h = (up * l).length() / 2.0;
        let w = (direction.cross(up).normalize() * l).length() / 2.0;

        let center = Vec3::zero() - l * direction;

        let right = direction.cross(up).normalize();
        let up = right.cross(direction).normalize();

        AABB::from_points(&[
            center,
            center + w * right,
            center - w * right,
            center + h * up,
            center - h * up,
            center + l * direction,
        ])
    }
}
