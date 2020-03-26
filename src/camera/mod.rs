use nalgebra_glm::*;

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
        Ray::new(*p + dir * crate::constants::EPSILON, dir)
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
    pub fov: f32,
    fov_distance: f32,
    width: u32,
    height: u32,
    inv_width: f32,
    inv_height: f32,
    aspect_ratio: f32,
    pub pitch: f32,
    pub yaw: f32,
    rotation_sensitivity: f32,
    movement_speed: f32,
    u: Vec3,
    v: Vec3,
}

#[allow(dead_code)]
impl Camera {
    pub fn new(
        pos: Vec3,
        width: u32,
        height: u32,
        fov: f32,
        rotation_sensitivity: f32,
        movement_speed: f32,
    ) -> Camera {
        let pitch = 0.0;
        let yaw = 0.0;

        let up = vec3(0.0, 1.0, 0.0);
        let view_dir = Camera::get_view_direction(pitch, yaw);
        let fov_distance = Camera::get_fov_distance(fov);
        let aspect_ratio = width as f32 / height as f32;

        let u: Vec3 = normalize(&cross(&view_dir, &up));
        let v: Vec3 = normalize(&cross(&u, &view_dir));

        let u: Vec3 = u * fov_distance * aspect_ratio;
        let v: Vec3 = v * fov_distance;

        Camera {
            pos,
            up,
            view_dir,
            fov,
            fov_distance,
            width,
            height,
            inv_width: 1.0 / width as f32,
            inv_height: 1.0 / height as f32,
            aspect_ratio,
            pitch,
            yaw,
            rotation_sensitivity,
            movement_speed,
            u,
            v,
        }
    }

    pub fn generate_ray(&self, x: u32, y: u32) -> Ray {
        let pixel_x = (x as f32) * self.inv_width;
        let pixel_y = (y as f32) * self.inv_height;

        let screen_x = 2.0 * pixel_x - 1.0;
        let screen_y = 1.0 - 2.0 * pixel_y;

        let ray_dir = normalize(&(self.view_dir + self.u * screen_x + self.v * screen_y));

        Ray::new(self.pos, ray_dir)
    }

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
        self.fov_distance = Camera::get_fov_distance(self.fov);

        let u = normalize(&cross(&self.view_dir, &vec3(0.0, 1.0, 0.0)));
        let v = normalize(&cross(&u, &self.view_dir));

        self.u = u * self.fov_distance * self.aspect_ratio;
        self.v = v * self.fov_distance;
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.inv_width = 1.0 / width as f32;
        self.inv_height = 1.0 / height as f32;
        self.aspect_ratio = width as f32 / height as f32;

        let u = normalize(&cross(&self.view_dir, &vec3(0.0, 1.0, 0.0)));
        let v = normalize(&cross(&u, &self.view_dir));

        self.u = u * self.fov_distance * self.aspect_ratio;
        self.v = v * self.fov_distance;
    }

    pub fn move_relative(&mut self, delta: &Vec3) {
        let delta_x = delta.x * cross(&self.view_dir, &self.up);
        let delta_y = delta.y * self.up;
        let delta_z = delta.z * self.view_dir;

        self.pos = self.pos + delta_x + delta_y + delta_z;
    }

    pub fn rotate(&mut self, x: f32, y: f32) {
        self.pitch += y * self.rotation_sensitivity;
        self.pitch = self
            .pitch
            .clamp(crate::constants::MIN_PITCH, crate::constants::MAX_PITCH);
        self.yaw += x * self.rotation_sensitivity;
        self.view_dir = Camera::get_view_direction(self.pitch, self.yaw);

        let u = normalize(&cross(&self.view_dir, &vec3(0.0, 1.0, 0.0)));
        let v = normalize(&cross(&u, &self.view_dir));

        self.u = u * self.fov_distance * self.aspect_ratio;
        self.v = v * self.fov_distance;
    }
}
