use nalgebra_glm::*;

pub struct Sphere {
    pos: Vec3,
    radius2: f32,
    pub mat_id: u32,
}

impl Sphere {
    pub fn new(pos: Vec3, radius: f32, mat_id: u32) -> Sphere {
        Sphere {
            pos,
            radius2: radius * radius,
            mat_id,
        }
    }

    pub fn intersect(&self, origin: &Vec3, direction: &Vec3) -> Option<f32> {
        let a = dot(direction, direction);
        let r_pos: Vec3 = origin - &self.pos;

        let b = dot(&(direction * 2.0), &r_pos);
        let r_pos2 = dot(&r_pos, &r_pos);
        let c = r_pos2 - self.radius2;

        let d: f32 = (b * b) - (4.0 * a * c);

        if d < 0.0 {
            return None;
        }

        let div_2a = 1.0 / (2.0 * a);

        let sqrt_d = if d > 0.0 { d.sqrt() } else { 0.0 };

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;

        if t1 > 0.0 && t1 < t2 { Some(t1) } else { Some(t2) }
    }

    pub fn tex_coordinates(&self, n: &Vec3) -> Vec2 {
        let u = n.x.atan2(n.z) * (1.0 / (2.0 * std::f32::consts::PI)) + 0.5;
        let v = n.y * 0.5 + 0.5;

        vec2(u, v)
    }

    pub fn get_normal(&self, p: &Vec3) -> Vec3 {
        let dir: Vec3 = p - &self.pos;
        normalize(&dir)
    }
}

pub struct RayHit {
    pub normal: Vec3,
    pub t: f32,
    pub uv: Vec2,
}

pub struct Scene {
    pub spheres: Vec<Sphere>
}

impl Scene {
    pub fn new() -> Scene {
        Scene {
            spheres: vec![]
        }
    }

    pub fn intersect(&self, origin: &Vec3, direction: &Vec3) -> Option<RayHit> {
        let mut t = 1e34 as f32;
        let mut hit_id = -1;

        for (i, sphere) in self.spheres.iter().enumerate() {
            if let Some(new_t) = sphere.intersect(origin, direction) {
                if new_t < t {
                    t = new_t;
                    hit_id = i as i32;
                }
            }
        }

        if hit_id >= 0 {
            let p: Vec3 = origin + direction * t;
            let sphere = unsafe { self.spheres.get_unchecked(hit_id as usize) };
            let normal = sphere.get_normal(&p);
            let uv = sphere.tex_coordinates(&normal);

            return Some(RayHit { normal, t, uv });
        }

        None
    }
}

