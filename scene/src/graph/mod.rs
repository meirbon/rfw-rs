use crate::utils::*;
use crate::{Instance, ObjectRef};
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
pub struct NodeMesh {
    pub object_id: ObjectRef,
    pub instance_id: u32,
}

impl std::fmt::Display for NodeMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NodeMesh {{ object_id: {}, instance_id: {} }}",
            self.object_id, self.object_id
        )
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
    pub skin: Option<u32>,
    pub weights: Vec<f32>,
    pub meshes: Vec<NodeMesh>,
    pub child_nodes: Vec<u32>,
    pub flags: Flags,
    pub name: String,
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
            skin: None,
            weights: Vec::new(),
            meshes: Vec::new(),
            child_nodes: Vec::new(),
            flags,
            name: String::new(),
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
        if !self.flags.has_flag(NodeFlags::Transformed) {
            return;
        }

        let trs =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
        self.local_matrix = trs * self.matrix;
        self.flags.unset_flag(NodeFlags::Transformed);
    }
}

#[derive(Debug, Clone)]
pub struct NodeGraph {
    nodes: FlaggedStorage<Node>,
    root_nodes: FlaggedStorage<u32>,
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self {
            nodes: FlaggedStorage::new(),
            root_nodes: FlaggedStorage::new(),
        }
    }
}

impl NodeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate(&mut self) -> usize {
        self.nodes.allocate()
    }

    pub fn add_root_node(&mut self, id: usize) {
        assert!(self.nodes.get(id).is_some());
        self.root_nodes.push(id as u32);
    }

    pub fn update(
        &mut self,
        instances: &mut TrackedStorage<Instance>,
        skins: &mut TrackedStorage<Skin>,
    ) -> bool {
        let mut changed = false;
        for (_, root_node) in self.root_nodes.iter() {
            changed |= Self::traverse_children(
                (*root_node) as usize,
                Mat4::identity(),
                self.nodes.as_mut_slice(),
                instances,
                skins,
                changed,
            );
        }

        changed
    }

    pub fn get(&self, index: usize) -> Option<&Node> {
        self.nodes.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Node> {
        self.nodes.get_mut(index)
    }

    pub unsafe fn get_unchecked(&self, index: usize) -> &Node {
        self.nodes.get_unchecked(index)
    }

    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut Node {
        self.nodes.get_unchecked_mut(index)
    }

    pub fn as_slice(&self) -> &[Node] {
        self.nodes.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [Node] {
        self.nodes.as_mut_slice()
    }

    pub unsafe fn as_ptr(&self) -> *const Node {
        self.nodes.as_ptr()
    }

    pub unsafe fn as_mut_ptr(&mut self) -> *mut Node {
        self.nodes.as_mut_ptr()
    }

    fn traverse_children(
        current_index: usize,
        matrix: Mat4,
        nodes: &mut [Node],
        instances: &mut TrackedStorage<Instance>,
        skins: &mut TrackedStorage<Skin>,
        changed: bool,
    ) -> bool {
        let mut changed = false;

        if nodes[current_index].flags.has_flag(NodeFlags::Transformed) {
            nodes[current_index].update_matrix();
            changed = true;
        }

        // Update matrix
        let combined_matrix = matrix * nodes[current_index].matrix;
        nodes[current_index].combined_matrix = combined_matrix;

        // Use an unsafe slice to prevent having to copy the vec
        let child_nodes = unsafe {
            std::slice::from_raw_parts(
                nodes[current_index].child_nodes.as_ptr(),
                nodes[current_index].child_nodes.len(),
            )
        };

        // Update children
        for c_id in child_nodes.iter() {
            let c_id = *c_id as usize;
            changed |= Self::traverse_children(c_id, combined_matrix, nodes, instances, skins, changed);
        }

        nodes[current_index].meshes.iter().for_each(|m| {
            if nodes[current_index].flags.has_flag(NodeFlags::Transformed) {
                instances[m.instance_id as usize].set_transform(combined_matrix);
                instances[m.instance_id as usize].skin_id = nodes[current_index].skin;
                // }

                // TODO: Morphed
                // TODO:
                // if nodes[current_index].flags.has_flag(NodeFlags::Morphed) {
            }
        });

        // Update skin
        if let Some(skin) = nodes[current_index].skin {
            if nodes[current_index].flags.has_flag(NodeFlags::Transformed) {
                let skin = &mut skins[skin as usize];
                let inverse_transform = combined_matrix.inverse();
                let inverse_bind_matrices = &skin.inverse_bind_matrices;
                let joint_matrices = &mut skin.joint_matrices;

                skin.joint_nodes
                    .iter()
                    .enumerate()
                    .for_each(|(i, node_id)| {
                        let node_id = *node_id as usize;
                        let joint_node: &Node = &nodes[node_id];
                        joint_matrices[i] = inverse_transform
                            * joint_node.combined_matrix
                            * inverse_bind_matrices[i];
                    });
            }
        }

        nodes[current_index].flags.clear();

        // Return whether this node or its children changed
        changed
    }

    pub fn iter_root_nodes(&self) -> FlaggedIterator<'_, u32> {
        self.root_nodes.iter()
    }
}

impl std::ops::Index<usize> for NodeGraph {
    type Output = Node;
    fn index(&self, index: usize) -> &Self::Output {
        &self.nodes[index]
    }
}

impl std::ops::IndexMut<usize> for NodeGraph {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.nodes[index]
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
