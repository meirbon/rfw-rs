use crate::bvh::*;
use crate::utils::Timer;
use crate::objects::*;
use crate::utils::*;

use glam::*;
use rayon::prelude::*;
use std::sync::Arc;
use std::collections::HashSet;


enum SceneFlags {
    Dirty = 0,
}

impl Into<u8> for SceneFlags {
    fn into(self) -> u8 {
        self as u8
    }
}

struct NullObject {
    dummy: f32
}

impl Intersect for NullObject {
    fn occludes(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> bool {
        false
    }

    fn intersect(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<HitRecord> {
        None
    }

    fn intersect_t(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<f32> {
        None
    }

    fn bounds(&self) -> AABB {
        AABB {
            min: Vec3::zero(),
            left_first: 0,
            max: Vec3::zero(),
            count: 0,
        }
    }
}

pub struct Scene {
    objects: Vec<Arc<Box<dyn Intersect>>>,
    object_references: Vec<HashSet<usize>>,
    instances: Vec<Instance>,
    instance_references: Vec<usize>,
    aabbs: Vec<AABB>,
    bvh: Option<BVH>,
    flags: Flags,
    null_object: Arc<Box<dyn Intersect>>,
    empty_object_slots: Vec<usize>,
    empty_instance_slots: Vec<usize>,
}

#[allow(dead_code)]
impl Scene {
    pub fn new() -> Scene {
        Scene {
            objects: Vec::new(),
            object_references: Vec::new(),
            instances: Vec::new(),
            instance_references: Vec::new(),
            bvh: None,
            aabbs: Vec::new(),
            flags: Flags::new(),
            null_object: Arc::new(Box::new(NullObject { dummy: 0.0 })),
            empty_object_slots: Vec::new(),
            empty_instance_slots: Vec::new(),
        }
    }

    pub fn add_object(&mut self, object: Box<dyn Intersect>) -> usize {
        if !self.empty_object_slots.is_empty() {
            let new_index = self.empty_object_slots.pop().unwrap();
            self.objects[new_index] = Arc::new(object);
            self.object_references[new_index] = HashSet::new();
            return new_index;
        }

        self.objects.push(Arc::new(object));
        self.object_references.push(HashSet::new());
        self.objects.len() - 1
    }

    pub fn set_object(&mut self, index: usize, object: Box<dyn Intersect>) -> Result<(), ()> {
        if self.objects.get(index).is_none() {
            return Err(());
        }

        self.objects[index] = Arc::new(object);
        let object_refs = self.object_references[index].clone();
        for i in object_refs { self.remove_instance(i).unwrap(); }

        self.object_references[index].clear();

        Ok(())
    }

    pub fn remove_object(&mut self, object: usize) -> Result<(), ()> {
        if self.objects.get(object).is_none() {
            return Err(());
        }

        self.objects[object] = self.null_object.clone();
        let object_refs = self.object_references[object].clone();
        for i in object_refs {
            self.remove_instance(i).unwrap();
        }

        self.object_references[object].clear();
        self.empty_object_slots.push(object);
        Ok(())
    }

    pub fn add_instance(&mut self, index: usize, transform: Mat4) -> Result<usize, ()> {
        if self.objects.get(index).is_none() || self.object_references.get(index).is_none() {
            return Err(());
        }

        if !self.empty_instance_slots.is_empty() {
            let new_index = self.empty_instance_slots.pop().unwrap();
            self.instances[new_index] = Instance::new(self.objects[index].clone(), transform);
            self.instance_references[new_index] = index;
            return Ok(new_index);
        }

        self.instances.push(Instance::new(self.objects[index].clone(), transform));
        self.instance_references.push(index);

        let instance_index = self.instances.len() - 1;
        self.object_references[index].insert(instance_index);
        self.flags.set_flag(SceneFlags::Dirty);

        Ok(instance_index)
    }

    pub fn set_instance_object(&mut self, instance: usize, obj_index: usize) -> Result<(), ()> {
        if self.objects.get(obj_index).is_none() || self.instances.get(instance).is_none() {
            return Err(());
        }

        let old_obj_index = self.instance_references[instance];
        self.object_references[old_obj_index].remove(&instance);
        self.instances[instance] = Instance::new(self.objects[obj_index].clone(), self.instances[instance].get_transform());
        self.object_references[obj_index].insert(instance);
        self.instance_references[instance] = obj_index;
        Ok(())
    }

    pub fn remove_instance(&mut self, index: usize) -> Result<(), ()> {
        if self.instances.get(index).is_none() {
            return Err(());
        }

        let old_obj_index = self.instance_references[index];
        if self.object_references.get(old_obj_index).is_some() {
            self.object_references[old_obj_index].remove(&index);
        }

        self.instances[index] = Instance::new(self.null_object.clone(), self.instances[index].get_transform());
        self.instance_references[index] = std::usize::MAX;
        self.empty_instance_slots.push(index);

        Ok(())
    }

    pub fn intersect(&self, origin: Vec3, direction: Vec3) -> Option<HitRecord> {
        let mut hit_record = None;
        if let Some(bvh) = &self.bvh {
            let intersection = |i, t_min, t_max| -> Option<(f32, HitRecord)> {
                if let Some(hit) = self.instances[i as usize].intersect(origin, direction, t_min, t_max) {
                    Some((hit.t, hit))
                } else {
                    None
                }
            };

            hit_record = BVHNode::traverse_stack(bvh.nodes.as_slice(), bvh.prim_indices.as_slice(),
                                                 origin, direction, 1e-5,
                                                 crate::constants::DEFAULT_T_MAX,
                                                 intersection);
        } else {
            panic!("Invalid bvh, bvh was None.");
        }

        hit_record
    }

    pub fn depth_test(&self, origin: Vec3, direction: Vec3) -> u32 {
        let mut depth = 0;
        if let Some(bvh) = &self.bvh {
            depth += BVHNode::depth_test(bvh.nodes.as_slice(), bvh.prim_indices.as_slice(), origin, direction, 1e-5,
                                         |i, t_min, t_max| { self.instances[i].intersect_t(origin, direction, t_min, t_max) },
            );
        }
        depth
    }

    pub fn build_bvh(&mut self) {
        if self.flags.has_flag(SceneFlags::Dirty) || self.bvh.is_none() { // Need to rebuild bvh
            self.bvh = Some(BVH::construct(self.instances.as_slice()));
        }
    }
}

