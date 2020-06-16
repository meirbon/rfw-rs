use glam::*;

#[derive(Debug, Copy, Clone)]
pub enum Child {
    None,
    Mesh(u32),
    Node(u32),
}

#[derive(Debug, Copy, Clone)]
pub struct Node {
    pub matrix: Mat4,
    pub inverse: Mat4,
    pub child: Child,
    pub skin_id: Option<u32>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            matrix: Mat4::identity(),
            inverse: Mat4::identity(),
            child: Child::None,
            skin_id: None,
        }
    }
}

