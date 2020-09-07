use glam::*;

#[cfg(feature = "object_caching")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct D2Mesh {
    pub vertices: Vec<D2Vertex>,
    pub tex_id: Option<u32>,
    pub color: [f32; 4],
}

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct D2Vertex {
    vertex: [f32; 3],
    tex_id: u32,
    uv: [f32; 2],
}

impl Default for D2Mesh {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            tex_id: None,
            color: [0.0_f32; 4],
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
                tex_id: tex,
                uv: *t,
            })
            .collect();

        Self {
            vertices,
            tex_id,
            color,
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
