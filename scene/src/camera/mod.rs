use glam::*;
use rtbvh::{Ray, RayPacket4};
use std::f32::consts::PI;

use crate::constants::DEFAULT_T_MAX;

pub mod frustrum;

pub use frustrum::*;

#[cfg(feature = "object_caching")]
use serde::{Deserialize, Serialize};

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

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Camera {
    pub pos: [f32; 3],
    up: [f32; 3],
    pub direction: [f32; 3],
    fov: f32,
    width: u32,
    height: u32,
    pub aspect_ratio: f32,
    pub aperture: f32,
    pub focal_distance: f32,
    pub near_plane: f32,
    pub far_plane: f32,
    pub speed: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            pos: [0.0; 3],
            up: [0.0, 1.0, 0.0],
            direction: [0.0, 0.0, 1.0],
            fov: 60.0,
            width: 1024,
            height: 768,
            aspect_ratio: 1024_f32 / 768_f32,
            aperture: 0.0001,
            focal_distance: 1.0,
            near_plane: 1e-2,
            far_plane: 1e5,
            speed: 1.0,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct CameraView {
    pub pos: Vec3A,
    pub right: Vec3A,
    pub up: Vec3A,
    pub p1: Vec3A,
    pub lens_size: f32,
    pub spread_angle: f32,
    pub epsilon: f32,
    pub inv_width: f32,
    pub inv_height: f32,
}

#[allow(dead_code)]
impl CameraView {
    pub fn generate_lens_ray(&self, x: u32, y: u32, r0: f32, r1: f32, r2: f32, r3: f32) -> Ray {
        let blade = (r0 * 9.0).round();
        let r2 = (r2 - blade * (1.0 / 9.0)) * 9.0;
        let pi_over_4dot5 = PI / 4.5;
        let blade_param = blade * pi_over_4dot5;

        let (x1, y1) = blade_param.sin_cos();
        let blade_param = (blade + 1.0) * pi_over_4dot5;
        let (x2, y2) = blade_param.sin_cos();

        let (r2, r3) = {
            if (r2 + r3) > 1.0 {
                (1.0 - r2, 1.0 - r3)
            } else {
                (r2, r3)
            }
        };

        let xr = x1 * r2 + x2 * r3;
        let yr = y1 * r2 + y2 * r3;

        let origin = self.pos + self.lens_size * (self.right * xr + self.up * yr);
        let u = (x as f32 + r0) * self.inv_width;
        let v = (y as f32 + r1) * self.inv_height;
        let point_on_pixel = self.p1 + u * self.right + v * self.up;
        let direction = (point_on_pixel - origin).normalize();

        Ray::new(origin.into(), direction.into())
    }

    pub fn generate_ray(&self, x: u32, y: u32) -> Ray {
        let u = x as f32 * self.inv_width;
        let v = y as f32 * self.inv_height;
        let point_on_pixel = self.p1 + u * self.right + v * self.up;
        let direction = (point_on_pixel - self.pos).normalize();

        Ray::new(self.pos.into(), direction.into())
    }

    pub fn generate_lens_ray4(
        &self,
        x: [u32; 4],
        y: [u32; 4],
        r0: [f32; 4],
        r1: [f32; 4],
        r2: [f32; 4],
        r3: [f32; 4],
        width: u32,
    ) -> RayPacket4 {
        let ids = [
            x[0] + y[0] * width,
            x[1] + y[1] * width,
            x[2] + y[2] * width,
            x[3] + y[3] * width,
        ];

        let r0 = Vec4::from(r0);
        let r1 = Vec4::from(r1);
        let r2 = Vec4::from(r2);
        let r3 = Vec4::from(r3);

        let blade: Vec4 = r0 * Vec4::splat(9.0);
        let r2: Vec4 = (r2 - blade * (1.0 / 9.0)) * 9.0;
        let pi_over_4dot5: Vec4 = Vec4::splat(PI / 4.5);
        let blade_param: Vec4 = blade * pi_over_4dot5;

        // #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        let (x1, y1) = {
            let mut x = [0.0 as f32; 4];
            let mut y = [0.0 as f32; 4];
            for i in 0..4 {
                let (cos, sin) = blade_param[i].sin_cos();
                x[i] = cos;
                y[i] = sin;
            }

            (Vec4::from(x), Vec4::from(y))
        };

        let blade_param = (blade + Vec4::one()) * pi_over_4dot5;
        let (x2, y2) = {
            let mut x = [0.0 as f32; 4];
            let mut y = [0.0 as f32; 4];
            for i in 0..4 {
                let (cos, sin) = blade_param[i].sin_cos();
                x[i] = cos;
                y[i] = sin;
            }

            (Vec4::from(x), Vec4::from(y))
        };

        let (r2, r3) = {
            let mask: Vec4Mask = (r2 + r3).cmpgt(Vec4::one());
            (
                mask.select(Vec4::one() - r2, r2),
                mask.select(Vec4::one() - r3, r3),
            )
        };

        let x = Vec4::from([x[0] as f32, x[1] as f32, x[2] as f32, x[3] as f32]);
        let y = Vec4::from([y[0] as f32, y[1] as f32, y[2] as f32, y[3] as f32]);

        let xr = x1 * r2 + x2 * r2;
        let yr = y1 * r2 + y2 * r3;

        let u = (x + r0) * self.inv_width;
        let v = (y + r1) * self.inv_height;

        let p_x = Vec4::from([self.p1.x(); 4]) + u * self.right.x() + v * self.up.x();
        let p_y = Vec4::from([self.p1.y(); 4]) + u * self.right.y() + v * self.up.y();
        let p_z = Vec4::from([self.p1.z(); 4]) + u * self.right.z() + v * self.up.z();

        let direction_x = p_x - Vec4::from([self.pos.x(); 4]);
        let direction_y = p_y - Vec4::from([self.pos.y(); 4]);
        let direction_z = p_z - Vec4::from([self.pos.z(); 4]);

        let length_squared = direction_x * direction_x;
        let length_squared = length_squared + direction_y * direction_y;
        let length_squared = length_squared + direction_z * direction_z;

        let length = vec4_sqrt(length_squared);

        let inv_length = Vec4::one() / length;

        let direction_x = (direction_x * inv_length).into();
        let direction_y = (direction_y * inv_length).into();
        let direction_z = (direction_z * inv_length).into();

        let origin_x = Vec4::splat(self.pos.x());
        let origin_y = Vec4::splat(self.pos.y());
        let origin_z = Vec4::splat(self.pos.z());

        let lens_size = Vec4::splat(self.lens_size);
        let right_x = Vec4::splat(self.right.x());
        let right_y = Vec4::splat(self.right.y());
        let right_z = Vec4::splat(self.right.z());
        let up_x = Vec4::splat(self.up.x());
        let up_y = Vec4::splat(self.up.y());
        let up_z = Vec4::splat(self.up.z());

        let origin_x = origin_x + lens_size * (right_x * xr + up_x * yr);
        let origin_y = origin_y + lens_size * (right_y * xr + up_y * yr);
        let origin_z = origin_z + lens_size * (right_z * xr + up_z * yr);

        RayPacket4 {
            origin_x: origin_x.into(),
            origin_y: origin_y.into(),
            origin_z: origin_z.into(),
            direction_x,
            direction_y,
            direction_z,
            t: [DEFAULT_T_MAX; 4],
            pixel_ids: ids,
        }
    }

    pub fn generate_ray4(&self, x: [u32; 4], y: [u32; 4], width: u32) -> RayPacket4 {
        let ids = [
            x[0] + y[0] * width,
            x[1] + y[1] * width,
            x[2] + y[2] * width,
            x[3] + y[3] * width,
        ];

        let x = [x[0] as f32, x[1] as f32, x[2] as f32, x[3] as f32];
        let y = [y[0] as f32, y[1] as f32, y[2] as f32, y[3] as f32];

        let x = Vec4::from(x);
        let y = Vec4::from(y);

        let u = x * self.inv_width;
        let v = y * self.inv_height;

        let p_x = Vec4::from([self.p1.x(); 4]) + u * self.right.x() + v * self.up.x();
        let p_y = Vec4::from([self.p1.y(); 4]) + u * self.right.y() + v * self.up.y();
        let p_z = Vec4::from([self.p1.z(); 4]) + u * self.right.z() + v * self.up.z();

        let direction_x = p_x - Vec4::from([self.pos.x(); 4]);
        let direction_y = p_y - Vec4::from([self.pos.y(); 4]);
        let direction_z = p_z - Vec4::from([self.pos.z(); 4]);

        let length_squared = direction_x * direction_x;
        let length_squared = length_squared + direction_y * direction_y;
        let length_squared = length_squared + direction_z * direction_z;

        let length = vec4_sqrt(length_squared);

        let inv_length = Vec4::one() / length;

        let direction_x = (direction_x * inv_length).into();
        let direction_y = (direction_y * inv_length).into();
        let direction_z = (direction_z * inv_length).into();

        let origin_x = [self.pos.x(); 4];
        let origin_y = [self.pos.y(); 4];
        let origin_z = [self.pos.z(); 4];

        RayPacket4 {
            origin_x,
            origin_y,
            origin_z,
            direction_x,
            direction_y,
            direction_z,
            t: [DEFAULT_T_MAX; 4],
            pixel_ids: ids,
        }
    }
}

