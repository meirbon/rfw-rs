use crate::{Mesh, ToMesh};
use glam::*;

pub struct Quad {
    pub normal: Vec3A,
    pub position: Vec3A,
    pub width: f32,
    pub height: f32,
    pub material_id: u32,

    vertices: [Vec3A; 6],
    normals: [Vec3A; 6],
    uvs: [Vec2; 6],
    material_ids: [u32; 2],
}

#[allow(dead_code)]
impl Quad {
    pub fn new<T: Into<[f32; 3]>>(
        normal: T,
        position: T,
        width: f32,
        height: f32,
        material_id: u32,
    ) -> Quad {
        let material_id = material_id.max(0);
        // TODO: uvs
        let uvs = [vec2(0.0, 0.0); 6];
        let material_ids = [material_id as u32; 2];

        let normal = Vec3A::from(normal.into());
        let position = Vec3A::from(position.into());

        let (vertices, normals) = Quad::generate_render_data(position, normal, width, height);

        Quad {
            normal,
            position,
            width,
            height,
            material_id,

            vertices,
            normals,
            uvs,
            material_ids,
        }
    }

    fn generate_render_data(pos: Vec3A, n: Vec3A, width: f32, height: f32) -> ([Vec3A; 6], [Vec3A; 6]) {
        let normal = n.normalize();
        let tmp = if normal.x() > 0.9 {
            Vec3A::new(0.0, 1.0, 0.0)
        } else {
            Vec3A::new(1.0, 0.0, 0.0)
        };

        let tangent: Vec3A = 0.5 * width * normal.cross(tmp).normalize();
        let bi_tangent: Vec3A = 0.5 * height * tangent.normalize().cross(normal);

        let vertices: [Vec3A; 6] = [
            pos - bi_tangent - tangent,
            pos + bi_tangent - tangent,
            pos - bi_tangent + tangent,
            pos + bi_tangent - tangent,
            pos + bi_tangent + tangent,
            pos - bi_tangent + tangent,
        ];

        let normals = [normal.clone(); 6];

        (vertices, normals)
    }
}

impl ToMesh for Quad {
    fn into_mesh(self) -> Mesh {
        Mesh::new(
            self.vertices.to_vec(),
            self.normals.to_vec(),
            self.uvs.to_vec(),
            self.material_ids.to_vec(),
            Some("quad"),
        )
    }
}
