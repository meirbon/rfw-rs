use rfw_backend::Vertex2D;
use rfw_math::*;

use crate::{Mesh2D, ToMesh2D};

#[derive(Debug, Clone)]
pub struct Quad2D {
    pub bottom_left: Vec2,
    pub top_right: Vec2,
    pub layer: f32,
    pub texture: Option<usize>,
    pub color: Vec4,
    pub bottom_left_uv: Vec2,
    pub top_right_uv: Vec2,
}

impl Default for Quad2D {
    fn default() -> Self {
        Self {
            bottom_left: Vec2::new(-0.5, -0.5),
            top_right: Vec2::new(0.5, 0.5),
            layer: 0.0,
            texture: None,
            color: Vec4::ONE,
            bottom_left_uv: Vec2::ZERO,
            top_right_uv: Vec2::ONE,
        }
    }
}

impl ToMesh2D for Quad2D {
    fn into_mesh_2d(self) -> crate::Mesh2D {
        let mut vertices = Vec::with_capacity(6);
        let tex = self.texture.unwrap_or(0) as u32;
        vertices.push(Vertex2D {
            vertex: self.bottom_left.extend(self.layer).into(),
            tex,
            uv: self.bottom_left_uv.into(),
            color: self.color.into(),
        });
        vertices.push(Vertex2D {
            vertex: Vec2::new(self.top_right.x, self.bottom_left.y)
                .extend(self.layer)
                .into(),
            tex,
            uv: Vec2::new(self.top_right_uv.x, self.bottom_left_uv.y).into(),
            color: self.color.into(),
        });
        vertices.push(Vertex2D {
            vertex: self.top_right.extend(self.layer).into(),
            tex,
            uv: self.top_right_uv.into(),
            color: self.color.into(),
        });
        vertices.push(Vertex2D {
            vertex: self.bottom_left.extend(self.layer).into(),
            tex,
            uv: self.bottom_left_uv.into(),
            color: self.color.into(),
        });
        vertices.push(Vertex2D {
            vertex: self.top_right.extend(self.layer).into(),
            tex,
            uv: self.top_right_uv.into(),
            color: self.color.into(),
        });
        vertices.push(Vertex2D {
            vertex: Vec2::new(self.bottom_left.x, self.top_right.y)
                .extend(self.layer)
                .into(),
            tex,
            uv: Vec2::new(self.bottom_left_uv.x, self.top_right_uv.y).into(),
            color: self.color.into(),
        });

        Mesh2D {
            vertices,
            tex_id: self.texture,
        }
    }
}
