// use nalgebra_glm;
// use crate::math::*;
use std::f32::consts::PI;
use glam::*;

#[derive(Copy, Clone)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

#[allow(dead_code)]
impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Ray {
        Ray {
            origin,
            direction,
        }
    }

    pub fn reflect_self(&mut self, p: Vec3, n: Vec3) {
        self.direction = self.direction - n * n.dot(self.direction);
        self.origin = p + self.direction * crate::constants::EPSILON;
    }

    pub fn reflect(&self, p: Vec3, n: Vec3) -> Ray {
        let tmp: Vec3 = n * n.dot(self.direction) * 2.0;
        let dir = self.direction - tmp;
        Ray::new(p + dir * crate::constants::EPSILON, dir)
    }

    pub fn get_point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

#[derive(Debug, Clone)]
pub struct Camera {
    pub pos: Vec3,
    up: Vec3,
    direction: Vec3,
    fov: f32,
    width: u32,
    height: u32,
    aspect_ratio: f32,
    aperture: f32,
    focal_distance: f32,
}

#[derive(Debug, Copy, Clone)]
pub struct CameraView {
    pub pos: Vec3,
    pub lens_size: f32,

    pub right: Vec3,
    pub spread_angle: f32,

    pub up: Vec3,
    pub epsilon: f32,

    pub p1: Vec3,
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

        Ray::new(origin, direction)
    }

    pub fn generate_ray(&self, x: u32, y: u32) -> Ray {
        let u = x as f32 * self.inv_width;
        let v = y as f32 * self.inv_height;
        let point_on_pixel = self.p1 + u * self.right + v * self.up;
        let direction = (point_on_pixel - self.pos).normalize();

        Ray::new(self.pos, direction)
    }
}

#[allow(dead_code)]
impl Camera {
    pub fn new(width: u32, height: u32) -> Camera {
        Camera {
            pos: Vec3::new(0.0, 0.0, 0.0),
            up: Vec3::new(0.0, 1.0, 0.0),
            direction: Vec3::new(0.0, 0.0, 1.0),
            fov: 40.0,
            width,
            height,
            aspect_ratio: width as f32 / height as f32,
            aperture: 0.0001,
            focal_distance: 1.0,
        }
    }

    pub fn get_view(&self) -> CameraView {
        let (right, up, forward) = self.calculate_matrix();
        let pos = self.pos;
        let fov = self.fov;
        let spread_angle = (fov * std::f32::consts::PI / 180.0) * (1.0 / self.height as f32);
        let screen_size = (fov * 0.5 / (180.0 / std::f32::consts::PI)).tan();
        let center = pos + self.focal_distance * forward;

        let p1 = center - screen_size * right * self.focal_distance * self.aspect_ratio + screen_size * self.focal_distance * up;
        let p2 = center + screen_size * right * self.focal_distance * self.aspect_ratio + screen_size * self.focal_distance * up;
        let p3 = center - screen_size * right * self.focal_distance * self.aspect_ratio - screen_size * self.focal_distance * up;

        let aperture = self.aperture;
        let right = p2 - p1;
        let up = p3 - p1;

        CameraView {
            pos: pos as Vec3,
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
        self.fov = fov.clamp(20.0, 160.0);
    }

    pub fn get_fov(&self) -> f32 {
        return self.fov;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.aspect_ratio = width as f32 / height as f32;
    }

    pub fn translate_relative(&mut self, delta: Vec3) {
        let (right, up, forward) = self.calculate_matrix();
        self.pos += delta.x() * right + delta.y() * up + delta.z() * forward;
    }

    pub fn translate_target(&mut self, delta: Vec3) {
        let (right, up, forward) = self.calculate_matrix();
        self.direction = (self.direction + delta.x() * right + delta.y() * up + delta.z() * forward).normalize();
    }

    pub fn look_at(&mut self, origin: Vec3, target: Vec3) {
        self.pos = origin;
        self.direction = (target - origin).normalize();
    }

    pub fn get_matrix(&self, near_plane: f32, far_plane: f32) -> Mat4 {
        let up = vec3(0.0, 1.0, 0.0);
        let fov_dist = (self.fov * 0.5).to_radians().tan();

        let flip = Mat4::from_scale([-1.0; 3].into());
        let projection = Mat4::perspective_rh_gl(self.fov.to_radians(), self.aspect_ratio, near_plane, far_plane);
        let view = Mat4::look_at_rh(self.pos, self.pos + self.direction * fov_dist, up);

        projection * flip * view
    }

    fn calculate_matrix(&self) -> (Vec3, Vec3, Vec3) {
        let y: Vec3 = vec3(0.0, 1.0, 0.0);
        let z: Vec3 = self.direction.normalize();
        let x: Vec3 = z.cross(y).normalize();
        let y: Vec3 = x.cross(z);
        (x, y, z)
    }
}
