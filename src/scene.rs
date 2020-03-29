use crate::bvh::*;
use crate::utils::Timer;
use glam::*;
use rayon::prelude::*;
use crate::objects::*;
use crate::utils::*;
use std::sync::Arc;

enum SceneFlags {
    Dirty = 0,
}

impl Into<u8> for SceneFlags {
    fn into(self) -> u8 {
        self as u8
    }
}

pub struct Scene {
    objects: Vec<Arc<Box<dyn Intersect>>>,
    instances: Vec<Instance>,
    aabbs: Vec<AABB>,
    bvh: Option<BVH>,
    flags: Flags,
}

#[allow(dead_code)]
impl Scene {
    pub fn new() -> Scene {
        Scene {
            objects: Vec::new(),
            instances: Vec::new(),
            bvh: None,
            aabbs: Vec::new(),
            flags: Flags::new(),
        }
    }

    pub fn add_object(&mut self, object: Box<dyn Intersect>) -> usize {
        self.objects.push(Arc::new(object));
        self.objects.len() - 1
    }

    pub fn add_instance(&mut self, index: usize, transform: Mat4) -> usize {
        self.instances.push(Instance::new(self.objects[index].clone(), transform));
        self.flags.set_flag(SceneFlags::Dirty);
        self.instances.len() - 1
    }

    pub fn intersect(&self, origin: Vec3, direction: Vec3) -> Option<HitRecord> {
        let mut hit_record = None;
        let mut t = crate::constants::DEFAULT_T_MAX;
        for instance in &self.instances {
            if let Some(hit) = instance.intersect(origin, direction, crate::constants::DEFAULT_T_MIN, t) {
                t = hit.t;
                hit_record = Some(hit);
            }
        }

        // if let Some(bvh) = &self.bvh {
        //     let intersection = |i, t_min, t_max| -> Option<(f32, HitRecord)> {
        //         if let Some(hit) = self.instances[i as usize].intersect(origin, direction, t_min, t_max) {
        //             Some((hit.t, hit))
        //         } else {
        //             None
        //         }
        //     };
        //
        //     hit_record = BVHNode::traverse_stack(bvh.nodes.as_slice(), bvh.prim_indices.as_slice(),
        //                                          origin, direction, 1e-5,
        //                                          crate::constants::DEFAULT_T_MAX,
        //                                          intersection);
        // } else {
        //     panic!("Invalid BVH!");
        // }

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

