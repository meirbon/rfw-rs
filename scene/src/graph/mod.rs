use crate::utils::*;
use crate::{Instance, ObjectRef};
use glam::*;

pub mod animation;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum NodeFlags {
    First = 0,
    Transformed = 1,
    Morphed = 2,
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
    pub changed: bool,
    pub first: bool,
    pub morhped: bool,
    pub name: String,
}

impl Default for Node {
    fn default() -> Self {
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
            changed: true,
            morhped: false,
            first: true,
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
        self.changed = true;
    }

    pub fn set_rotation(&mut self, r: Quat) {
        self.rotation = r;
        self.changed = true;
    }

    pub fn set_scale(&mut self, s: Vec3) {
        self.scale = s;
        self.changed = true;
    }

    pub fn set_matrix(&mut self, matrix: Mat4) {
        self.matrix = matrix;
        self.local_matrix = self.matrix;
        self.changed = true;
    }

    pub fn update_matrix(&mut self) {
        // let t: glm::Mat4 = glm::translation(&glm::vec3(self.translation.x(), self.translation.y(), self.translation.z()));
        // let r: glm::Mat4 = glm::quat_to_mat4(&glm::quat(self.rotation.x(), self.rotation.y(), self.rotation.z(), self.rotation.w()));
        // let s: glm::Mat4 = glm::scale(&glm::identity(), &glm::vec3(self.scale.x(), self.scale.y(), self.scale.z()));
        // let trs: glm::Mat4 = t * r * s;
        // let trs = Mat4::from_cols_array_2d(trs.as_ref());

        let trs =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
        self.local_matrix = trs * self.matrix;
        self.changed = false;
    }
}

#[derive(Debug, Clone)]
pub struct NodeGraph {
    nodes: TrackedStorage<Node>,
    root_nodes: FlaggedStorage<u32>,
}

impl Default for NodeGraph {
    fn default() -> Self {
        Self {
            nodes: TrackedStorage::new(),
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
                &mut self.nodes,
                instances,
                skins,
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

    pub unsafe fn as_ptr(&self) -> *const Node {
        self.nodes.as_ptr()
    }

    pub unsafe fn as_mut_ptr(&mut self) -> *mut Node {
        self.nodes.as_mut_ptr()
    }

    fn traverse_children(
        current_index: usize,
        accumulated_matrix: Mat4,
        nodes: &mut TrackedStorage<Node>,
        instances: &mut TrackedStorage<Instance>,
        skins: &mut TrackedStorage<Skin>,
    ) -> bool {
        let mut changed = nodes[current_index].changed;
        if changed {
            nodes[current_index].update_matrix();
        }

        let combined_matrix = accumulated_matrix * nodes[current_index].local_matrix;
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
            changed |= Self::traverse_children(
                c_id,
                nodes[current_index].combined_matrix,
                nodes,
                instances,
                skins,
            );
        }

        if !changed && !nodes[current_index].first {
            return false;
        }

        let meshes = &nodes[current_index].meshes;
        let skin = nodes[current_index].skin;
        meshes.iter().for_each(|m| {
            instances[m.instance_id as usize].skin_id = skin;
            instances[m.instance_id as usize].set_transform(combined_matrix);

            // TODO: Morphed
        });

        // Update skin
        if let Some(skin) = nodes[current_index].skin {
            let skin = &mut skins[skin as usize];
            let inverse_transform = combined_matrix.inverse();
            let inverse_bind_matrices = &skin.inverse_bind_matrices;
            let joint_matrices = &mut skin.joint_matrices;

            skin.joint_nodes
                .iter()
                .enumerate()
                .for_each(|(i, node_id)| {
                    let node_id = *node_id as usize;
                    // println!("joint matrix {}, {}", node_id, nodes[node_id].combined_matrix);
                    // println!("\tmatrix {}, {}", node_id, nodes[node_id].matrix);
                    // println!("\tlocal matrix {}, {}", node_id, nodes[node_id].local_matrix);

                    // let t: glm::Mat4 = glm::translate(&glm::identity(),&glm::vec3(nodes[node_id].translation.x(), nodes[node_id].translation.y(), nodes[node_id].translation.z()));
                    // let r: glm::Mat4 = glm::quat_to_mat4(&glm::quat(nodes[node_id].rotation.x(), nodes[node_id].rotation.y(), nodes[node_id].rotation.z(), nodes[node_id].rotation.w()));
                    // let s: glm::Mat4 = glm::scale(&glm::identity(), &glm::vec3(nodes[node_id].scale.x(), nodes[node_id].scale.y(), nodes[node_id].scale.z()));
                    // println!("t: {}", Mat4::from_cols_array_2d(t.as_ref()));
                    // println!("r: {}", Mat4::from_cols_array_2d(r.as_ref()));
                    // println!("s: {}", Mat4::from_cols_array_2d(s.as_ref()));
                    // println!("\ttrue trs {}, {}", node_id, Mat4::from_scale_rotation_translation(nodes[node_id].scale, nodes[node_id].rotation, nodes[node_id].translation));

                    // println!("\tT {}", nodes[node_id].translation);
                    // println!("\tR {}", nodes[node_id].rotation);
                    // println!("\tS {}", nodes[node_id].scale);
                    // nodes[node_id].update_matrix();

                    joint_matrices[i] = inverse_transform
                        * nodes[node_id].combined_matrix
                        * inverse_bind_matrices[i];
                });
        }

        nodes[current_index].changed = false;

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
