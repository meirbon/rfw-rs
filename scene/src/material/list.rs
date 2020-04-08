use crate::material::Material;

use glam::*;
use std::ops::{Index, IndexMut};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub unsafe fn get_unchecked(&self, index: usize) -> &Material {
        self.materials.get_unchecked(index)
    }

    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &Material {
        self.materials.get_unchecked_mut(index)
    }

    pub fn get_default(&self) -> usize {
        0
    }

    #[cfg(feature = "wgpu")]
    pub fn create_wgpu_buffer(&self, device: &wgpu::Device) -> (wgpu::BufferAddress, wgpu::Buffer) {
        use wgpu::*;

        let size = (self.materials.len() * std::mem::size_of::<Material>()) as BufferAddress;
        let buffer = device.create_buffer_mapped(&wgpu::BufferDescriptor {
            label: Some("material-buffer"),
            size,
            usage: BufferUsage::STORAGE_READ
        });

        buffer.data.copy_from_slice(unsafe {
            std::slice::from_raw_parts(
                self.materials.as_ptr() as *const u8,
                size as usize
            )
        });

        (size, buffer.finish())
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