use crate::utils::*;
use glam::*;

pub mod animation;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum NodeFlags {
    Transformed = 0,
    Morphed = 1,
}

impl Into<u8> for NodeFlags {
    fn into(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
    matrix: Mat4,
    combined_matrix: Mat4,
    weights: Vec<f32>,

    object_id: Option<usize>,
    target_meshes: Vec<u32>,
    target_skins: Vec<u32>,
    child_nodes: Vec<u32>,
    flags: Flags,
}

impl Default for Node {
    fn default() -> Self {
        let mut flags = Flags::new();
        flags.set_flag(NodeFlags::Transformed);

        Self {
            translation: Vec3::zero(),
            rotation: Quat::identity(),
            scale: Vec3::splat(1.0),
            matrix: Mat4::identity(),
            combined_matrix: Mat4::identity(),
            weights: Vec::new(),

            object_id: None,
            target_meshes: Vec::new(),
            target_skins: Vec::new(),
            child_nodes: Vec::new(),
            flags,
        }
    }
}

impl Node {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_translation(&mut self, t: Vec3) {
        self.translation = t;
        self.flags.set_flag(NodeFlags::Transformed);
    }

    pub fn set_rotation(&mut self, r: Quat) {
        self.rotation = r;
        self.flags.set_flag(NodeFlags::Transformed);
    }

    pub fn set_scale(&mut self, s: Vec3) {
        self.scale = s;
        self.flags.set_flag(NodeFlags::Transformed);
    }
}

pub struct NodeGraph {
    nodes: FlaggedStorage<Node>,
    root_nodes: FlaggedStorage<u32>,
}

impl NodeGraph {
    pub fn update(&mut self) -> bool {
        let mut changed = false;
        for root_node in self.root_nodes.iter() {
            changed |= Self::traverse_children(
                (*root_node) as usize,
                Mat4::identity(),
                self.nodes.as_mut_slice(),
            );
        }

        changed
    }

    fn traverse_children(current_index: usize, matrix: Mat4, nodes: &mut [Node]) -> bool {
        let mut changed = false;

        // Update matrix
        let combined_matrix = matrix * nodes[current_index].matrix;
        nodes[current_index].combined_matrix = combined_matrix;

        let child_nodes = nodes[current_index].child_nodes.clone();
        // Update children
        for c_id in child_nodes.into_iter() {
            let c_id = c_id as usize;
            changed |= Self::traverse_children(c_id, combined_matrix, nodes);
        }

        // Return whether this node or its children changed
        changed
    }
}
