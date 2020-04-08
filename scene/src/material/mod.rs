pub mod list;

pub use list::*;

use glam::*;
use serde::{Serialize, Deserialize};

#[repr(C)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Material {
    pub color: [f32; 4],
    pub specular: [f32; 4],

    pub opacity: f32,
    pub roughness: f32,
    pub diffuse_tex: i32,
    pub normal_tex: i32,
}

impl Material {
    pub fn new(color: Vec3, roughness: f32, specular: Vec3, opacity: f32) -> Material {
        Material {
            color: color.extend(1.0).into(),
            specular: specular.extend(1.0).into(),
            roughness,
            opacity,
            diffuse_tex: -1,
            normal_tex: -1,
        }
    }
}