#[allow(dead_code)]
impl Camera {
    pub fn zero() -> Camera {
        Camera {
            pos: [0.0; 3],
            up: [0.0; 3],
            direction: [0.0, 0.0, 1.0],
            fov: 90.0,
            width: 0,
            height: 0,
            aspect_ratio: 1.0,
            aperture: 0.0001,
            focal_distance: 1.0,
            near_plane: 1.0,
            far_plane: 1e5,
            speed: 1.0,
        }
    }

    pub fn new(width: u32, height: u32) -> Camera {
        Camera {
            pos: [0.0; 3],
            up: [0.0; 3],
            direction: [0.0, 0.0, 1.0],
            fov: 40.0,
            width,
            height,
            aspect_ratio: width as f32 / height as f32,
            aperture: 0.0001,
            focal_distance: 1.0,
            near_plane: 1e-2,
            far_plane: 1e5,
            speed: 1.0,
        }
    }

    pub fn get_view(&self) -> CameraView {
        let (right, up, forward) = self.calculate_matrix();
        let pos = Vec3A::from(self.pos);
        let fov = self.fov;
        let spread_angle = (fov * std::f32::consts::PI / 180.0) * (1.0 / self.height as f32);
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

        CameraView {
            pos,
            lens_size: aperture,
            right,
            spread_angle,
            up,
            epsilon: crate::constants::EPSILON,
            p1,
            inv_width: 1.0 / self.width as f32,
            inv_height: 1.0 / self.height as f32,
        }
    }

