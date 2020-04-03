use crate::objects::*;
use crate::utils::*;

use glam::*;
use std::collections::HashSet;
use bvh::{AABB, Bounds, BVH, MBVH};

pub static mut USE_MBVH: bool = true;

enum SceneFlags {
    Dirty = 1,
}

impl Into<u8> for SceneFlags {
    fn into(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Copy, Clone)]
struct NullObject {
    _dummy: f32
}

impl Intersect for NullObject {
    fn occludes(&self, _origin: Vec3, _direction: Vec3, _t_min: f32, _t_max: f32) -> bool {
        false
    }

    fn intersect(&self, _origin: Vec3, _direction: Vec3, _t_min: f32, _t_max: f32) -> Option<HitRecord> {
        None
    }

    fn intersect_t(&self, _origin: Vec3, _direction: Vec3, _t_min: f32, _t_max: f32) -> Option<f32> {
        None
    }

    fn depth_test(&self, _origin: Vec3, _direction: Vec3, _t_min: f32, _t_max: f32) -> Option<(f32, u32)> {
        None
    }
}

impl Bounds for NullObject {
    fn bounds(&self) -> AABB {
        AABB::new()
    }
}

pub struct Scene {
    objects: Vec<Box<dyn Intersect>>,
    object_references: Vec<HashSet<usize>>,
    instances: Vec<Instance>,
    instance_references: Vec<usize>,
    bvh: BVH,
    mbvh: MBVH,
    flags: Flags,
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
            bvh: BVH::empty(),
            mbvh: MBVH::empty(),
            flags: Flags::new(),
            empty_object_slots: Vec::new(),
            empty_instance_slots: Vec::new(),
        }
    }

    pub fn get_object<T>(&self, index: usize, mut cb: T) where T: FnMut(Option<&Box<dyn Intersect>>) {
        cb(self.objects.get(index));
    }

    pub fn get_object_mut<T>(&mut self, index: usize, mut cb: T) where T: FnMut(Option<&mut Box<dyn Intersect>>) {
        cb(self.objects.get_mut(index));
        self.flags.set_flag(SceneFlags::Dirty);
    }

    pub fn add_object(&mut self, object: Box<dyn Intersect>) -> usize {
        if !self.empty_object_slots.is_empty() {
            let new_index = self.empty_object_slots.pop().unwrap();
            self.objects[new_index] = object;
            self.object_references[new_index] = HashSet::new();
            return new_index;
        }

        self.objects.push(object);
        self.object_references.push(HashSet::new());
        self.flags.set_flag(SceneFlags::Dirty);
        self.objects.len() - 1
    }

    pub fn set_object(&mut self, index: usize, object: Box<dyn Intersect>) -> Result<(), ()> {
        if self.objects.get(index).is_none() {
            return Err(());
        }

        self.objects[index] = object;
        let object_refs = self.object_references[index].clone();
        for i in object_refs { self.remove_instance(i).unwrap(); }

        self.object_references[index].clear();
        self.flags.set_flag(SceneFlags::Dirty);
        Ok(())
    }

    pub fn remove_object(&mut self, object: usize) -> Result<(), ()> {
        if self.objects.get(object).is_none() {
            return Err(());
        }

        self.objects[object] = Box::new(NullObject { _dummy: 0.0 });
        let object_refs = self.object_references[object].clone();
        for i in object_refs {
            self.remove_instance(i).unwrap();
        }

        self.object_references[object].clear();
        self.empty_object_slots.push(object);
        self.flags.set_flag(SceneFlags::Dirty);
        Ok(())
    }

    pub fn add_instance(&mut self, index: usize, transform: Mat4) -> Result<usize, ()> {
        let instance_index = {
            if self.objects.get(index).is_none() || self.object_references.get(index).is_none() {
                return Err(());
            }

            if !self.empty_instance_slots.is_empty() {
                let new_index = self.empty_instance_slots.pop().unwrap();
                self.instances[new_index] = Instance::new(index as isize, &self.objects[index].bounds(), transform);
                self.instance_references[new_index] = index;
                return Ok(new_index);
            }

            self.instances.push(Instance::new(index as isize, &self.objects[index].bounds(), transform));
            self.instances.len() - 1
        };
        self.instance_references.push(index);

        self.object_references[index].insert(instance_index);
        self.flags.set_flag(SceneFlags::Dirty);
        self.flags.set_flag(SceneFlags::Dirty);
        Ok(instance_index)
    }

    pub fn set_instance_object(&mut self, instance: usize, obj_index: usize) -> Result<(), ()> {
        if self.objects.get(obj_index).is_none() || self.instances.get(instance).is_none() {
            return Err(());
        }

        let old_obj_index = self.instance_references[instance];
        self.object_references[old_obj_index].remove(&instance);
        self.instances[instance] = Instance::new(obj_index as isize, &self.objects[obj_index].bounds(), self.instances[instance].get_transform());
        self.object_references[obj_index].insert(instance);
        self.instance_references[instance] = obj_index;
        self.flags.set_flag(SceneFlags::Dirty);
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

        self.instances[index] = Instance::new(-1, &self.objects[index].bounds(), self.instances[index].get_transform());
        self.instance_references[index] = std::usize::MAX;
        self.empty_instance_slots.push(index);
        self.flags.set_flag(SceneFlags::Dirty);
        Ok(())
    }

    pub fn build_bvh(&mut self) {
        if self.flags.has_flag(SceneFlags::Dirty) { // Need to rebuild bvh
            let aabbs: Vec<AABB> = self.instances.iter().map(|o| { o.bounds() }).collect::<Vec<AABB>>();
            self.bvh = BVH::construct(aabbs.as_slice());
            self.mbvh = MBVH::construct(&self.bvh);
        }
    }

    pub fn create_intersector(&self) -> Intersector {
        Intersector {
            objects: self.objects.as_slice(),
            instances: self.instances.as_slice(),
            bvh: &self.bvh,
            mbvh: &self.mbvh,
        }
    }
}

