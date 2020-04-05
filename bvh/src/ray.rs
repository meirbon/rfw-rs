use glam::*;

#[derive(Copy, Clone)]
pub struct Ray {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
}

#[derive(Clone)]
pub struct RayPacket4 {
    pub origin_x: [f32; 4],
    pub origin_y: [f32; 4],
    pub origin_z: [f32; 4],

    pub direction_x: [f32; 4],
    pub direction_y: [f32; 4],
    pub direction_z: [f32; 4],

    pub t: [f32; 4],
    pub hit_id: [i32; 4],
    pub instance_id: [i32; 4],
    pub pixel_ids: [u32; 4],
}

impl RayPacket4 {
    pub fn new() -> RayPacket4 {
        Self {
            origin_x: [0.0; 4],
            origin_y: [0.0; 4],
            origin_z: [0.0; 4],
            direction_x: [0.0; 4],
            direction_y: [0.0; 4],
            direction_z: [0.0; 4],
            t: [0.0; 4],
            hit_id: [-1; 4],
            instance_id: [-1; 4],
            pixel_ids: [0; 4],
        }
    }
}

#[derive(Copy, Clone)]
pub struct ShadowPacket4 {
    pub origin_x: [f32; 4],
    pub origin_y: [f32; 4],
    pub origin_z: [f32; 4],
    pub direction_x: [f32; 4],
    pub direction_y: [f32; 4],
    pub direction_z: [f32; 4],
    pub t_max: [f32; 4],
}

#[allow(dead_code)]
impl Ray {
    pub fn new(origin: [f32; 3], direction: [f32; 3]) -> Ray {
        Ray { origin, direction }
    }

    pub fn reflect(&self, p: &[f32; 3], n: &[f32; 3], epsilon: f32) -> Ray {
        let p = Vec3::from(*p);
        let n = Vec3::from(*n);

        let direction = Vec3::from(self.direction);

        let tmp: Vec3 = n * n.dot(direction) * 2.0;
        let direction = direction - tmp;

        Ray {
            origin: (p + direction * epsilon).into(),
            direction: direction.into(),
        }
    }

    pub fn get_point_at(&self, t: f32) -> Vec3 {
        Vec3::from(self.origin) + Vec3::from(self.direction) * t
    }
}
