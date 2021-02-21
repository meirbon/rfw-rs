use rfw_math::*;
use rfw_scene::{obj, Camera3D, InstanceHandle2D, InstanceHandle3D};
use std::collections::HashSet;

#[derive(Debug)]
pub struct Node {
    pub(crate) parent: u32,
    pub(crate) matrix: Mat4,
    pub(crate) children: HashSet<u32>,
    pub(crate) objects: Vec<NodeChild>,
}

pub enum NodeChild {
    Instance2D(InstanceHandle2D),
    Instance3D(InstanceHandle3D),
    Custom(Box<dyn HasMatrix>),
}

impl Into<NodeChild> for InstanceHandle2D {
    fn into(self) -> NodeChild {
        NodeChild::Instance2D(self)
    }
}

impl Into<NodeChild> for InstanceHandle3D {
    fn into(self) -> NodeChild {
        NodeChild::Instance3D(self)
    }
}

impl Default for Node {
    fn default() -> Self {
        Node {
            parent: 0,
            matrix: Default::default(),
            children: HashSet::with_capacity(16),
            objects: Vec::with_capacity(16),
        }
    }
}

unsafe impl Send for Node {}
unsafe impl Sync for Node {}

impl Node {
    pub(crate) fn with_parent(mut self, id: u32) -> Self {
        self.parent = id;
        self
    }

    pub(crate) fn update(&mut self, matrix: Mat4) {
        let matrix = self.matrix * matrix;
        for object in &mut self.objects {
            object.set_matrix(matrix);
        }
    }

    pub(crate) fn add_child(&mut self, child: u32) {
        assert!(self.children.insert(child));
    }

    pub(crate) fn remove_child(&mut self, child: u32) {
        assert!(self.children.remove(&child));
    }

    pub fn get_children(&self) -> &HashSet<u32> {
        &self.children
    }

    pub fn get_objects(&self) -> &[NodeChild] {
        self.objects.as_slice()
    }

    pub fn get_objects_mut(&self) -> &mut [NodeChild] {
        self.objects.as_slice_mut()
    }

    pub fn add_instance<I: Into<NodeChild>>(&mut self, instance: I) {
        self.objects.push(instance.into());
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        for object in self.objects.into_iter() {
            match object {
                NodeChild::Instance2D(i) => i.make_invalid(),
                NodeChild::Instance3D(i) => i.make_invalid(),
                NodeChild::Custom(_) => {}
            }
        }
    }
}

impl HasTranslation for Node {
    fn get_translation(&self) -> Vec3 {
        let (_, _, t) = self.matrix.to_scale_rotation_translation();
        t
    }
}

impl HasRotation for Node {
    fn get_rotation(&self) -> Quat {
        let (_, r, _) = self.matrix.to_scale_rotation_translation();
        r
    }
}

impl HasScale for Node {
    fn get_scale(&self) -> Vec3 {
        let (s, _, _) = self.matrix.to_scale_rotation_translation();
        s
    }
}

impl HasMatrix for Node {
    fn update(&mut self, t: Vec3, r: Quat, s: Vec3) {
        self.matrix = Mat4::from_scale_rotation_translation(s, r, t);
    }

    fn set_matrix(&mut self, matrix: Mat4) {
        self.matrix = matrix;
    }
}