#[derive(Copy, Clone)]
pub struct Intersector<'a> {
    objects: &'a [Box<dyn Intersect>],
    instances: &'a [Instance],
    bvh: &'a BVH,
    mbvh: &'a MBVH,
}

impl<'a> Intersector<'a> {
    pub fn occludes(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> bool {
        let intersection = |i, t_min, t_max| {
            let instance = &self.instances[i as usize];
            if let Some((origin, direction)) = instance.intersects(origin, direction, t_max) {
                return self.objects[instance.get_hit_id() as usize].occludes(origin, direction, t_min, t_max);
            }
            false
        };

        let bvh = self.bvh;
        let mbvh = self.mbvh;

        unsafe {
            return match USE_MBVH {
                true => mbvh.occludes(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection,
                ),
                _ => bvh.occludes(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection,
                )
            };
        }
    }

    pub fn intersect(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let mut instance_id = -1;
        let intersection = |i, t_min, t_max| {
            let instance = &self.instances[i as usize];
            if let Some((origin, direction)) = instance.intersects(origin, direction, t_max) {
                if let Some(hit) = self.objects[instance.get_hit_id() as usize].intersect(origin, direction, t_min, t_max) {
                    instance_id = i as i32;
                    return Some((hit.t, hit));
                }
            }
            None
        };

        let hit = unsafe {
            match USE_MBVH {
                true => self.mbvh.traverse(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection,
                ),
                _ => self.bvh.traverse(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection,
                )
            }
        };

        hit.and_then(|hit| Some(self.instances[instance_id as usize].transform_hit(hit)))
    }

    pub fn intersect_t(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> Option<f32> {
        let intersection = |i, t_min, t_max| {
            let instance = &self.instances[i as usize];
            if let Some((origin, direction)) = instance.intersects(origin, direction, t_max) {
                return self.objects[instance.get_hit_id() as usize].intersect_t(origin, direction, t_min, t_max);
            }
            None
        };

        unsafe {
            return match USE_MBVH {
                true => {
                    self.mbvh.traverse_t(
                        origin.as_ref(),
                        direction.as_ref(),
                        t_min,
                        t_max,
                        intersection,
                    )
                }
                _ => {
                    self.bvh.traverse_t(
                        origin.as_ref(),
                        direction.as_ref(),
                        t_min,
                        t_max,
                        intersection,
                    )
                }
            };
        }
    }

    pub fn depth_test(&self, origin: Vec3, direction: Vec3, t_min: f32, t_max: f32) -> (f32, u32) {
        let intersection = |i, t_min, t_max| -> Option<(f32, u32)> {
            let instance = &self.instances[i as usize];
            if let Some((origin, direction)) = instance.intersects(origin, direction, t_max) {
                return self.objects[instance.get_hit_id() as usize].depth_test(origin, direction, t_min, t_max);
            }
            None
        };

        unsafe {
            return match USE_MBVH {
                true => {
                    self.mbvh.depth_test(
                        origin.as_ref(),
                        direction.as_ref(),
                        t_min,
                        t_max,
                        intersection,
                    )
                }
                _ => {
                    self.bvh.depth_test(
                        origin.as_ref(),
                        direction.as_ref(),
                        t_min,
                        t_max,
                        intersection,
                    )
                }
            };
        }
    }
}

impl Bounds for Scene {
    fn bounds(&self) -> AABB {
        self.bvh.bounds()
    }
}