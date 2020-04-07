use bvh::{Bounds, Ray, RayPacket4, AABB};
use glam::*;
use std::f32::consts::PI;

use scene::constants::DEFAULT_T_MAX;

pub fn vec4_sqrt(vec: Vec4) -> Vec4 {
    use std::arch::x86_64::_mm_sqrt_ps;
    unsafe { _mm_sqrt_ps(vec.into()).into() }
}

#[derive(Debug, Clone)]
pub struct Camera {
    pub pos: [f32; 3],
    up: [f32; 3],
    direction: [f32; 3],
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
    pub right: Vec3,
    pub up: Vec3,
    pub p1: Vec3,
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

    pub fn generate_ray4(&self, x: &[u32; 4], y: &[u32; 4], width: u32) -> RayPacket4 {
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
        }
    }

    pub fn get_view(&self) -> CameraView {
        let (right, up, forward) = self.calculate_matrix();
        let pos = Vec3::from(self.pos);
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
            epsilon: scene::constants::EPSILON,
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
        self.pos = (Vec3::from(self.pos)
            + (delta.x() * right + delta.y() * up + delta.z() * forward))
            .into();
    }

    pub fn translate_target(&mut self, delta: Vec3) {
        let (right, up, forward) = self.calculate_matrix();
        self.direction =
            (Vec3::from(self.direction) + delta.x() * right + delta.y() * up + delta.z() * forward)
                .normalize()
                .into();
    }

    pub fn look_at(&mut self, origin: Vec3, target: Vec3) {
        self.pos = origin.into();
        self.direction = (target - origin).normalize().into();
    }

    pub fn get_matrix(&self, near_plane: f32, far_plane: f32) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);
        let fov = self.fov.to_radians();
        let fov_dist = (fov * 0.5).tan();

        let flip = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
        let projection = Mat4::perspective_lh(fov, self.aspect_ratio, near_plane, far_plane);

        let pos = Vec3::from(self.pos);
        let dir = Vec3::from(self.direction);

        let view = Mat4::look_at_lh(pos, pos + dir * fov_dist, up);

        projection * flip * view
    }

    fn calculate_matrix(&self) -> (Vec3, Vec3, Vec3) {
        let y: Vec3 = vec3(0.0, 1.0, 0.0);
        let z: Vec3 = Vec3::from(self.direction).normalize();
        let x: Vec3 = z.cross(y);
        let y: Vec3 = x.cross(z);
        (x, y, z)
    }

    pub fn calculate_frustrum(&self, near_plane: f32, far_plane: f32) -> Frustrum {
        // let (right, up, forward) = self.calculate_matrix();

        // let fov = self.fov.to_radians();

        // let center_z1 = origin + forward;

        // let half_height_z1 = (fov * 0.5).tan();
        // let half_width_z1 = half_height_z1 * self.aspect_ratio;

        // let up_offset: Vec3 = up * half_height_z1;
        // let right_offset: Vec3 = right * half_width_z1;

        // // Calculate normals for every frustrum plane
        // let right_plane_dir = ((center_z1 + right_offset) - origin).normalize();
        // let left_plane_dir = ((center_z1 - right_offset) - origin).normalize();
        // let top_plane_dir = ((center_z1 + up_offset) - origin).normalize();
        // let bottom_plane_dir = ((center_z1 - up_offset) - origin).normalize();

        let origin = Vec3::from(self.pos);
        let (right, up, forward) = self.calculate_matrix();
        let pos = Vec3::from(self.pos);
        let fov = self.fov;
        let screen_size = (fov * 0.5 / (180.0 / PI)).tan();
        let center = pos + forward;

        let right_offset: Vec3 = screen_size * right * self.aspect_ratio;
        let up_offset = screen_size * up;

        let right: Vec3 = center + right_offset;
        let left: Vec3 = center - right_offset;
        let top: Vec3 = center + up_offset;
        let bottom: Vec3 = center - up_offset;

        let right_plane_dir: Vec3 = (right - origin).normalize();
        let left_plane_dir: Vec3 = (left - origin).normalize();
        let top_plane_dir: Vec3 = (top - origin).normalize();
        let bottom_plane_dir: Vec3 = (bottom - origin).normalize();

        let right_plane_normal = right_plane_dir.cross(up);
        let left_plane_normal = left_plane_dir.cross(up);
        let top_plane_normal = top_plane_dir.cross(right);
        let bottom_plane_normal = bottom_plane_dir.cross(right);


        Frustrum {
            origin,
            right_normal: right_plane_normal,
            left_normal: left_plane_normal,
            top_normal: top_plane_normal,
            bottom_normal: bottom_plane_normal,
            near2: near_plane * near_plane,
            far2: far_plane * far_plane,
        }
    }
}

pub struct Frustrum {
    origin: Vec3,
    right_normal: Vec3,
    left_normal: Vec3,
    top_normal: Vec3,
    bottom_normal: Vec3,
    near2: f32,
    far2: f32,
}

impl Frustrum {
    pub fn object_in_frustrum<T: Bounds + Sized>(&self, bounds: &T) -> bool {
        self.aabb_in_frustrum(&bounds.bounds())
    }

    pub fn point_in_frustrum(&self, point: Vec3) -> bool {
        let dot_right = point.dot(self.right_normal) > 0.0;
        let dot_left = point.dot(self.left_normal) > 0.0;
        let dot_top = point.dot(self.top_normal) > 0.0;
        let dot_bottom = point.dot(self.bottom_normal) > 0.0;

        // If all dot products are negative, object is in front of all planes (thus in frustrum)
        dot_right && dot_left && dot_top && dot_bottom
    }

    pub fn aabb_in_frustrum(&self, aabb: &AABB) -> bool {
        let min = Vec3::from(aabb.min);
        let max = Vec3::from(aabb.max);

        // If either point of bounding box is in frustrum, object is at least partially in frustrum
        if !self.point_in_frustrum(min) && !self.point_in_frustrum(max) {
            return false;
        }

        true
        // let min_z2: Vec3 = min - self.origin;
        // let min_z2: f32 = min_z2.dot(min_z2);

        // let max_z2: Vec3 = max - self.origin;
        // let max_z2: f32 = max_z2.dot(max_z2);

        // let min_inside = min_z2 > self.near2 && min_z2 < self.far2;
        // let max_inside = max_z2 > self.near2 && max_z2 < self.far2;
        // let both_outside = min_z2 < self.near2 && max_z2 > self.far2;

        // min_inside || max_inside || both_outside
    }
}
