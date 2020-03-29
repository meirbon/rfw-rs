use glam::*;
use std::ops::{Index, IndexMut};

pub struct MaterialList {
    materials: Vec<Material>,
}

#[allow(dead_code)]
impl MaterialList {
    pub fn new() -> MaterialList {
        let materials = vec![Material::new(vec3(1.0, 0.0, 0.0), 1.0, vec3(1.0, 0.0, 0.0), 1.0)];
        MaterialList { materials }
    }

    pub fn add(&mut self, color: Vec3, roughness: f32, specular: Vec3, opacity: f32) -> usize {
        let material = Material::new(color, roughness, specular, opacity);
        self.push(material)
    }

    pub fn push(&mut self, mat: Material) -> usize {
        let i = self.materials.len();
        self.materials.push(mat);
        i
    }

    pub fn get(&self, index: usize) -> Option<&Material> {
        self.materials.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Material> {
        self.materials.get_mut(index)
    }

    pub unsafe fn get_unchecked(&mut self, index: usize) -> &Material {
        self.materials.get_unchecked(index)
    }

    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &Material {
        self.materials.get_unchecked_mut(index)
    }

    pub fn get_default(&self) -> usize {
        0
    }
}

impl Index<usize> for MaterialList {
    type Output = Material;

    fn index(&self, index: usize) -> &Self::Output {
        &self.materials[index]
    }
}

impl IndexMut<usize> for MaterialList {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.materials[index]
    }
}

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