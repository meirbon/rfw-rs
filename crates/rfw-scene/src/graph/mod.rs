use crate::{
    utils::{HasMatrix, HasRotation, HasScale, HasTranslation, Transform},
    InstanceList3D, LoadResult, Mesh3D, SkinID, ToMesh3D,
};
use l3d::load::{Animation, AnimationDescriptor, AnimationNode, SkinDescriptor};
use rayon::prelude::*;
use rfw_backend::*;
use rfw_math::*;

use rfw_utils::collections::{FlaggedStorage, TrackedStorage};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
pub enum NodeFlags {
    First = 0,
    Transformed = 1,
    Morphed = 2,
}

impl From<NodeFlags> for u8 {
    fn from(f: NodeFlags) -> Self {
        f as u8
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct NodeMesh {
    pub object_id: u32,
    pub instance_id: Option<u32>,
}

impl std::fmt::Display for NodeMesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NodeMesh {{ object_id: {}, instance_id: {} }}",
            self.object_id,
            match self.instance_id {
                None => String::from("None"),
                Some(id) => format!("{}", id),
            }
        )
    }
}

#[derive(Debug, Clone)]
pub struct NodeDescriptor {
    pub name: String,
    pub child_nodes: Vec<Self>,

    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,

    pub meshes: Vec<u32>,
    pub skin: Option<SkinDescriptor>,
    pub weights: Vec<f32>,

    /// An ID that is guaranteed to be unique within the scene descriptor this
    /// node descriptor belongs to.
    pub id: u32,
}

impl Default for NodeDescriptor {
    fn default() -> Self {
        Self {
            name: Default::default(),
            child_nodes: Default::default(),

            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,

            meshes: Default::default(),
            skin: None,
            weights: Default::default(),

            id: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SceneDescriptor {
    pub meshes: Vec<MeshId3D>,
    pub nodes: Vec<NodeDescriptor>,
    pub animations: Vec<AnimationDescriptor>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Node {
    pub(crate) translation: Vec3,
    pub(crate) rotation: Quat,
    pub(crate) scale: Vec3,
    pub(crate) local_matrix: Mat4,
    pub combined_matrix: Mat4,
    pub skin: Option<u32>,
    pub weights: Vec<f32>,
    pub meshes: Vec<NodeMesh>,
    pub child_nodes: Vec<u32>,
    pub name: String,
    pub changed: bool,
    pub first: bool,
    pub morphed: bool,
}

#[derive(Debug)]
pub struct GraphHandle {
    id: usize,
    data: Arc<Mutex<NodeGraph>>,
}

impl GraphHandle {
    pub fn get_id(&self) -> usize {
        self.id
    }

    pub fn get_transform(&mut self) -> Transform<Self> {
        let data = self.data.lock().unwrap();
        let root_node = data.root_node as usize;
        let translation = data.nodes[root_node].translation;
        let rotation = data.nodes[root_node].rotation;
        let scale = data.nodes[root_node].scale;
        drop(data);

        Transform {
            translation,
            rotation,
            scale,
            handle: self,
            changed: false,
        }
    }

    /// # Safety
    ///
    /// Although in this case cloning is safe, the graph could be deleted and thus not reference any existing data.
    /// Having multiple handles could work but management of whether the graph actually still exists is up to the user
    /// once they start cloning handles.
    pub unsafe fn clone_handle(&self) -> Self {
        Self {
            id: self.id,
            data: self.data.clone(),
        }
    }
}

impl HasTranslation for GraphHandle {}
impl HasRotation for GraphHandle {}
impl HasScale for GraphHandle {}

impl HasMatrix for GraphHandle {
    fn update(&mut self, t: Vec3, r: Quat, s: Vec3) {
        if let Ok(mut graph) = self.data.lock() {
            let root_node = graph.root_node as usize;
            graph.nodes[root_node].scale = s;
            graph.nodes[root_node].rotation = r;
            graph.nodes[root_node].translation = t;
            graph.nodes[root_node].changed = true;
        }
    }
}

impl AnimationNode for Node {
    fn set_translation(&mut self, translation: [f32; 3]) {
        self.translation = Vec3::from(translation);
        self.changed = true;
    }

    fn set_rotation(&mut self, rotation: [f32; 4]) {
        self.rotation = Quat::from_array(rotation);
        self.changed = true;
    }

    fn set_scale(&mut self, scale: [f32; 3]) {
        self.scale = Vec3::from(scale);
        self.changed = true;
    }

    fn set_weights(&mut self, weights: &[f32]) {
        let num_weights = self.weights.len().min(weights.len());
        self.weights[0..num_weights].copy_from_slice(&weights[0..num_weights]);
        self.morphed = true;
    }

    fn update_matrix(&mut self) {
        let trs =
            Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation);
        self.local_matrix = trs;
        self.changed = false;
    }
}

impl Default for Node {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(1.0),
            local_matrix: Mat4::IDENTITY,
            combined_matrix: Mat4::IDENTITY,
            skin: None,
            weights: Vec::new(),
            meshes: Vec::new(),
            child_nodes: Vec::new(),
            changed: true,
            morphed: false,
            first: true,
            name: String::new(),
        }
    }
}

impl Node {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_translation<T: Into<[f32; 3]>>(&mut self, t: T) {
        self.translation = Vec3::from(t.into());
        self.changed = true;
    }

