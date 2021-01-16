use rayon::prelude::*;
use rfw_backend::*;
use rfw_math::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};


#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Mesh2D {
    pub vertices: Vec<Vertex2D>,
    pub tex_id: Option<usize>,
}

impl Default for Mesh2D {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            tex_id: None,
        }
    }
}

impl Mesh2D {
    pub fn new(
        vertices: Vec<[f32; 3]>,
        uvs: Vec<[f32; 2]>,
        tex_id: Option<usize>,
        color: [f32; 4],
    ) -> Self {
        let uvs = if !uvs.is_empty() {
            assert_eq!(vertices.len(), uvs.len());
            uvs
        } else {
            vec![[0.0; 2]; vertices.len()]
        };

        let tex = if let Some(id) = tex_id { id as u32 } else { 0 };
        let vertices = vertices
            .iter()
            .zip(uvs.iter())
            .map(|(v, t)| Vertex2D {
                vertex: *v,
                has_tex: tex,
                uv: *t,
                color,
            })
            .collect();

        Self {
            vertices,
            tex_id,
        }
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

impl From<Vec<Vertex2D>> for Mesh2D {
    fn from(vec: Vec<Vertex2D>) -> Self {
        Self {
            vertices: vec,
            tex_id: None,
        }
    }
}

impl From<&[Vertex2D]> for Mesh2D {
    fn from(vec: &[Vertex2D]) -> Self {
        Self {
            vertices: vec.to_vec(),
            tex_id: None,
        }
    }
}