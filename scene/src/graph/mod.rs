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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NodeMeshType {
    Static,
    Animated,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct NodeMesh {
    pub mesh_type: NodeMeshType,
    pub object_id: u32,
}

impl std::fmt::Display for NodeMeshType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            NodeMeshType::Static => "Static",
            NodeMeshType::Animated => "Animated",
        })
    }
}

impl std::fmt::Display for NodeMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeMesh {{ mesh_type: {}, object_id: {} }}", self.mesh_type, self.object_id)
    }
}

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Node {
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
    matrix: Mat4,
    local_matrix: Mat4,
    pub combined_matrix: Mat4,
    pub weights: Vec<f32>,
    pub instance_id: Option<u32>,
    pub object_id: Option<NodeMesh>,
    pub target_meshes: Vec<u32>,
    pub target_skins: Vec<u32>,
    pub child_nodes: Vec<u32>,
    pub flags: Flags,
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
            local_matrix: Mat4::identity(),
            combined_matrix: Mat4::identity(),
            weights: Vec::new(),
            instance_id: None,
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

    pub fn set_matrix(&mut self, matrix: Mat4) {
        self.matrix = matrix;
        self.flags.set_flag(NodeFlags::Transformed);
    }

    pub fn update_matrix(&mut self) {
        if self.flags.has_flag(NodeFlags::Transformed) {
            return;
        }

        let trs = Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
        self.local_matrix = trs * self.matrix;
        self.flags.unset_flag(NodeFlags::Transformed);
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

#[cfg_attr(feature = "object_caching", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Skin {
    pub name: String,
    pub joint_nodes: Vec<u32>,
    pub inverse_bind_matrices: Vec<Mat4>,
    pub joint_matrices: Vec<Mat4>,
}

impl Default for Skin {
    fn default() -> Self {
        Self {
            name: String::new(),
            joint_nodes: Vec::new(),
            inverse_bind_matrices: Vec::new(),
            joint_matrices: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub node_index: u32,

    pub vertex_ids: Vec<u16>,
    pub vertex_weights: Vec<f32>,
    pub offset_matrix: Mat4,
}

impl Default for Bone {
    fn default() -> Self {
        Self {
            name: String::new(),
            node_index: 0,
            vertex_ids: Vec::new(),
            vertex_weights: Vec::new(),
            offset_matrix: Mat4::identity(),
        }
    }
}