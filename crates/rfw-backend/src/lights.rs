use super::structs::RTTriangle;
use rfw_math::*;
use rtbvh::Aabb;
use std::fmt::Display;

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct AreaLight {
    pub position: Vec3,
    // 12
    pub energy: f32,
    // 16
    pub normal: Vec3,
    // 28
    pub area: f32,
    pub vertex0: Vec3,
    // 44
    pub inst_idx: i32,
    // 48
    pub vertex1: Vec3,
    // 60
    mesh_id: i32,
    pub radiance: Vec3,
    // 72
    _dummy1: i32,
    pub vertex2: Vec3,
    // 84
    _dummy2: i32,
}

impl Default for AreaLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO, // 12
            energy: 0.0,          // 16
            normal: Vec3::ZERO,   // 28
            area: 0.0,
            vertex0: Vec3::ZERO, // 44
            inst_idx: 0,         // 48
            vertex1: Vec3::ZERO, // 60
            mesh_id: -1,
            radiance: Vec3::ZERO, // 72
            _dummy1: 0,
            vertex2: Vec3::ZERO, // 84
            _dummy2: 0,
        }
    }
}

impl Display for AreaLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AreaLight {{ position: {}, energy: {}, normal: {}, area: {}, vertex0: {}, inst_idx: {}, vertex1: {}, radiance: {}, vertex2: {} }}",
            self.position,
            self.energy,
            self.normal,
            self.area,
            self.vertex0,
            self.inst_idx,
            self.vertex1,
            self.radiance,
            self.vertex2,
        )
    }
}

impl AreaLight {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        position: Vec3,
        radiance: Vec3,
        normal: Vec3,
        mesh_id: i32,
        inst_id: i32,
        vertex0: Vec3,
        vertex1: Vec3,
        vertex2: Vec3,
    ) -> AreaLight {
        let radiance = radiance.abs();
        let energy = radiance.length();
        Self {
            position,
            energy,
            normal,
            area: RTTriangle::area(vertex0, vertex1, vertex2),
            vertex0,
            inst_idx: inst_id,
            vertex1,
            mesh_id,
            radiance,
            _dummy1: 1,
            vertex2,
            _dummy2: 2,
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
#[repr(C, align(32))]
pub struct PointLight {
    pub position: Vec3,
    pub energy: f32,
    pub radiance: Vec3,
    _dummy: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            energy: 0.0,
            radiance: Vec3::ZERO,
            _dummy: 0.0,
        }
    }
}

impl PointLight {
    pub fn new(position: Vec3, radiance: Vec3) -> PointLight {
        Self {
            position,
            energy: radiance.length(),
            radiance,
            _dummy: 0.0,
        }
    }

    pub fn set_radiance(&mut self, radiance: Vec3) {
        let radiance = radiance.abs();
        self.radiance = radiance;
        self.energy = radiance.length();
    }

    pub fn get_matrix(&self, _: &Aabb) -> [Mat4; 6] {
        let fov = 90.0_f32.to_radians();
        let projection = Mat4::perspective_rh_gl(fov, 1.0, 0.1, 1e3);
        let center = self.position;

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

    pub fn get_range(&self, _scene_bounds: &Aabb) -> Aabb {
        unimplemented!()
    }

    pub fn translate_x(&mut self, offset: f32) {
        self.position[0] += offset;
    }

    pub fn translate_y(&mut self, offset: f32) {
        self.position[1] += offset;
    }

    pub fn translate_z(&mut self, offset: f32) {
        self.position[2] += offset;
    }
} // 32 Bytes

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct SpotLight {
    pub position: Vec3,
    pub cos_inner: f32,
    pub radiance: Vec3,
    pub cos_outer: f32,
    pub direction: Vec3,
    pub energy: f32,
}

impl Default for SpotLight {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            cos_inner: 0.0,
            radiance: Vec3::ZERO,
            cos_outer: 0.0,
            direction: Vec3::ZERO,
            energy: 0.0,
        }
    }
}

impl Display for SpotLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SpotLight {{ position: {}, cos_inner: {}, radiance: {}, cos_outer: {}, direction: {}, energy: {} }}",
            self.position,
            self.cos_inner,
            self.radiance,
            self.cos_outer,
            self.direction,
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
            position,
            cos_inner: inner_angle.cos(),
            radiance,
            cos_outer: outer_angle.cos(),
            direction: direction.normalize(),
            energy: radiance.length(),
        }
    }

    pub fn translate_x(&mut self, offset: f32) {
        self.position[0] += offset;
    }

    pub fn translate_y(&mut self, offset: f32) {
        self.position[1] += offset;
    }

    pub fn translate_z(&mut self, offset: f32) {
        self.position[2] += offset;
    }

    pub fn rotate_x(&mut self, degrees: f32) {
        let rotation = Mat4::from_rotation_x(degrees.to_radians());
        let direction = rotation * self.direction.extend(0.0);
        self.direction = direction.truncate();
    }

    pub fn rotate_y(&mut self, degrees: f32) {
        let rotation = Mat4::from_rotation_y(degrees.to_radians());
        let direction = rotation * self.direction.extend(0.0);
        self.direction = direction.truncate();
    }

    pub fn rotate_z(&mut self, degrees: f32) {
        let rotation = Mat4::from_rotation_z(degrees.to_radians());
        let direction = rotation * self.direction.extend(0.0);
        self.direction = direction.truncate();
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct DirectionalLight {
    pub direction: Vec3,
    pub energy: f32,
    pub radiance: Vec3,
    _dummy: f32,
} // 32 Bytes

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            direction: Vec3::ZERO,
            energy: 0.0,
            radiance: Vec3::ZERO,
            _dummy: 0.0,
        }
    }
}

impl Display for DirectionalLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "DirectionalLight {{ direction: {}, energy: {}, radiance: {} }}",
            self.direction, self.energy, self.radiance,
        )
    }
}

impl DirectionalLight {
    pub fn new(direction: Vec3, radiance: Vec3) -> DirectionalLight {
        let radiance = radiance.abs();
        Self {
            direction: direction.normalize(),
            energy: radiance.length(),
            radiance,
            _dummy: 0.0,
        }
    }

    pub fn rotate_x(&mut self, degrees: f32) {
        let rotation = Mat4::from_rotation_x(degrees.to_radians());
        let direction = rotation * self.direction.extend(0.0);
        self.direction = direction.truncate();
    }

    pub fn rotate_y(&mut self, degrees: f32) {
        let rotation = Mat4::from_rotation_y(degrees.to_radians());
        let direction = rotation * self.direction.extend(0.0);
        self.direction = direction.truncate();
    }

    pub fn rotate_z(&mut self, degrees: f32) {
        let rotation = Mat4::from_rotation_z(degrees.to_radians());
        let direction = rotation * self.direction.extend(0.0);
        self.direction = direction.truncate();
    }
}
