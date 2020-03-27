use crate::bvh::*;
use crate::math::*;

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

    pub fn intersect(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        let a = dot(direction, direction);
        let r_pos = origin - self.pos;

        let b = (direction * 2.0).dot(&r_pos);
        let r_pos2 = r_pos.dot(&r_pos);
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

    pub fn get_uv(&self, n: Vec3) -> Vec2 {
        let u = n.x.atan2(n.z) * (1.0 / (2.0 * std::f32::consts::PI)) + 0.5;
        let v = n.y * 0.5 + 0.5;

        vec2(u, v)
    }

    pub fn get_normal(&self, p: Vec3) -> Vec3 {
        let dir: Vec3 = p - &self.pos;
        dir.normalize()
    }
}

pub struct RayHit {
    pub normal: Vec3,
    pub t: f32,
    pub uv: Vec2,
}

pub struct Scene {
    pub spheres: Vec<Sphere>,
    pub bvh: Option<BVH>,
}

impl Scene {
    pub fn new() -> Scene {
        Scene {
            spheres: vec![],
            bvh: None,
        }
    }

    pub fn intersect(&self, origin: Vec3, direction: Vec3) -> Option<RayHit> {
        if let Some(bvh) = &self.bvh {
            return BVHNode::traverse(bvh.nodes.as_slice(), bvh.prim_indices.as_slice(), origin, direction, 1e-5,
                                     |i| { self.spheres[i].intersect(origin, direction) },
                                     |i, t, p| -> Vec3{ self.spheres[i].get_normal(p) },
                                     |i, t, p, n| -> Vec2 { self.spheres[i].get_uv(n) },
            );
        }

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
            let normal = sphere.get_normal(p);
            let uv = sphere.get_uv(normal);

            return Some(RayHit { normal, t, uv });
        }

        None
    }

    pub fn depth_test(&self, origin: Vec3, direction: Vec3) -> u32 {
        let mut depth = 0;
        if let Some(bvh) = &self.bvh {
            // let mut t = 1e34;
            // let dir_inverse = vec3(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z);
            // if bvh.nodes[0].bounds.intersect(origin, dir_inverse, 1e34).is_some() {
            //     depth = 1 +
            //         bvh.nodes[0].depth_test_recursive(bvh.nodes.as_slice(), bvh.prim_indices.as_slice(), origin, direction, 1e-5, &mut t, |i| { self.spheres[i].intersect(origin, direction) });
            // }
            depth += BVHNode::depth_test(bvh.nodes.as_slice(), bvh.prim_indices.as_slice(), origin, direction, 1e-5,
                                         |i| { self.spheres[i].intersect(origin, direction) },
            );
        }
        depth
    }


    pub fn build_bvh(&mut self) {
        let mut aabbs = Vec::with_capacity(self.spheres.len());
        for sphere in &self.spheres {
            let mut aabb = AABB::new();
            let radius = sphere.radius2.sqrt();
            let radius = radius + crate::constants::EPSILON;

            let min = sphere.pos - radius;
            let max = sphere.pos + radius;

            aabb.min = min;
            aabb.max = max;

            aabbs.push(aabb);
        }

        let mut bvh = BVH::new(self.spheres.len());
        bvh.build(aabbs.as_slice());
        self.bvh = Some(bvh);
    }
}