    /// Set rotation using an xyzw quaternion
    pub fn set_rotation<T: Into<[f32; 4]>>(&mut self, r: T) {
        self.rotation = Quat::from_array(r.into());
        self.changed = true;
    }

    pub fn set_scale<T: Into<[f32; 3]>>(&mut self, s: T) {
        self.scale = Vec3::from(s.into());
        self.changed = true;
    }

    pub fn set_matrix<T: Into<[f32; 16]>>(&mut self, matrix: T) {
        let matrix = Mat4::from_cols_array(&matrix.into());
        let (s, r, t) = matrix.to_scale_rotation_translation();
        self.scale = s;
        self.rotation = r;
        self.translation = t;
        self.changed = true;
    }

    pub fn set_matrix_cols<T: Into<[[f32; 4]; 4]>>(&mut self, matrix: T) {
        let matrix = Mat4::from_cols_array_2d(&matrix.into());
        let (s, r, t) = matrix.to_scale_rotation_translation();
        self.scale = s;
        self.rotation = r;
        self.translation = t;
        self.changed = true;
    }

    pub fn scale_x(&mut self, scale: f32) {
        self.scale *= Vec3::new(scale, 1.0, 1.0);
        self.changed = true;
    }

    pub fn scale_y(&mut self, scale: f32) {
        self.scale *= Vec3::new(1.0, scale, 1.0);
        self.changed = true;
    }

    pub fn scale_z(&mut self, scale: f32) {
        self.scale *= Vec3::new(1.0, 1.0, scale);
        self.changed = true;
    }

    pub fn scale<T: Into<[f32; 3]>>(&mut self, offset: T) {
        self.scale *= Vec3::from(offset.into());
        self.changed = true;
    }

    pub fn translate_x(&mut self, offset: f32) {
        self.translation += Vec3::new(offset, 0.0, 0.0);
        self.changed = true;
    }

    pub fn translate_y(&mut self, offset: f32) {
        self.translation += Vec3::new(0.0, offset, 0.0);
        self.changed = true;
    }

    pub fn translate_z(&mut self, offset: f32) {
        self.translation += Vec3::new(0.0, 0.0, offset);
        self.changed = true;
    }

    pub fn translate<T: Into<[f32; 3]>>(&mut self, offset: T) {
        let offset: [f32; 3] = offset.into();
        self.translation += Vec3::from(offset);
        self.changed = true;
    }

