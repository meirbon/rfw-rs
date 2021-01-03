use rfw_scene::bvh::AABB;
use rfw_scene::{
    mesh::VertexMesh, JointData, Mesh2D, Mesh3D, RTTriangle, Skin, SkinnedMesh3D,
    SkinnedTriangles3D, Vertex2D, Vertex3D,
};

#[derive(Debug, Clone)]
pub struct Mesh3dData<'a> {
    pub name: String,
    pub bounds: AABB,
    pub vertices: &'a [Vertex3D],
    pub triangles: &'a [RTTriangle],
    pub ranges: &'a [VertexMesh],
    pub skin_data: &'a [JointData],
}

impl<'a> Mesh3dData<'a> {
    pub fn apply_skin_vertices(&self, skin: &Skin) -> SkinnedMesh3D {
        SkinnedMesh3D::apply(self.vertices, self.skin_data, self.ranges, skin)
    }

    pub fn apply_skin_triangles(&self, skin: &Skin) -> SkinnedTriangles3D {
        SkinnedTriangles3D::apply(self.triangles, self.skin_data, skin)
    }
}

impl<'a> From<&'a Mesh3D> for Mesh3dData<'a> {
    fn from(m: &'a Mesh3D) -> Self {
        Self {
            name: m.name.clone(),
            bounds: m.bounds.clone(),
            vertices: m.vertices.as_slice(),
            triangles: m.triangles.as_slice(),
            ranges: m.ranges.as_slice(),
            skin_data: m.skin_data.as_slice(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mesh2dData<'a> {
    pub vertices: &'a [Vertex2D],
    pub tex_id: Option<u32>,
}

impl<'a> From<&'a Mesh2D> for Mesh2dData<'a> {
    fn from(m: &'a Mesh2D) -> Self {
        Self {
            vertices: m.vertices.as_slice(),
            tex_id: m.tex_id,
        }
    }
}
