use crate::InstanceList3D;
use futures::{future::BoxFuture, FutureExt};
use l3d::load::{AnimationDescriptor, NodeDescriptor};
use rfw_backend::{MeshId3D, SkinData};
use rfw_math::*;
use rfw_utils::collections::{FlaggedStorage, TrackedStorage};
use smallvec::{smallvec, SmallVec};
use std::{
    cell::UnsafeCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};
use std::{collections::HashSet, ops::Index};

mod node;
pub use node::*;

#[derive(Debug)]
#[repr(transparent)]
pub struct UnsafeNode(UnsafeCell<Node>);

impl UnsafeNode {
    pub fn new(node: Node) -> Arc<Self> {
        Arc::new(Self(UnsafeCell::new(node)))
    }

    pub fn get(&self) -> &Node {
        unsafe { self.0.get().as_ref().unwrap() }
    }

    pub fn get_mut(&self) -> &mut Node {
        unsafe { self.0.get().as_mut().unwrap() }
    }
}

unsafe impl Send for UnsafeNode {}
unsafe impl Sync for UnsafeNode {}

#[derive(Debug)]
pub struct NodeHandle {
    id: u32,
    node: Arc<UnsafeNode>,
}

impl NodeHandle {
    pub fn get_id(&self) -> u32 {
        self.id
    }
}

impl Deref for NodeHandle {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        self.node.get()
    }
}

impl DerefMut for NodeHandle {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.node.0.get().as_mut().unwrap() }
    }
}

#[derive(Debug)]
pub struct Graph {
    nodes: HashMap<u32, Arc<UnsafeNode>>,
    pointer: u32,
    empty_slots: Vec<u32>,
    animations:
}

impl Default for Graph {
    fn default() -> Self {
        let mut nodes = HashMap::new();
        let empty_slots = Vec::new();

        nodes.insert(0, UnsafeNode::new(Node::default()));

        Self {
            nodes,
            pointer: 1,
            empty_slots,
        }
    }
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every node can only be referenced once, thus we can safely get mutable references to each node for faster (parallel) updates
    pub async fn update(&mut self) {
        Self::update_node(0, Mat4::identity(), &self.nodes).await;
    }

