use crate::{triangle_scene::SceneError, Material, MaterialList};
use glam::*;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

pub struct gLTFObject {
    vertices: Vec<Vec3>,
    normals: Vec<Vec3>,
    indices: Vec<[u32; 3]>,
    tex_coords: Vec<Vec2>,
}

impl gLTFObject {
    pub fn new<T: AsRef<Path>>(
        path: T,
        mat_manager: Arc<Mutex<MaterialList>>,
    ) -> Result<Self, SceneError> {
        let object = gltf::import(path.as_ref());
        if let Err(_) = object {
            return Err(SceneError::LoadError(path.as_ref().to_path_buf()));
        }
        let (document, data, a) = object.unwrap();
        document.materials().for_each(|m| {
            let mut material = Material::default();
            material.name = m.name().unwrap_or("").to_string();
        });

        let mut vertices = Vec::new();
        let mut normals = Vec::new();
        let mut indices = Vec::new();
        let mut tex_coords = Vec::new();

        Ok(Self {
            vertices,
            normals,
            indices,
            tex_coords,
        })
    }
}
