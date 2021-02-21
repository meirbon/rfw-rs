use futures::{future::BoxFuture, FutureExt};
use rfw_ecs::ResourceList;
use rfw_math::*;
use rfw_scene::{InstanceList2D, InstanceList3D};
use rfw_utils::task::TaskPool;
use smallvec::{smallvec, SmallVec};
use std::collections::HashSet;
use std::{
    cell::UnsafeCell,
    collections::HashMap,
    ops::{Deref, DerefMut},
    sync::Arc,
};

mod node;

pub use node::Node;

#[derive(Debug)]
#[repr(transparent)]
struct UnsafeNode(UnsafeCell<Node>);

impl UnsafeNode {
    pub fn new(node: Node) -> Arc<Self> {
        Arc::new(Self(UnsafeCell::new(node)))
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
        unsafe { self.node.0.get().as_ref().unwrap() }
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
            Some(unsafe { node.0.get().as_ref().unwrap() })
        } else {
            None
        }
    }

    pub fn get_node_mut(&mut self, id: u32) -> Option<&mut Node> {
        if let Some(node) = self.nodes.get_mut(&id) {
            Some(unsafe { node.0.get().as_mut().unwrap() })
        } else {
            None
        }
    }

    fn allocate_index(&mut self) -> u32 {
        if let Some(id) = self.empty_slots.pop() {
            id
        } else {
            let id = self.pointer;
            self.pointer += 1;
            id
        }
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
            unsafe { node.0.get().as_mut().unwrap() }.add_child(id);
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
        for child in unsafe { node.0.get().as_ref().unwrap() }.children.iter() {
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

        for child in unsafe { node.0.get().as_ref().unwrap() }.children.iter() {
            self.remove_node_internal(*child);
        }
        self.empty_slots.push(id);

        let parent = self
            .nodes
            .get_mut(&unsafe { node.0.get().as_ref().unwrap() }.parent)
            .unwrap();
        unsafe { parent.0.get().as_mut().unwrap() }.remove_child(id);
        true
    }

    pub fn merge_scene(&mut self, mut other: Graph) -> (u32, HashMap<u32, NodeHandle>) {
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
}
