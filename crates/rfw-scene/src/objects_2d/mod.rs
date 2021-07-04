use rfw_backend::*;
use rfw_math::*;

mod quad;
pub use quad::*;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Mesh2D {
    pub vertices: Vec<Vertex2D>,
    pub triangles: Vec<RTTriangle2D>,
    pub tex_id: Option<usize>,
}

impl Default for Mesh2D {
    fn default() -> Self {
        Self {
            vertices: Vec::new(),
            triangles: Vec::new(),
            tex_id: None,
        }
    }
}

impl Mesh2D {
    pub fn new(vertices: Vec<Vec3>, uvs: Vec<Vec2>, tex_id: Option<usize>, color: Vec4) -> Self {
        let uvs = if !uvs.is_empty() {
            assert_eq!(vertices.len(), uvs.len());
            uvs
        } else {
            vec![Vec2::ZERO; vertices.len()]
        };

        let tex = if let Some(id) = tex_id { id as u32 } else { 0 };
        let vertices: Vec<Vertex2D> = vertices
            .iter()
            .zip(uvs.iter())
            .map(|(v, t)| Vertex2D {
                vertex: *v,
                tex,
                uv: *t,
                color,
            })
            .collect();

        let triangles = vertices
            .chunks_exact(3)
            .map(|vs| RTTriangle2D {
                normal: RTTriangle2D::normal(vs[0].vertex, vs[1].vertex, vs[2].vertex),
                vertex0: vs[0].vertex,
                vertex1: vs[1].vertex,
                vertex2: vs[2].vertex,
            })
            .collect();

        Self {
            vertices,
            triangles,
            tex_id,
        }
    }

    pub fn set_tex_id(&mut self, id: u32) {
        self.vertices.iter_mut().for_each(|v| {
            v.tex = id;
        });
    }

    pub fn set_color(&mut self, color: Vec4) {
        self.vertices.iter_mut().for_each(|v| {
            v.color = color;
        });
    }

    pub fn update_triangles(&mut self) {
        self.triangles
            .resize(self.vertices.len() / 3, RTTriangle2D::default());

        for (triangle, vs) in self.triangles.iter_mut().zip(self.vertices.chunks_exact(3)) {
            *triangle = RTTriangle2D {
                normal: RTTriangle2D::normal(vs[0].vertex, vs[1].vertex, vs[2].vertex),
                vertex0: vs[0].vertex,
                vertex1: vs[1].vertex,
                vertex2: vs[2].vertex,
            };
        }
    }
}

impl From<Vec<Vertex2D>> for Mesh2D {
    fn from(vec: Vec<Vertex2D>) -> Self {
        let s = Self {
            vertices: vec,
            triangles: Vec::new(),
            tex_id: None,
        };

        s
    }
}

impl From<&[Vertex2D]> for Mesh2D {
    fn from(vec: &[Vertex2D]) -> Self {
        let s = Self {
            vertices: vec.to_vec(),
            triangles: Vec::new(),
            tex_id: None,
        };

        s
    }
}

pub trait ToMesh2D {
    fn into_mesh_2d(self) -> Mesh2D;
}

impl ToMesh2D for Mesh2D {
    fn into_mesh_2d(self) -> Mesh2D {
        self
    }
}
