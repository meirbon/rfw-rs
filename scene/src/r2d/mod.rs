use glam::*;
use rayon::prelude::*;

#[cfg(feature = "object_caching")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct D2Mesh {
    pub vertices: Vec<D2Vertex>,
    pub tex_id: Option<u32>,
}

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct D2Vertex {
    pub vertex: [f32; 3],
    pub has_tex: u32,
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl Default for D2Mesh {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            tex_id: None,
        }
    }
}

impl D2Mesh {
    pub fn new(
        vertices: Vec<[f32; 3]>,
        uvs: Vec<[f32; 2]>,
        tex_id: Option<u32>,
        color: [f32; 4],
    ) -> Self {
        let uvs = if !uvs.is_empty() {
            assert_eq!(vertices.len(), uvs.len());
            uvs
        } else {
            vec![[0.0; 2]; vertices.len()]
        };

        let tex = if let Some(id) = tex_id { id } else { 0 };
        let vertices = vertices
            .iter()
            .zip(uvs.iter())
            .map(|(v, t)| D2Vertex {
                vertex: *v,
                has_tex: tex,
                uv: *t,
                color,
            })
            .collect();

        Self { vertices, tex_id }
    }

    pub fn set_tex_id(&mut self, id: u32) {
        let has_tex = match id {
            0 => 0,
            _ => 1,
        };
        self.vertices.par_iter_mut().for_each(|v| {
            v.has_tex = has_tex;
        });
    }

    pub fn set_color(&mut self, color: [f32; 4]) {
        self.vertices.par_iter_mut().for_each(|v| {
            v.color = color;
        });
    }
}

impl From<Vec<D2Vertex>> for D2Mesh {
    fn from(vec: Vec<D2Vertex>) -> Self {
        Self {
            vertices: vec,
            tex_id: None,
        }
    }
}

impl From<&[D2Vertex]> for D2Mesh {
    fn from(vec: &[D2Vertex]) -> Self {
        Self {
            vertices: vec.to_vec(),
            tex_id: None,
        }
    }
}

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct D2Instance {
    pub mesh: Option<u32>,
    pub transform: [f32; 16],
}

impl Default for D2Instance {
    fn default() -> Self {
        Self {
            mesh: None,
            transform: Mat4::identity().to_cols_array(),
        }
    }
}

impl D2Instance {
    pub fn new(mesh: u32) -> Self {
        Self {
            mesh: Some(mesh),
            transform: Mat4::identity().to_cols_array(),
        }
    }

    pub fn with_transform(mut self, transform: [f32; 16]) -> Self {
        self.transform = transform;
        self
    }
}
