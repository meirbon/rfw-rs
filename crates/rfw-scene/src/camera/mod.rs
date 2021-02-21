use rfw_backend::{CameraView2D, CameraView3D};
use rfw_math::*;

pub mod frustrum;

pub use frustrum::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Camera3D {
    pub pos: [f32; 3],
    up: [f32; 3],
    pub direction: [f32; 3],
    fov: f32,
    pub aspect_ratio: f32,
    pub aperture: f32,
    pub focal_distance: f32,
    pub near_plane: f32,
    pub far_plane: f32,
    pub speed: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            pos: [0.0; 3],
            up: [0.0, 1.0, 0.0],
            direction: [0.0, 0.0, 1.0],
            fov: 60.0,
            aspect_ratio: 1024_f32 / 768_f32,
            aperture: 0.0001,
            focal_distance: 1.0,
            near_plane: 1e-2,
            far_plane: 1e5,
            speed: 1.0,
        }
    }
}

#[allow(dead_code)]
impl Camera3D {
    pub fn zero() -> Camera3D {
        Camera3D {
            pos: [0.0; 3],
            up: [0.0; 3],
            direction: [0.0, 0.0, 1.0],
            fov: 90.0,
            aspect_ratio: 1.0,
            aperture: 0.0001,
            focal_distance: 1.0,
            near_plane: 1.0,
            far_plane: 1e5,
            speed: 1.0,
        }
    }

    pub fn new() -> Camera3D {
        Camera3D {
            pos: [0.0; 3],
            up: [0.0; 3],
            direction: [0.0, 0.0, 1.0],
            fov: 40.0,
            aspect_ratio: 1.0,
            aperture: 0.0001,
            focal_distance: 1.0,
            near_plane: 1e-2,
            far_plane: 1e5,
            speed: 1.0,
        }
    }

    pub fn get_view(&self, width: u32, height: u32) -> CameraView3D {
        let (right, up, forward) = self.calculate_matrix();
        let pos = Vec3::from(self.pos);
        let fov = self.fov;
        let spread_angle = (fov * std::f32::consts::PI / 180.0) * (1.0 / height as f32);
        let screen_size = (fov * 0.5 / (180.0 / std::f32::consts::PI)).tan();
        let center = pos + self.focal_distance * forward;

        let p1 = center - screen_size * right * self.focal_distance * self.aspect_ratio
            + screen_size * self.focal_distance * up;
        let p2 = center
            + screen_size * right * self.focal_distance * self.aspect_ratio
            + screen_size * self.focal_distance * up;
        let p3 = center
            - screen_size * right * self.focal_distance * self.aspect_ratio
            - screen_size * self.focal_distance * up;

        let aperture = self.aperture;
        let right = p2 - p1;
        let up = p3 - p1;

        CameraView3D {
            pos: pos.into(),
            lens_size: aperture,
            right: right.into(),
            p1: p1.into(),
            direction: forward.into(),
            spread_angle,
            up: up.into(),
            epsilon: crate::constants::EPSILON,
            inv_width: 1.0 / width as f32,
            inv_height: 1.0 / height as f32,
            aspect_ratio: self.aspect_ratio,
            fov: self.fov.to_radians(),
            near_plane: self.near_plane,
            far_plane: self.far_plane,
        }
    }

    pub fn set_fov(&mut self, fov: f32) {
        self.fov = fov.min(160.0).max(20.0);
    }

    pub fn with_fov(mut self, fov: f32) -> Self {
        self.set_fov(fov);
        self
    }

    pub fn get_fov(&self) -> f32 {
        self.fov
    }