    pub fn rotate<T: Into<[f32; 3]>>(&mut self, degrees: T) {
        let degrees: [f32; 3] = degrees.into();
        self.rotation *= Quat::from_rotation_x(degrees[0].to_radians());
        self.rotation *= Quat::from_rotation_y(degrees[1].to_radians());
        self.rotation *= Quat::from_rotation_z(degrees[2].to_radians());
        self.changed = true;
    }

    pub fn rotate_x(&mut self, degrees: f32) {
        self.rotation *= Quat::from_rotation_x(degrees.to_radians());
        self.changed = true;
    }

    pub fn rotate_y(&mut self, degrees: f32) {
        self.rotation *= Quat::from_rotation_y(degrees.to_radians());
        self.changed = true;
    }

    pub fn rotate_z(&mut self, degrees: f32) {
        self.rotation *= Quat::from_rotation_z(degrees.to_radians());
        self.changed = true;
    }
}

pub trait ToScene {
    fn to_scene(
        &self,
        meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
        skins: &mut TrackedStorage<Skin>,
    ) -> NodeGraph;
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct NodeGraph {
    nodes: TrackedStorage<Node>,
    root_node: u32,
    pub animations: TrackedStorage<Animation>,
    pub skins: TrackedStorage<Skin>,
    pub active_animation: Option<usize>,
}

impl ToScene for LoadResult {
    fn to_scene(
        &self,
        meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
        skins: &mut TrackedStorage<Skin>,
    ) -> NodeGraph {
        match self {
            LoadResult::Object(mesh) => mesh.to_scene(meshes, instances, skins),
            LoadResult::Scene(scene) => scene.to_scene(meshes, instances, skins),
        }
    }
}

impl<T: ToMesh3D + Clone> ToScene for T {
    fn to_scene(
        &self,
        meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
        skins: &mut TrackedStorage<Skin>,
    ) -> NodeGraph {
        let mesh = self.clone().into_mesh_3d();
        let id = MeshId3D(meshes.push(mesh) as _);
        id.to_scene(meshes, instances, skins)
    }
}

impl ToScene for MeshId3D {
    fn to_scene(
        &self,
        _meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
        _skins: &mut TrackedStorage<Skin>,
    ) -> NodeGraph {
        let mut graph = NodeGraph::new();
        graph.nodes[graph.root_node as usize].meshes.push(NodeMesh {
            object_id: self.0 as _,
            instance_id: Some(instances[self.0 as usize].allocate().get_id() as _),
        });
        graph
    }
}

impl ToScene for SceneDescriptor {
    fn to_scene(
        &self,
        meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
        skins: &mut TrackedStorage<Skin>,
    ) -> NodeGraph {
        let mut graph = NodeGraph::new();
        graph.load_scene_descriptor(self, meshes, instances);
        graph.initialize(meshes, instances, skins);
        graph
    }
}

impl Default for NodeGraph {
    fn default() -> Self {
        let mut nodes = TrackedStorage::new();
        nodes.push(Node::default());

        Self {
            nodes,
            root_node: 0,
            animations: TrackedStorage::new(),
            skins: TrackedStorage::new(),
            active_animation: None,
        }
    }
}

impl NodeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn initialize(
        &mut self,
        _meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
        skins: &mut TrackedStorage<Skin>,
    ) {
        for (_, node) in self.nodes.iter_mut() {
            if let Some(skin_id) = node.skin {
                let skin_id = skins.push(self.skins[skin_id as usize].clone());
                node.skin = Some(skin_id as u32);
            }

            for mesh in &mut node.meshes {
                if mesh.instance_id.is_none() {
                    let mut instance = instances[mesh.object_id as usize].allocate();
                    mesh.instance_id = Some(instance.get_id() as u32);
                    instance.set_skin(SkinID(if let Some(s) = node.skin {
                        s as i32
                    } else {
                        -1
                    }));
                }
            }
        }
    }

    pub fn add_animation(&mut self, anim: Animation) -> usize {
        self.animations.push(anim)
    }

    pub fn add_skin(&mut self, skin: Skin) -> usize {
        self.skins.push(skin)
    }

    pub fn allocate(&mut self) -> usize {
        self.nodes.allocate()
    }

