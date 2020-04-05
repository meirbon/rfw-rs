use glam::*;

pub mod list;

pub use list::*;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Material {
    pub color: Vec3,
    pub specular: Vec3,

    pub opacity: f32,
    pub roughness: f32,
    pub diffuse_tex: i32,
    pub normal_tex: i32,
}

impl Material {
    pub fn new(color: Vec3, roughness: f32, specular: Vec3, opacity: f32) -> Material {
        Material {
            color,
            roughness,
            specular,
            opacity,
            diffuse_tex: -1,
            normal_tex: -1,
        }
    }
}