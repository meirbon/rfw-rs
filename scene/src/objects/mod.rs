use glam::*;

pub mod instance;
pub mod mesh;
pub mod plane;
pub mod quad;
pub mod sphere;
pub mod triangle;

use crate::PrimID;
use rtbvh::{Bounds, Ray, RayPacket4};
pub use instance::*;
pub use mesh::*;
pub use plane::*;
pub use quad::*;
pub use sphere::*;
pub use triangle::*;


#[cfg(feature = "object_caching")]
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Debug)]
pub struct HitRecord {
    pub normal: [f32; 3],
    pub t: f32,
    pub p: [f32; 3],
    pub mat_id: u32,
    pub g_normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Default for HitRecord {
    fn default() -> Self {
        Self {
            normal: [0.0; 3],
            t: 0.0,
            p: [0.0; 3],
            mat_id: 0,
            g_normal: [0.0; 3],
            uv: [0.0; 2],
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct HitRecord4 {
    pub normal_x: [f32; 4],
    pub normal_y: [f32; 4],
    pub normal_z: [f32; 4],
    pub t: [f32; 4],
    pub p_x: [f32; 4],
    pub p_y: [f32; 4],
    pub p_z: [f32; 4],
    pub mat_id: [u32; 4],
    pub g_normal_x: [f32; 4],
    pub g_normal_y: [f32; 4],
    pub g_normal_z: [f32; 4],
    pub u: [f32; 4],
    pub v: [f32; 4],
}

impl Default for HitRecord4 {
    fn default() -> Self {
        Self {
            normal_x: [0.0; 4],
            normal_y: [0.0; 4],
            normal_z: [0.0; 4],
            t: [0.0; 4],
            p_x: [0.0; 4],
            p_y: [0.0; 4],
            p_z: [0.0; 4],
            mat_id: [0; 4],
            g_normal_x: [0.0; 4],
            g_normal_y: [0.0; 4],
            g_normal_z: [0.0; 4],
            u: [0.0; 4],
            v: [0.0; 4],
        }
    }
}

impl From<[HitRecord; 4]> for HitRecord4 {
    fn from(hits: [HitRecord; 4]) -> Self {
        let normal_x: [f32; 4] = [
            hits[0].normal[0],
            hits[1].normal[0],
            hits[2].normal[0],
            hits[3].normal[0],
        ];
        let normal_y: [f32; 4] = [
            hits[0].normal[1],
            hits[1].normal[1],
            hits[2].normal[1],
            hits[3].normal[1],
        ];
        let normal_z: [f32; 4] = [
            hits[0].normal[2],
            hits[1].normal[2],
            hits[2].normal[2],
            hits[3].normal[2],
        ];

        let t: [f32; 4] = [hits[0].t, hits[1].t, hits[2].t, hits[3].t];

        let p_x: [f32; 4] = [hits[0].p[0], hits[1].p[0], hits[2].p[0], hits[3].p[0]];
        let p_y: [f32; 4] = [hits[0].p[1], hits[1].p[1], hits[2].p[1], hits[3].p[1]];
        let p_z: [f32; 4] = [hits[0].p[2], hits[1].p[2], hits[2].p[2], hits[3].p[2]];

        let mat_id: [u32; 4] = [
            hits[0].mat_id,
            hits[1].mat_id,
            hits[2].mat_id,
            hits[3].mat_id,
        ];

        let g_normal_x: [f32; 4] = [
            hits[0].g_normal[0],
            hits[1].g_normal[0],
            hits[2].g_normal[0],
            hits[3].g_normal[0],
        ];
        let g_normal_y: [f32; 4] = [
            hits[0].g_normal[1],
            hits[1].g_normal[1],
            hits[2].g_normal[1],
            hits[3].g_normal[1],
        ];
        let g_normal_z: [f32; 4] = [
            hits[0].g_normal[2],
            hits[1].g_normal[2],
            hits[2].g_normal[2],
            hits[3].g_normal[2],
        ];

        let u: [f32; 4] = [hits[0].uv[0], hits[1].uv[0], hits[2].uv[0], hits[3].uv[0]];
        let v: [f32; 4] = [hits[0].uv[1], hits[1].uv[1], hits[2].uv[1], hits[3].uv[1]];

        Self {
            normal_x,
            normal_y,
            normal_z,
            t,
            p_x,
            p_y,
            p_z,
            mat_id,
            g_normal_x,
            g_normal_y,
            g_normal_z,
            u,
            v,
        }
    }
}

impl HitRecord4 {
    pub fn normal(&self, index: usize) -> Vec3 {
        debug_assert!(index < 4);
        Vec3::new(
            self.normal_x[index],
            self.normal_y[index],
            self.normal_z[index],
        )
    }

    pub fn t(&self, index: usize) -> f32 {
        debug_assert!(index < 4);
        self.t[index]
    }

    pub fn p(&self, index: usize) -> Vec3 {
        debug_assert!(index < 4);
        Vec3::new(self.p_x[index], self.p_y[index], self.p_z[index])
    }

    pub fn g_normal(&self, index: usize) -> Vec3 {
        debug_assert!(index < 4);
        Vec3::new(
            self.g_normal_x[index],
            self.g_normal_y[index],
            self.g_normal_z[index],
        )
    }

    pub fn uv(&self, index: usize) -> Vec2 {
        debug_assert!(index < 4);
        Vec2::new(self.u[index], self.v[index])
    }
}

pub trait Intersect: Bounds + Send + Sync {
    fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool;
    fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord>;
    fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32>;
    fn depth_test(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<(f32, u32)>;
    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[PrimID; 4]>;
    fn get_hit_record(&self, ray: Ray, t: f32, hit_data: u32) -> HitRecord;
    fn get_mat_id(&self, prim_id: PrimID) -> u32;
}

#[cfg(feature = "object_caching")]
pub trait SerializableObject<'a, T: Serialize + Deserialize<'a>> {
    fn serialize_object<S: AsRef<std::path::Path>>(
        &self,
        path: S,
        materials: &crate::MaterialList,
    ) -> Result<(), Box<dyn std::error::Error>>;
    fn deserialize_object<S: AsRef<std::path::Path>>(
        path: S,
        materials: &mut crate::MaterialList,
    ) -> Result<T, Box<dyn std::error::Error>>;
}
