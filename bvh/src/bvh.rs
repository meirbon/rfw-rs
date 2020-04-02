use glam::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Sender, Receiver};
use rayon::prelude::*;

use crate::AABB;
use crate::bvh_node::*;
use crate::mbvh_node::*;
use crate::aabb::Bounds;

pub struct BVH {
    pub nodes: Vec<BVHNode>,
    pub prim_indices: Vec<u32>,
}

impl BVH {
    pub fn new(prim_count: usize) -> BVH {
        let nodes = vec![BVHNode { bounds: AABB::new() }; prim_count * 2];
        let mut prim_indices = vec![0; prim_count];
        for i in 0..prim_count { prim_indices[i] = i as u32; }

        BVH {
            nodes,
            prim_indices,
        }
    }

    pub fn construct<T: Bounds + Sync + Send + Sized>(objects: &[T]) -> BVH {
        let aabbs: Vec<AABB> = objects.into_par_iter().map(|o| { o.bounds() }).collect::<Vec<AABB>>();
        let mut bvh = BVH::new(objects.len());
        bvh.build(aabbs.as_slice());
        bvh
    }

    pub fn build(&mut self, aabbs: &[AABB]) {
        assert_eq!(aabbs.len(), (self.nodes.len() / 2));
        assert_eq!(aabbs.len(), self.prim_indices.len());

        let centers = aabbs.into_iter().map(|bb| {
            let mut center = [0.0; 3];
            for i in 0..3 {
                center[i] = (bb.min[i] + bb.max[i]) * 0.5;
            }
            center
        }).collect::<Vec<[f32; 3]>>();
        let pool_ptr = Arc::new(AtomicUsize::new(2));
        let depth = 1;

        let mut root_bounds = AABB::new();

        root_bounds.left_first = 0;
        root_bounds.count = aabbs.len() as i32;
        for aabb in aabbs { root_bounds.grow_bb(aabb); }
        self.nodes[0].bounds = root_bounds.clone();

        let (sender, receiver) = std::sync::mpsc::channel();
        let prim_indices = self.prim_indices.as_mut_slice();
        let thread_count = Arc::new(AtomicUsize::new(1));
        let handle = crossbeam::scope(|s| {
            BVHNode::subdivide_mt(0, root_bounds, aabbs, &centers, sender, prim_indices, depth, pool_ptr.clone(), thread_count, num_cpus::get(), s);
        });

        for payload in receiver.iter() {
            if payload.index >= self.nodes.len() {
                panic!("Index was {} but only {} nodes available, bounds: {}", payload.index, self.nodes.len(), payload.bounds);
            }
            self.nodes[payload.index].bounds = payload.bounds;
        }

        handle.unwrap();

        let node_count = pool_ptr.load(Ordering::SeqCst);
        self.nodes.resize(node_count, BVHNode::new());

        println!("Building done");
    }

    pub fn traverse<I, R>(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
        intersection_test: I,
    ) -> Option<R>
        where I: Fn(usize, f32, f32) -> Option<(f32, R)>, R: Copy
    {
        BVHNode::traverse_stack(
            self.nodes.as_slice(),
            self.prim_indices.as_slice(),
            origin,
            direction,
            t_min,
            t_max,
            intersection_test,
        )
    }

    pub fn traverse_t<I>(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
        intersection_test: I,
    ) -> Option<f32>
        where I: Fn(usize, f32, f32) -> Option<f32>
    {
        BVHNode::traverse_t(
            self.nodes.as_slice(),
            self.prim_indices.as_slice(),
            origin,
            direction,
            t_min,
            t_max,
            intersection_test,
        )
    }

    pub fn occludes<I>(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
        intersection_test: I,
    ) -> bool
        where I: Fn(usize, f32, f32) -> bool
    {
        BVHNode::occludes(
            self.nodes.as_slice(),
            self.prim_indices.as_slice(),
            origin,
            direction,
            t_min,
            t_max,
            intersection_test,
        )
    }
}

pub struct MBVH {
    pub nodes: Vec<BVHNode>,
    pub m_nodes: Vec<MBVHNode>,
    pub prim_indices: Vec<u32>,
}

impl MBVH {
    pub fn new(bvh: &BVH) -> MBVH {
        let nodes = &bvh.nodes;
        let mut m_nodes = vec![MBVHNode::new(); nodes.len()];
        let mut pool_ptr = 1;
        MBVHNode::merge_nodes(0, 0, nodes.as_slice(), m_nodes.as_mut_slice(), &mut pool_ptr);

        MBVH { nodes: bvh.nodes.clone(), m_nodes, prim_indices: bvh.prim_indices.clone() }
    }

    pub fn convert(bvh: BVH) -> MBVH {
        let nodes = bvh.nodes;
        let prim_indices = bvh.prim_indices;
        let mut m_nodes = vec![MBVHNode::new(); nodes.len()];
        let mut pool_ptr = 1;
        MBVHNode::merge_nodes(0, 0, nodes.as_slice(), m_nodes.as_mut_slice(), &mut pool_ptr);

        MBVH { nodes, m_nodes, prim_indices }
    }

    pub fn traverse<I, R>(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
        intersection_test: I,
    ) -> Option<R>
        where I: Fn(usize, f32, f32) -> Option<(f32, R)>, R: Copy
    {
        MBVHNode::traverse(self.m_nodes.as_slice(), self.prim_indices.as_slice(), origin, direction, t_min, t_max, intersection_test)
    }

    pub fn traverse_t<I>(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
        intersection_test: I,
    ) -> Option<f32>
        where I: Fn(usize, f32, f32) -> Option<f32>
    {
        MBVHNode::traverse_t(self.m_nodes.as_slice(), self.prim_indices.as_slice(), origin, direction, t_min, t_max, intersection_test)
    }

    pub fn occludes<I>(
        &self,
        origin: Vec3,
        direction: Vec3,
        t_min: f32,
        t_max: f32,
        intersection_test: I,
    ) -> bool
        where I: Fn(usize, f32, f32) -> bool
    {
        MBVHNode::occludes(self.m_nodes.as_slice(), self.prim_indices.as_slice(), origin, direction, t_min, t_max, intersection_test)
    }
}

impl From<BVH> for MBVH {
    fn from(bvh: BVH) -> Self {
        MBVH::convert(bvh)
    }
}
