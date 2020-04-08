use glam::*;

pub mod instance;
pub mod mesh;
pub mod obj;
pub mod sphere;
pub mod triangle;
pub mod plane;
pub mod quad;

use crate::scene::PrimID;
use bvh::{Bounds, Ray, RayPacket4};
pub use instance::*;
pub use mesh::*;
pub use obj::*;
pub use sphere::*;
pub use triangle::*;
pub use plane::*;
pub use quad::*;

use serde::{Serialize, Deserialize};

#[derive(Copy, Clone, Debug)]
pub struct HitRecord {
    pub normal: [f32; 3],
    pub t: f32,
    pub p: [f32; 3],
    pub mat_id: u32,
    pub g_normal: [f32; 3],
    pub uv: [f32; 2],
}

pub trait Intersect: Bounds + Send + Sync {
    fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool;
    fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord>;
    fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32>;
    fn depth_test(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<(f32, u32)>;
    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[PrimID; 4]>;
    fn get_hit_record(&self, ray: Ray, t: f32, hit_data: u32) -> HitRecord;
}

pub trait SerializableObject<'a, T: Serialize + Deserialize<'a>> {
    fn serialize<S: AsRef<std::path::Path>>(&self, path: S) -> Result<(), Box<dyn std::error::Error>>;
    fn deserialize<S: AsRef<std::path::Path>>(path: S) -> Result<T, Box<dyn std::error::Error>>;
}