    pub fn get_aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }

    pub fn set_aspect_ratio(&mut self, aspect_ratio: f32) {
        self.aspect_ratio = aspect_ratio;
    }

    pub fn with_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        self.aspect_ratio = aspect_ratio;
        self
    }

    pub fn get_transform(&mut self) -> Transform<Self> {
        Transform::from_trs(self.pos.into(), Quat::identity(), Vec3::default(), self)
    }

    pub fn with_position<T: Into<[f32; 3]>>(mut self, position: T) -> Self {
        self.pos = position.into();
        self
    }

    pub fn with_direction<T: Into<[f32; 3]>>(mut self, direction: T) -> Self {
        let direction: Vec3 = Vec3::from(direction.into());
        self.direction = direction.normalize().into();
        self
    }

    pub fn translate_relative<T: Into<[f32; 3]>>(&mut self, delta: T) {
        let delta = Vec3::from(delta.into());
        let delta = delta * self.speed;
        let (right, up, forward) = self.calculate_matrix();
        self.pos =
            (Vec3::from(self.pos) + (delta.x * right + delta.y * up + delta.z * forward)).into();
    }

    pub fn translate_target<T: Into<[f32; 3]>>(&mut self, delta: T) {
        let (right, up, forward) = self.calculate_matrix();
        let delta: [f32; 3] = delta.into();
        self.direction =
            (Vec3::from(self.direction) + delta[0] * right + delta[1] * up + delta[2] * forward)
                .normalize()
                .into();
    }

    pub fn look_at<T: Into<[f32; 3]>>(&mut self, origin: T, target: T) {
        let origin: Vec3 = Vec3::from(origin.into());
        let target: Vec3 = Vec3::from(target.into());
        self.pos = origin.into();
        self.direction = (target - origin).normalize().into();
    }

    pub fn get_rh_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);
        let fov = self.fov.to_radians();

        let projection =
            Mat4::perspective_rh_gl(fov, self.aspect_ratio, self.near_plane, self.far_plane);

        let pos = Vec3::from(self.pos);
        let dir = Vec3::from(self.direction);

        let view = Mat4::look_at_rh(pos, pos + dir, up);

        projection * view
    }

    pub fn get_lh_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);
        let fov = self.fov.to_radians();

        let projection =
            Mat4::perspective_lh(fov, self.aspect_ratio, self.near_plane, self.far_plane);

        let pos = Vec3::from(self.pos);
        let dir = Vec3::from(self.direction);

        let view = Mat4::look_at_lh(pos, pos + dir, up);

        projection * view
    }

    pub fn get_rh_projection(&self) -> Mat4 {
        let fov = self.fov.to_radians();
        Mat4::perspective_rh_gl(fov, self.aspect_ratio, self.near_plane, self.far_plane)
    }

    pub fn get_lh_projection(&self) -> Mat4 {
        let fov = self.fov.to_radians();
        Mat4::perspective_lh(fov, self.aspect_ratio, self.near_plane, self.far_plane)
    }

    pub fn get_rh_view_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);

        let pos = Vec3::from(self.pos);
        let dir = Vec3::from(self.direction);

        Mat4::look_at_rh(pos, pos + dir, up)
    }

    pub fn get_lh_view_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);

        let pos = Vec3::from(self.pos);
        let dir = Vec3::from(self.direction);

        Mat4::look_at_lh(pos, pos + dir, up)
    }

    fn calculate_matrix(&self) -> (Vec3, Vec3, Vec3) {
        let y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
        let z: Vec3 = Vec3::from(self.direction).normalize();
        let x: Vec3 = z.cross(y).normalize();
        let y: Vec3 = x.cross(z).normalize();
        (x, y, z)
    }

    pub fn calculate_frustrum(&self) -> frustrum::FrustrumG {
        frustrum::FrustrumG::from_matrix(self.get_rh_matrix())
    }

    #[cfg(feature = "serde")]
    pub fn serialize<S: AsRef<std::path::Path>>(
        &self,
        path: S,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.as_ref().with_extension("cam");
        use std::io::Write;
        let encoded: Vec<u8> = bincode::serialize(self)?;
        let mut file = std::fs::File::create(&path)?;
        file.write_all(encoded.as_ref())?;
        Ok(())
    }

    #[cfg(feature = "serde")]
    pub fn deserialize<S: AsRef<std::path::Path>>(
        path: S,
    ) -> Result<Camera, Box<dyn std::error::Error>> {
        let path = path.as_ref().with_extension("cam");
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let object: Self = bincode::deserialize_from(reader)?;
        Ok(object)
    }
}

impl HasMatrix for Camera3D {
    fn update(&mut self, t: Vec3, r: Quat, _s: Vec3) {
        self.pos = t.into();
        self.direction = r.mul_vec3(self.direction.into()).into();
    }

    fn set_matrix(&mut self, matrix: Mat4) {
        let (_s, r, t) = matrix.to_scale_rotation_translation();
        self.pos = t.into();
        self.direction = r.mul_vec3(self.direction.into()).into();
    }
}

impl HasTranslation for Camera3D {
    fn get_translation(&self) -> Vec3 {
        self.pos.into()
    }
}

impl HasRotation for Camera3D {
    fn get_rotation(&self) -> Quat {
        Quat::identity()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Fov(f32);

#[derive(Debug, Clone, Copy)]
pub struct Dimensions {
    pub left: f32,
    pub right: f32,
    pub bottom: f32,
    pub top: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Camera2D {
    pub dimensions: Dimensions,
}

impl Camera2D {
    pub fn get_view(&self) -> CameraView2D {
        self.into()
    }

    pub fn from_width_height(width: u32, height: u32, scale_factor: Option<f64>) -> Self {
        let scale_fcator = scale_factor.unwrap_or(1.0) as f32;
        let w = width as f32 * scale_fcator / 2.0;
        let h = height as f32 * scale_fcator / 2.0;
        Self {
            dimensions: Dimensions {
                left: -w,
                right: w,
                bottom: -h,
                top: h,
                near: 10.0,
                far: -10.0,
            },
        }
    }

    pub fn width(&self) -> f32 {
        (self.dimensions.right - self.dimensions.left).abs()
    }

    pub fn height(&self) -> f32 {
        (self.dimensions.top - self.dimensions.bottom).abs()
    }
}

impl From<Camera2D> for CameraView2D {
    fn from(c: Camera2D) -> Self {
        CameraView2D {
            matrix: Mat4::orthographic_rh(
                c.dimensions.left,
                c.dimensions.right,
                c.dimensions.bottom,
                c.dimensions.top,
                c.dimensions.near,
                c.dimensions.far,
            ),
        }
    }
}

impl From<&Camera2D> for CameraView2D {
    fn from(c: &Camera2D) -> Self {
        CameraView2D {
            matrix: Mat4::orthographic_rh(
                c.dimensions.left,
                c.dimensions.right,
                c.dimensions.bottom,
                c.dimensions.top,
                c.dimensions.near,
                c.dimensions.far,
            ),
        }
    }
}

impl From<&mut Camera2D> for CameraView2D {
    fn from(c: &mut Camera2D) -> Self {
        CameraView2D {
            matrix: Mat4::orthographic_rh(
                c.dimensions.left,
                c.dimensions.right,
                c.dimensions.bottom,
                c.dimensions.top,
                c.dimensions.near,
                c.dimensions.far,
            ),
        }
    }
}
