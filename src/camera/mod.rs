use nalgebra_glm::*;
use std::f32::consts::PI;

#[derive(Copy, Clone)]
pub struct Ray {
    pub origin: Vec3,
    pub t: f32,
    pub direction: Vec3,
    pub hit_obj: i32,
}

#[allow(dead_code)]
impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Ray {
        Ray {
            origin,
            t: 1e34 as f32,
            direction,
            hit_obj: -1,
        }
    }

    pub fn reflect_self(&mut self, p: &Vec3, n: &Vec3) {
        self.direction = self.direction - n * dot(n, &self.direction);
        self.origin = *p + self.direction * crate::constants::EPSILON;
        self.reset();
    }

    pub fn reflect(&self, p: &Vec3, n: &Vec3) -> Ray {
        let tmp = n * dot(n, &self.direction) * 2.0;
        let dir = self.direction - tmp;
        Ray::new(p + dir * crate::constants::EPSILON, dir)
    }

    pub fn get_point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    pub fn update_t(&mut self, t: f32, hit_obj: u32) {
        if t > crate::constants::EPSILON && t < self.t {
            self.t = t;
            self.hit_obj = hit_obj as i32;
        }
    }

    pub fn is_valid(&self) -> bool {
        self.hit_obj > -1
    }

    pub fn reset(&mut self) {
        self.t = 1e34 as f32;
        self.hit_obj = -1;
    }
}

pub trait Intersectable: Send + Sync {
    fn set_hit_idx(&mut self, idx: u32);

    fn get_mat_idx(&self) -> u32;

    fn intersect(&self, r: &mut Ray);

    fn get_tex_coordinates(&self, p: &Vec3, n: &Vec3) -> Vec2;

    fn get_normal(&self, p: &Vec3) -> Vec3;

    fn get_centroid(&self) -> Vec3;
}

#[derive(Debug, Clone)]
pub struct Camera {
    pub pos: Vec3,
    up: Vec3,
    view_dir: Vec3,
    fov: f32,
    width: u32,
    height: u32,
    aspect_ratio: f32,
    aperture: f32,
    focal_distance: f32,
}

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

        let u = (x as f32 + r0) * self.inv_width;
        let v = (y as f32 + r1) * self.inv_height;
        let point_on_pixel = self.p1 + u * self.right + v * self.up;
        let direction = normalize(&(point_on_pixel - self.pos));

        Ray::new(self.pos.clone(), direction)
    }

    pub fn generate_ray(&self, x: u32, y: u32) -> Ray {
        let u = x as f32 * self.inv_width;
        let v = y as f32 * self.inv_height;
        let point_on_pixel = self.p1 + u * self.right + v * self.up;
        let direction = normalize(&(point_on_pixel - self.pos));

        Ray::new(self.pos.clone(), direction)
    }
}

#[allow(dead_code)]
impl Camera {
    pub fn new(pos: Vec3, width: u32, height: u32, fov: f32) -> Camera {
        let up = vec3(0.0, 1.0, 0.0);
        let aspect_ratio = width as f32 / height as f32;

        Camera {
            pos,
            up,
            view_dir: vec3(0.0, 0.0, 1.0),
            fov,
            width,
            height,
            aspect_ratio,
            aperture: 0.0001,
            focal_distance: 5.0,
        }
    }

    pub fn get_view(&self) -> CameraView {
        let fov_distance = Camera::get_fov_distance(self.fov);
        let (right, up, forward) = self.calculate_matrix();
        let pos = self.pos.clone();
        let spread_angle = (self.fov * PI / 180.0) * (1.0 / self.height as f32);
        let screen_size = (self.fov / 2.0 / (180.0 / PI)).tan();
        let center = self.pos + fov_distance * self.view_dir;
        let p1 = center - screen_size * right * fov_distance * self.aspect_ratio
            + screen_size * fov_distance * up;
        let p2 = center
            + screen_size * right * fov_distance * self.aspect_ratio
            + screen_size * fov_distance * up;
        let p3 = center
            - screen_size * right * fov_distance * self.aspect_ratio
            - screen_size * fov_distance * up;
        let aperture = self.aperture;

        CameraView {
            pos,
            lens_size: aperture,
            right: normalize(&(p2 - p1)),
            spread_angle,
            up: normalize(&(p3 - p1)),
            epsilon: 1e-5,
            p1,
            inv_width: 1.0 / self.width as f32,
            inv_height: 1.0 / self.height as f32,
        }
    }

    // pub fn generate_ray(&self, x: u32, y: u32) -> Ray {
    //     let pixel_x = (x as f32) * self.inv_width;
    //     let pixel_y = (y as f32) * self.inv_height;

    //     let screen_x = 2.0 * pixel_x - 1.0;
    //     let screen_y = 1.0 - 2.0 * pixel_y;

    //     let ray_dir: Vec3 = normalize(&(self.view_dir + self.u * screen_x + self.v * screen_y));

    //     Ray::new(self.pos.clone(), ray_dir)
    // }

    fn get_view_direction(pitch: f32, yaw: f32) -> Vec3 {
        normalize(&vec3(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            -1.0 * yaw.cos() * pitch.cos(),
        ))
    }

    fn get_fov_distance(fov: f32) -> f32 {
        (fov.to_radians() * 0.5).atan()
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

    pub fn translate_relative(&mut self, delta: &Vec3) {
        let delta_x = delta.x * cross(&self.view_dir, &self.up);
        let delta_y = delta.y * self.up;
        let delta_z = delta.z * self.view_dir;

        self.pos = self.pos + delta_x + delta_y + delta_z;
    }

    pub fn translate_target(&mut self, delta: &Vec3) {
        let (right, up, forward) = self.calculate_matrix();
        let new_dir: Vec3 = self.view_dir + delta.x * right + delta.y * up + delta.z * forward;
        self.view_dir = normalize(&new_dir);
    }

    pub fn look_at(&mut self, origin: &Vec3, target: &Vec3) {
        self.pos = (*origin).clone();
        self.view_dir = normalize(&(target - origin));
    }

    pub fn get_matrix(&self, near_plane: f32, far_plane: f32) -> Mat4 {
        let up = vec3(0.0, 1.0, 0.0);
        let fov_dist = (self.fov * 0.5).to_radians().tan();

        let identity: Mat4 = identity();
        let flip = scale(&identity, &vec3(-1.0, -1.0, -1.0));
        let projection = perspective(
            self.aspect_ratio,
            self.fov.to_radians(),
            near_plane,
            far_plane,
        );
        let view = look_at(&self.pos, &(self.pos + self.view_dir * fov_dist), &up);

        projection * flip * view
    }

    fn calculate_matrix(&self) -> (Vec3, Vec3, Vec3) {
        let y = vec3(0.0, 1.0, 0.0);
        let z = self.view_dir.clone();
        let x = normalize(&cross(&z, &y));
        let y = cross(&x, &z);
        (x, y, z)
    }
}