    pub fn change_fov(&mut self, fov: f32) {
        self.fov = fov.min(160.0).max(20.0);
    }

    pub fn with_fov(mut self, fov: f32) -> Self {
        self.change_fov(fov);
        self
    }

    pub fn get_fov(&self) -> f32 {
        return self.fov;
    }

    pub fn get_aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }

    pub fn get_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.aspect_ratio = width as f32 / height as f32;
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.resize(width, height);
        self
    }

    pub fn with_position<T: Into<[f32; 3]>>(mut self, position: T) -> Self {
        self.pos = position.into();
        self
    }

    pub fn translate_relative<T: Into<[f32; 3]>>(&mut self, delta: T) {
        let delta = Vec3A::from(delta.into());
        let delta = delta * self.speed;
        let (right, up, forward) = self.calculate_matrix();
        self.pos = (Vec3A::from(self.pos)
            + (delta.x() * right + delta.y() * up + delta.z() * forward))
            .into();
    }

    pub fn translate_target<T: Into<[f32; 3]>>(&mut self, delta: T) {
        let (right, up, forward) = self.calculate_matrix();
        let delta: [f32; 3] = delta.into();
        self.direction =
            (Vec3A::from(self.direction) + delta[0] * right + delta[1] * up + delta[2] * forward)
                .normalize()
                .into();
    }

    pub fn look_at<T: Into<[f32; 3]>>(&mut self, origin: T, target: T) {
        let origin: Vec3A = Vec3A::from(origin.into());
        let target: Vec3A = Vec3A::from(target.into());
        self.pos = origin.into();
        self.direction = (target - origin).normalize().into();
    }

    pub fn with_direction<T: Into<[f32; 3]>>(mut self, direction: T) -> Self {
        let direction: Vec3A = Vec3A::from(direction.into());
        self.direction = direction.normalize().into();
        self
    }

    pub fn get_rh_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);
        let fov = self.fov.to_radians();

        let projection = Mat4::perspective_rh_gl(
            fov,
            self.width as f32 / self.height as f32,
            self.near_plane,
            self.far_plane,
        );

        let pos = Vec3A::from(self.pos);
        let dir = Vec3A::from(self.direction);

        let view = Mat4::look_at_rh(pos.into(), (pos + dir).into(), up);

        projection * view
    }

    pub fn get_lh_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);
        let fov = self.fov.to_radians();

        let projection = Mat4::perspective_lh(
            fov,
            self.width as f32 / self.height as f32,
            self.near_plane,
            self.far_plane,
        );

        let pos = Vec3A::from(self.pos);
        let dir = Vec3A::from(self.direction);

        let view = Mat4::look_at_lh(pos.into(), (pos + dir).into(), up);

        projection * view
    }

    pub fn get_rh_projection(&self) -> Mat4 {
        let fov = self.fov.to_radians();
        Mat4::perspective_rh_gl(
            fov,
            self.width as f32 / self.height as f32,
            self.near_plane,
            self.far_plane,
        )
    }

    pub fn get_lh_projection(&self) -> Mat4 {
        let fov = self.fov.to_radians();
        Mat4::perspective_lh(
            fov,
            self.width as f32 / self.height as f32,
            self.near_plane,
            self.far_plane,
        )
    }

    pub fn get_rh_view_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);

        let pos = Vec3A::from(self.pos);
        let dir = Vec3A::from(self.direction);

        let view = Mat4::look_at_rh(pos.into(), (pos + dir).into(), up);

        view
    }

    pub fn get_lh_view_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);

        let pos = Vec3A::from(self.pos);
        let dir = Vec3A::from(self.direction);

        let view = Mat4::look_at_lh(pos.into(), (pos + dir).into(), up);

        view
    }

    fn calculate_matrix(&self) -> (Vec3A, Vec3A, Vec3A) {
        let y: Vec3A = Vec3A::new(0.0, 1.0, 0.0);
        let z: Vec3A = Vec3A::from(self.direction).normalize();
        let x: Vec3A = z.cross(y).normalize();
        let y: Vec3A = x.cross(z).normalize();
        (x, y, z)
    }

    pub fn calculate_frustrum(&self) -> frustrum::FrustrumG {
        frustrum::FrustrumG::from_matrix(self.get_rh_matrix())
    }

    #[cfg(feature = "object_caching")]
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

    #[cfg(feature = "object_caching")]
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