    pub fn any_changed(&self) -> bool {
        self.nodes.any_changed()
    }

    pub fn reset_changed(&mut self) {
        self.nodes.reset_changed();
    }

    pub fn trigger_changed(&mut self, id: usize) {
        self.nodes.trigger_changed(id);
    }

    pub fn trigger_changed_all(&mut self) {
        self.nodes.trigger_changed_all();
    }

    pub fn update(
        &mut self,
        meshes: &RwLock<&mut TrackedStorage<Mesh3D>>,
        instances: &RwLock<&mut FlaggedStorage<InstanceList3D>>,
        skins: &RwLock<&mut TrackedStorage<Skin>>,
    ) -> bool {
        if !self.nodes.any_changed() {
            return false;
        }

        let mut changed = false;
        let id = self.root_node as usize;

        changed |= Self::traverse_children(
            id,
            Mat4::IDENTITY,
            &mut self.nodes,
            meshes,
            instances,
            skins,
        );

        for i in self.nodes[id].child_nodes.iter() {
            if self.nodes.get_changed(*i as usize) {
                self.nodes.trigger_changed(id);
                break;
            }
        }

        self.nodes.reset_changed();
        changed
    }

    pub fn get(&self, index: usize) -> Option<&Node> {
        self.nodes.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Node> {
        self.nodes.get_mut(index)
    }

    /// # Safety
    ///
    /// Returns reference to node without index checking.
    pub unsafe fn get_unchecked(&self, index: usize) -> &Node {
        self.nodes.get_unchecked(index)
    }

    /// # Safety
    ///
    /// Returns mutable reference to node without index checking.
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> &mut Node {
        self.nodes.get_unchecked_mut(index)
    }

    pub fn as_ptr(&self) -> *const Node {
        self.nodes.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut Node {
        self.nodes.as_mut_ptr()
    }

    fn traverse_children(
        current_index: usize,
        accumulated_matrix: Mat4,
        nodes: &mut TrackedStorage<Node>,
        meshes: &RwLock<&mut TrackedStorage<Mesh3D>>,
        instances: &RwLock<&mut FlaggedStorage<InstanceList3D>>,
        skins: &RwLock<&mut TrackedStorage<Skin>>,
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
            changed |=
                Self::traverse_children(c_id, combined_matrix, nodes, meshes, instances, skins);
        }

        if !changed && !nodes[current_index].first {
            return false;
        }

        let node_meshes = &nodes[current_index].meshes;
        node_meshes
            .iter()
            .filter(|m| m.instance_id.is_some())
            .for_each(|m| {
                let instances = instances.read().unwrap();
                if let Some(instances) = instances.get(m.object_id as usize) {
                    if let Some(mut instance) = instances.get(m.instance_id.unwrap() as usize) {
                        instance.set_matrix(combined_matrix);
                    }
                }

                // TODO: Morph animations
            });

        // Update skin
        if let Some(skin) = nodes[current_index].skin {
            if let Ok(mut skins) = skins.write() {
                let skin = &mut skins[skin as usize];
                let inverse_transform = combined_matrix.inverse();
                let inverse_bind_matrices = &skin.inverse_bind_matrices;
                let joint_matrices = &mut skin.joint_matrices;

                skin.joint_nodes
                    .iter()
                    .enumerate()
                    .for_each(|(i, node_id)| {
                        let node_id = *node_id as usize;
                        joint_matrices[i] = inverse_transform
                            * nodes[node_id].combined_matrix
                            * inverse_bind_matrices[i];
                    });
            }

            node_meshes
                .iter()
                .filter(|m| m.instance_id.is_some())
                .for_each(|m| {
                    if let Some(skin) = nodes[current_index].skin {
                        let instances = instances.read().unwrap();
                        if let Some(instances) = instances.get(m.object_id as usize) {
                            if let Some(mut instance) =
                                instances.get(m.instance_id.unwrap() as usize)
                            {
                                instance.set_skin(SkinID(skin as i32));
                            }
                        }
                    }
                });
        }

        nodes[current_index].changed = false;

        // Return whether this node or its children changed
        changed
    }

    pub fn root_node(&self) -> usize {
        self.root_node as usize
    }

    pub fn update_animation(&mut self, time: f32) {
        if let Some(animation) = self.active_animation {
            self.animations[animation].set_time(time, self.nodes.as_mut_slice());
            self.nodes.trigger_changed(self.root_node as usize); // Trigger a change
        }
    }

    pub fn set_active_animation(&mut self, id: usize) {
        if self.animations.get(id).is_some() {
            self.active_animation = Some(id);
        }
    }

    pub fn load_scene_descriptor(
        &mut self,
        scene_descriptor: &SceneDescriptor,
        meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
    ) {
        let mut node_map: HashMap<u32, u32> =
            HashMap::with_capacity(scene_descriptor.nodes.len() + 1);
        let root_id = self.load_node_descriptor(
            &mut node_map,
            &NodeDescriptor {
                name: String::new(),
                child_nodes: Vec::new(),
                translation: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                meshes: Default::default(),
                skin: None,
                weights: Default::default(),
                id: 0,
            },
            scene_descriptor,
            meshes,
            instances,
        );

        let mut root_nodes = Vec::with_capacity(scene_descriptor.nodes.len());

        for node in &scene_descriptor.nodes {
            let node_id =
                self.load_node_descriptor(&mut node_map, node, scene_descriptor, meshes, instances);

            self.nodes[self.root_node as usize]
                .child_nodes
                .push(node_id);
            root_nodes.push(node_id);
        }

        for animation in scene_descriptor.animations.iter() {
            let channels = animation
                .channels
                .iter()
                .map(|(node_desc_id, channel)| {
                    let node_id = node_map[&node_desc_id];
                    (node_id, channel.clone())
                })
                .collect();

            let animation_id = self.animations.push(Animation {
                name: animation.name.clone(),
                affected_roots: root_nodes.clone(),
                channels,
            });

            self.set_active_animation(animation_id);
            self.update_animation(0.0);
        }

        for node in root_nodes {
            self.nodes[root_id as usize].child_nodes.push(node);
        }
    }

    pub fn load_node_descriptor(
        &mut self,
        node_map: &mut HashMap<u32, u32>,
        descriptor: &NodeDescriptor,
        scene_descriptor: &SceneDescriptor,
        meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
    ) -> u32 {
        let child_nodes: Vec<u32> = descriptor
            .child_nodes
            .iter()
            .map(|child_node_descriptor| {
                self.load_node_descriptor(
                    node_map,
                    child_node_descriptor,
                    scene_descriptor,
                    meshes,
                    instances,
                )
            })
            .collect();

        let skin_id = descriptor
            .skin
            .as_ref()
            .map(|s| {
                let joint_nodes = s
                    .joint_nodes
                    .iter()
                    .map(|joint_node_id| node_map[joint_node_id])
                    .collect();

                self.skins.push(Skin {
                    name: s.name.clone(),
                    joint_nodes,
                    inverse_bind_matrices: s
                        .inverse_bind_matrices
                        .iter()
                        .map(|m| Mat4::from_cols_array(m))
                        .collect(),
                    joint_matrices: vec![Mat4::IDENTITY; s.inverse_bind_matrices.len()],
                })
            })
            .map(|id| id as u32);

        let meshes: Vec<NodeMesh> = descriptor
            .meshes
            .iter()
            .map(|mesh| {
                let mut instance = instances.get_mut(*mesh as usize).unwrap().allocate();
                instance.set_skin(SkinID(match skin_id {
                    None => -1,
                    Some(id) => id as i32,
                }));

                NodeMesh {
                    object_id: *mesh,
                    instance_id: Some(instance.get_id() as u32),
                }
            })
            .collect();

        let mut node = Node {
            translation: descriptor.translation,
            rotation: descriptor.rotation,
            scale: descriptor.scale,
            local_matrix: Mat4::IDENTITY,
            combined_matrix: Mat4::IDENTITY,
            skin: skin_id,
            weights: descriptor.weights.clone(),
            meshes,
            child_nodes,
            changed: true,
            morphed: false,
            first: true,
            name: descriptor.name.clone(),
        };
        node.update_matrix();
        let node_id = self.nodes.push(node) as u32;

        node_map.insert(descriptor.id, node_id);

        node_id
    }

    pub fn from_scene_descriptor(
        scene_descriptor: &SceneDescriptor,
        meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
    ) -> Self {
        let mut graph = Self::new();
        graph.load_scene_descriptor(scene_descriptor, meshes, instances);
        graph
    }

    pub fn from_node_descriptor(
        node_map: &mut HashMap<u32, u32>,
        descriptor: &NodeDescriptor,
        scene_descriptor: &SceneDescriptor,
        meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
    ) -> Self {
        let mut graph = Self::new();
        graph.load_node_descriptor(node_map, descriptor, scene_descriptor, meshes, instances);
        graph
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
pub struct SceneGraph {
    sub_graphs: TrackedStorage<Arc<Mutex<NodeGraph>>>,
    times: TrackedStorage<f32>,
}

impl Default for SceneGraph {
    fn default() -> Self {
        Self {
            sub_graphs: TrackedStorage::default(),
            times: TrackedStorage::default(),
        }
    }
}

impl SceneGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn synchronize(
        &mut self,
        meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
        skins: &mut TrackedStorage<Skin>,
    ) -> bool {
        let times = &self.times;
        let (meshes, instances, skins) = (
            RwLock::new(meshes),
            RwLock::new(instances),
            RwLock::new(skins),
        );
        let changed: u32 = self
            .sub_graphs
            .iter()
            .par_bridge()
            .map(|(i, graph)| {
                if let Ok(mut graph) = graph.lock() {
                    graph.update_animation(times[i]);
                    if graph.update(&meshes, &instances, &skins) {
                        graph.reset_changed();
                        return 1;
                    }
                }

                0
            })
            .sum();

        self.times.reset_changed();
        self.sub_graphs.reset_changed();
        changed > 0
    }

    pub fn add_graph(&mut self, graph: NodeGraph) -> GraphHandle {
        let data = Arc::new(Mutex::new(graph));
        let id = self.sub_graphs.push(data.clone());
        self.times.overwrite(id, 0.0);
        GraphHandle { id, data }
    }

    pub fn get_graph(&self, id: usize) -> Option<GraphHandle> {
        self.sub_graphs.get(id).map(|graph| GraphHandle {
            id,
            data: graph.clone(),
        })
    }

    pub fn remove_graph(
        &mut self,
        handle: GraphHandle,
        _meshes: &mut TrackedStorage<Mesh3D>,
        instances: &mut FlaggedStorage<InstanceList3D>,
        skins: &mut TrackedStorage<Skin>,
    ) -> bool {
        let id = handle.get_id();

        // Remove instances part of this sub graph
        if let Some(graph) = self.sub_graphs.get(id) {
            if let Ok(graph) = graph.lock() {
                for (_, node) in graph.nodes.iter() {
                    if let Some(skin_id) = node.skin {
                        skins.erase(skin_id as usize).unwrap();
                    }

                    for mesh in &node.meshes {
                        if let Some(id) = mesh.instance_id {
                            if let Some(handle) =
                                instances[mesh.object_id as usize].get(id as usize)
                            {
                                handle.make_invalid();
                            }
                        }
                    }
                }
            }
        }

        self.sub_graphs.erase(id).is_ok() && self.times.erase(id).is_ok()
    }

    pub fn set_animation(&mut self, handle: &GraphHandle, time: f32) {
        if let Some(t) = self.times.get_mut(handle.get_id()) {
            *t = time;
        }
    }

    pub fn set_animations(&mut self, time: f32) {
        self.times.iter_mut().for_each(|(_, t)| {
            *t = time;
        });
    }
}