    fn update_node(
        current: u32,
        matrix: Mat4,
        nodes: &HashMap<u32, Arc<UnsafeNode>>,
    ) -> BoxFuture<'_, ()> {
        async move {
            let node = unsafe { nodes.get(&current).unwrap().0.get().as_mut().unwrap() };
            node.update(matrix);

            let matrix = node.matrix * matrix;
            let mut handles: SmallVec<[Box<_>; 16]> = smallvec![];

            for child in node.children.iter() {
                handles.push(Box::new(Self::update_node(*child, matrix, nodes)));
            }

            for handle in handles.into_iter() {
                handle.await;
            }
        }
        .boxed()
    }

    pub fn get_node(&self, id: u32) -> Option<&Node> {
        if let Some(node) = self.nodes.get(&id) {
            Some(node.get())
        } else {
            None
        }
    }

    pub fn get_node_mut(&mut self, id: u32) -> Option<&mut Node> {
        if let Some(node) = self.nodes.get_mut(&id) {
            Some(node.get_mut())
        } else {
            None
        }
    }

    fn allocate_index(&mut self) -> u32 {
        let id = if let Some(id) = self.empty_slots.pop() {
            id
        } else {
            let id = self.pointer;
            self.pointer += 1;
            id
        };

        debug_assert_ne!(id, 0);
        id
    }

    /// Adds a node to the graph.
    ///
    /// If parent == None, the node will become a child of the root of this graph.
    /// This function will return None if the provided parent does not exist.
    pub fn add_node(&mut self, parent: Option<u32>, node: Node) -> Option<NodeHandle> {
        let id = self.allocate_index();

        // Either use a specified parent or the root of this scene
        let parent = parent.unwrap_or(0);

        if let Some(node) = self.nodes.get_mut(&parent) {
            node.get_mut().add_child(id);
        } else {
            return None;
        }

        let node = UnsafeNode::new(node.with_parent(parent));
        self.nodes.insert(id, node.clone());
        let handle = NodeHandle { id, node };

        Some(handle)
    }

    fn remove_node_internal(&mut self, id: u32) {
        let (id, node) = self.nodes.remove_entry(&id).unwrap();
        for child in node.get().children.iter() {
            self.remove_node_internal(*child);
        }
        self.empty_slots.push(id);
    }

    /// # Safety
    ///
    /// Removing a node invalidates all NodeHandles of the node's children.
    /// Using handles makes updates much faster at the cost of memory safety.
    pub unsafe fn remove_node(&mut self, node: NodeHandle) -> bool {
        let (id, node) = if let Some(node) = self.nodes.remove_entry(&node.id) {
            node
        } else {
            return false;
        };

        for child in node.get().children.iter() {
            self.remove_node_internal(*child);
        }
        self.empty_slots.push(id);

        let parent_id = node.get().parent;
        let parent: &UnsafeNode = self.nodes.get_mut(&parent_id).unwrap();

        parent.get_mut().remove_child(id);
        true
    }

    pub fn merge_scene(&mut self, other: Graph) -> (u32, HashMap<u32, NodeHandle>) {
        let mut mapping = HashMap::new();
        let mut handles = HashMap::new();

        for (k, v) in other.nodes.into_iter() {
            let id = self.allocate_index();
            // Add the new id to the mapping
            mapping.insert(k, id);

            // Add node to storage
            self.nodes.insert(id, v.clone());

            // Store a handle
            handles.insert(id, NodeHandle { id, node: v });
        }

        let mut stack: SmallVec<[u32; 32]> = smallvec![0];
        while !stack.is_empty() {
            let original_id = stack.pop().unwrap();
            let new_id = *mapping.get(&original_id).unwrap();

            let handle = handles.get_mut(&new_id).unwrap();
            let mut new_set = HashSet::with_capacity(handle.children.len());
            for child in handle.children.iter() {
                let new_id = *mapping.get(child).unwrap();

                // Add the new id to the new set of children
                new_set.insert(new_id);

                // Push the original child id to the stack
                stack.push(*child);
            }

            // Replace set of children with updated ids
            handle.children = new_set;
        }

        (*mapping.get(&0).unwrap(), handles)
    }

    pub fn load_descriptor(
        &mut self,
        descriptor: &GraphDescriptor,
        instances: &mut FlaggedStorage<InstanceList3D>,
        _skins: &mut TrackedStorage<Skin>,
    ) -> NodeHandle {
        let mut root_node = Node::default();
        let root_id = self.allocate_index();

        let mut mapping = HashMap::new();
        for node in descriptor.nodes.iter() {
            Self::traverse_desc(node, &mut |_parent, desc| {
                mapping.insert(desc.id, self.allocate_index());
                0
            });
        }

        for node in descriptor.nodes.iter() {
            let root_id = Self::traverse_desc(node, &mut |parent, node| {
                let mut new_node = Node::default().with_parent(*mapping.get(&parent).unwrap());
                for desc in node.child_nodes.iter() {
                    new_node.add_child(*mapping.get(&desc.id).unwrap());
                }

                new_node.set_matrix(Mat4::from_scale_rotation_translation(
                    Vec3::from(node.scale),
                    Quat::from(node.rotation),
                    Vec3::from(node.translation),
                ));

                for mesh in node.meshes.iter() {
                    if let Some(mesh) = instances.get_mut(*mesh as usize) {
                        new_node.add_instance(NodeChild::Instance3D(mesh.allocate()));
                    }
                }

                let id = *mapping.get(&node.id).unwrap();
                self.nodes.insert(id, UnsafeNode::new(new_node));
                id
            });

            root_node.add_child(root_id);
        }

        let handle = UnsafeNode::new(root_node);
        self.nodes.insert(root_id, handle.clone());

        unsafe { self.nodes.get(&0).unwrap().0.get().as_mut().unwrap() }.add_child(root_id);

        NodeHandle {
            id: root_id,
            node: handle,
        }
    }

    fn traverse_desc<Cb: FnMut(u32, &NodeDescriptor) -> u32>(
        desc: &NodeDescriptor,
        cb: &mut Cb,
    ) -> u32 {
        let id = cb(0, desc);
        for child in desc.child_nodes.iter() {
            Self::traverse_desc(child, cb);
        }

        id
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
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

impl<'a> From<&'a Skin> for SkinData<'a> {
    fn from(skin: &'a Skin) -> Self {
        Self {
            name: skin.name.as_str(),
            inverse_bind_matrices: skin.inverse_bind_matrices.as_slice(),
            joint_matrices: skin.joint_matrices.as_slice(),
        }
    }
}

impl<'a> From<&'a mut Skin> for SkinData<'a> {
    fn from(skin: &'a mut Skin) -> Self {
        Self {
            name: skin.name.as_str(),
            inverse_bind_matrices: skin.inverse_bind_matrices.as_slice(),
            joint_matrices: skin.joint_matrices.as_slice(),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct GraphDescriptor {
    pub meshes: Vec<MeshId3D>,
    pub nodes: Vec<NodeDescriptor>,
    pub animations: Vec<AnimationDescriptor>,
}

impl From<MeshId3D> for GraphDescriptor {
    fn from(m: MeshId3D) -> Self {
        Self {
            meshes: vec![m],
            nodes: vec![NodeDescriptor {
                name: Default::default(),
                child_nodes: Default::default(),
                camera: None,
                translation: [0.0; 3],
                rotation: Quat::identity().into(),
                scale: [1.0; 3],
                meshes: vec![m.0 as _],
                skin: None,
                weights: Default::default(),
                id: 0,
            }],
            animations: Default::default(),
        }
    }
}

impl Index<u32> for Graph {
    type Output = UnsafeNode;

    fn index(&self, index: u32) -> &Self::Output {
        self.nodes.get(&index).unwrap()
    }
}
