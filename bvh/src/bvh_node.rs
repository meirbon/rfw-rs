use glam::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::fmt::{Display, Formatter};
use std::sync::mpsc::Sender;

use crate::AABB;

#[derive(Debug, Clone)]
pub struct BVHNode {
    pub bounds: AABB
}

impl Display for BVHNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.bounds)
    }
}

pub struct NewNodeInfo {
    pub left: usize,
    pub left_box: AABB,
    pub right_box: AABB,
}

pub struct NodeUpdatePayLoad {
    pub index: usize,
    pub bounds: AABB,
}

#[allow(dead_code)]
impl BVHNode {
    const BINS: usize = 7;
    const MAX_PRIMITIVES: i32 = 5;
    const MAX_DEPTH: u32 = 32;

    pub fn new() -> BVHNode {
        BVHNode {
            bounds: AABB::new()
        }
    }

    pub fn get_left_first(&self) -> i32 {
        self.bounds.left_first
    }

    pub fn get_count(&self) -> i32 {
        self.bounds.count
    }

    pub fn has_children(&self) -> bool { self.bounds.count < 0 && self.bounds.left_first >= 0 }

    pub fn is_leaf(&self) -> bool { self.bounds.count >= 0 }

    pub fn subdivide_mt<'a>(
        index: usize,
        mut bounds: AABB,
        aabbs: &'a [AABB],
        centers: &'a [[f32; 3]],
        update_node: Sender<NodeUpdatePayLoad>,
        prim_indices: &'a mut [u32],
        depth: u32,
        pool_ptr: Arc<AtomicUsize>,
        thread_count: Arc<AtomicUsize>,
        max_threads: usize,
        scope: &crossbeam::thread::Scope<'a>,
    ) {
        let depth = depth + 1;
        if depth >= Self::MAX_DEPTH {
            bounds.count = 0;
            update_node.send(NodeUpdatePayLoad { index, bounds }).unwrap();
            return;
        }

        let new_nodes = Self::partition(&bounds, aabbs, centers, prim_indices, pool_ptr.clone());
        if new_nodes.is_none() { return; }

        let new_nodes = new_nodes.unwrap();
        bounds.left_first = new_nodes.left as i32;
        bounds.count = -1;
        update_node.send(NodeUpdatePayLoad { index, bounds }).unwrap();

        let (left_indices, right_indices) = prim_indices.split_at_mut(new_nodes.left_box.count as usize);
        let threads = thread_count.load(Ordering::SeqCst);

        let mut handle = None;

        if new_nodes.left_box.count > Self::MAX_PRIMITIVES {
            let left = new_nodes.left;
            let left_box = new_nodes.left_box;
            let sender = update_node.clone();
            let tc = thread_count.clone();
            let pp = pool_ptr.clone();

            if threads < num_cpus::get() {
                thread_count.fetch_add(1, Ordering::SeqCst);
                handle = Some(scope.spawn(move |s| {
                    Self::subdivide_mt(left, left_box, aabbs, centers, sender, left_indices, depth, pp, tc, max_threads, s);
                }));
            } else {
                Self::subdivide_mt(left, left_box, aabbs, centers, sender, left_indices, depth, pp, tc, max_threads, scope);
            }
        } else {
            update_node.send(NodeUpdatePayLoad { index: new_nodes.left, bounds: new_nodes.left_box }).unwrap();
        }

        if new_nodes.right_box.count > Self::MAX_PRIMITIVES {
            let right = new_nodes.left + 1;
            let right_box = new_nodes.right_box;
            Self::subdivide_mt(right, right_box, aabbs, centers, update_node, right_indices, depth, pool_ptr, thread_count.clone(), max_threads, scope);
        } else {
            update_node.send(NodeUpdatePayLoad { index: new_nodes.left + 1, bounds: new_nodes.right_box }).unwrap();
        }

        if let Some(handle) = handle {
            handle.join().unwrap();
            thread_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    // Reference single threaded subdivide method
    pub fn subdivide(index: usize, aabbs: &[AABB], centers: &[[f32; 3]], tree: &mut [BVHNode], prim_indices: &mut [u32], depth: u32, pool_ptr: Arc<AtomicUsize>) {
        let depth = depth + 1;
        if depth >= Self::MAX_DEPTH {
            return;
        }

        let new_nodes = Self::partition(&tree[index].bounds, aabbs, centers, prim_indices, pool_ptr.clone());
        if new_nodes.is_none() { return; }
        let new_nodes = new_nodes.unwrap();

        tree[index].bounds.left_first = new_nodes.left as i32;
        tree[index].bounds.count = -1;

        let (left_indices, right_indices) = prim_indices.split_at_mut(new_nodes.left_box.count as usize);
        tree[new_nodes.left].bounds = new_nodes.left_box;
        if tree[new_nodes.left].bounds.count > Self::MAX_PRIMITIVES {
            Self::subdivide(new_nodes.left, aabbs, centers, tree, left_indices, depth, pool_ptr.clone());
        }

        tree[new_nodes.left + 1].bounds = new_nodes.right_box;
        if tree[new_nodes.left + 1].bounds.count > Self::MAX_PRIMITIVES {
            Self::subdivide(new_nodes.left + 1, aabbs, centers, tree, right_indices, depth, pool_ptr.clone());
        }
    }

    pub fn partition(
        bounds: &AABB,
        aabbs: &[AABB],
        centers: &[[f32; 3]],
        prim_indices: &mut [u32],
        pool_ptr: Arc<AtomicUsize>,
    ) -> Option<NewNodeInfo> {
        let mut best_split = 0.0 as f32;
        let mut best_axis = 0;

        let mut best_left_box = AABB::new();
        let mut best_right_box = AABB::new();

        let mut lowest_cost = 1e34;
        let parent_cost = bounds.area() * bounds.count as f32;
        let lengths = bounds.lengths();

        let bin_size = 1.0 / (Self::BINS + 2) as f32;

        for axis in 0..3 {
            for i in 1..(Self::BINS + 2) {
                let bin_offset = i as f32 * bin_size;
                let split_offset = bounds.min[axis] + lengths[axis] * bin_offset;

                let mut left_count = 0;
                let mut right_count = 0;

                let mut left_box = AABB::new();
                let mut right_box = AABB::new();

                let (left_area, right_area) = {
                    for idx in 0..bounds.count {
                        let idx = unsafe { *prim_indices.get_unchecked(idx as usize) as usize };
                        let center = centers[idx][axis];
                        let aabb = unsafe { aabbs.get_unchecked(idx) };

                        if center <= split_offset {
                            left_box.grow_bb(aabb);
                            left_count = left_count + 1;
                        } else {
                            right_box.grow_bb(aabb);
                            right_count = right_count + 1;
                        }
                    }

                    (left_box.area(), right_box.area())
                };

                let split_node_cost = left_area * left_count as f32 + right_area * right_count as f32;
                if lowest_cost > split_node_cost {
                    lowest_cost = split_node_cost;
                    best_split = split_offset;
                    best_axis = axis;
                    best_left_box = left_box;
                    best_right_box = right_box;
                }
            }
        }

        if parent_cost < lowest_cost { return None; }

        let left_first = bounds.left_first;
        let mut left_count = 0;

        for idx in 0..bounds.count {
            let id = unsafe { *prim_indices.get_unchecked(idx as usize) as usize };
            let center = centers[id][best_axis];

            if center <= best_split {
                prim_indices.swap((idx) as usize, (left_count) as usize);
                left_count = left_count + 1;
            }
        }

        let right_first = bounds.left_first + left_count;
        let right_count = bounds.count - left_count;

        let left = pool_ptr.fetch_add(2, Ordering::SeqCst);

        best_left_box.left_first = left_first;
        best_left_box.count = left_count;
        best_left_box.offset_by(1e-6);

        best_right_box.left_first = right_first;
        best_right_box.count = right_count;
        best_right_box.offset_by(1e-6);

        Some(NewNodeInfo {
            left,
            left_box: best_left_box,
            right_box: best_right_box,
        })
    }

    pub fn depth_test<I>(
        tree: &[BVHNode],
        prim_indices: &[u32],
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        t_max: f32,
        depth_test: I,
    ) -> (f32, u32)
        where I: Fn(usize, f32, f32) -> Option<(f32, u32)>
    {
        let mut t = t_max;
        let dir_inverse = Vec3::new(1.0, 1.0, 1.0) / dir;

        if tree[0].bounds.intersect(origin, dir_inverse, t).is_none() {
            return (t_max, 0);
        }

        let mut depth: i32 = 0;
        let mut hit_stack = [0; 32];
        let mut stack_ptr: i32 = 0;

        while stack_ptr >= 0 {
            depth = depth + 1;
            let node = &tree[hit_stack[stack_ptr as usize] as usize];
            stack_ptr = stack_ptr - 1;

            if node.bounds.count > -1 { // Leaf node
                for i in 0..node.bounds.count {
                    let prim_id = prim_indices[(node.bounds.left_first + i) as usize];
                    if let Some((new_t, d)) = depth_test(prim_id as usize, t_min, t) {
                        t = new_t;
                        depth += d as i32;
                    }
                }
            } else {
                let hit_left = tree[node.bounds.left_first as usize].bounds.intersect(origin, dir_inverse, t);
                let hit_right = tree[(node.bounds.left_first + 1) as usize].bounds.intersect(origin, dir_inverse, t);
                let new_stack_ptr = Self::sort_nodes(hit_left, hit_right, hit_stack.as_mut(), stack_ptr, node.bounds.left_first);
                stack_ptr = new_stack_ptr;
            }
        }

        (t, depth as u32)
    }

    pub fn traverse<I, N, U, R>(
        &self,
        tree: &[BVHNode],
        prim_indices: &[u32],
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        t_max: f32,
        intersection_test: I,
    ) -> Option<R>
        where I: Fn(usize, f32, f32) -> Option<(f32, R)>, R: Copy
    {
        let mut hit_index: i32 = -1;
        let mut t = t_max;
        let mut hit_record = None;

        self.traverse_recursive(tree, prim_indices, origin, Vec3::new(1.0, 1.0, 1.0) / dir, t_min, &mut t, &mut hit_index, &mut hit_record, &intersection_test);

        hit_record
    }

    fn traverse_recursive<I, R>(&self, tree: &[BVHNode],
                                prim_indices: &[u32],
                                origin: Vec3,
                                dir_inverse: Vec3,
                                t_min: f32,
                                t: &mut f32,
                                hit_id: &mut i32,
                                hit_record: &mut Option<R>,
                                intersection_test: &I)
        where I: Fn(usize, f32, f32) -> Option<(f32, R)>, R: Copy
    {
        if self.bounds.count > -1 {
            for i in 0..self.bounds.count {
                let prim_id = prim_indices[(self.bounds.left_first + i) as usize];
                if let Some((new_t, new_hit)) = intersection_test(prim_id as usize, t_min, *t) {
                    *t = new_t;
                    *hit_record = Some(new_hit);
                }
            }
        } else {
            let left = tree[self.bounds.left_first as usize].bounds.intersect(origin, dir_inverse, *t);
            let right = tree[(self.bounds.left_first + 1) as usize].bounds.intersect(origin, dir_inverse, *t);
            if left.is_some() & &right.is_some() {
                let (t_near_left, _) = left.unwrap();
                let (t_near_right, _) = right.unwrap();

                if t_near_left < t_near_right {
                    tree[self.bounds.left_first as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, hit_record, intersection_test);
                    tree[(self.bounds.left_first + 1) as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, hit_record, intersection_test);
                } else {
                    tree[(self.bounds.left_first + 1) as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, hit_record, intersection_test);
                    tree[self.bounds.left_first as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, hit_record, intersection_test);
                }
            } else if left.is_some() {
                tree[self.bounds.left_first as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, hit_record, intersection_test);
            } else if right.is_some() {
                tree[(self.bounds.left_first + 1) as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, hit_record, intersection_test);
            }
        }
    }

    pub fn traverse_stack<I, R>(
        tree: &[BVHNode],
        prim_indices: &[u32],
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        t_max: f32,
        mut intersection_test: I,
    ) -> Option<R>
        where I: FnMut(usize, f32, f32) -> Option<(f32, R)>, R: Copy
    {
        let mut hit_stack = [0; 32];
        let mut stack_ptr: i32 = 0;
        let mut t = t_max;
        let mut hit_record = None;

        let dir_inverse = Vec3::new(1.0, 1.0, 1.0) / dir;
        hit_stack[stack_ptr as usize] = 0;
        while stack_ptr >= 0 {
            let node = &tree[hit_stack[stack_ptr as usize] as usize];
            stack_ptr = stack_ptr - 1;

            if node.bounds.count > -1 { // Leaf node
                for i in 0..node.bounds.count {
                    let prim_id = prim_indices[(node.bounds.left_first + i) as usize];
                    if let Some((new_t, new_hit)) = intersection_test(prim_id as usize, t_min, t) {
                        t = new_t;
                        hit_record = Some(new_hit);
                    }
                }
            } else {
                let hit_left = tree[node.bounds.left_first as usize].bounds.intersect(origin, dir_inverse, t);
                let hit_right = tree[(node.bounds.left_first + 1) as usize].bounds.intersect(origin, dir_inverse, t);
                stack_ptr = Self::sort_nodes(hit_left, hit_right, hit_stack.as_mut(), stack_ptr, node.bounds.left_first);
            }
        }

        hit_record
    }

    pub fn traverse_t<I>(
        tree: &[BVHNode],
        prim_indices: &[u32],
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        t_max: f32,
        mut intersection_test: I,
    ) -> Option<f32>
        where I: FnMut(usize, f32, f32) -> Option<f32>
    {
        let mut hit_stack = [0; 32];
        let mut stack_ptr: i32 = 0;
        let mut t = t_max;

        let dir_inverse = Vec3::new(1.0, 1.0, 1.0) / dir;
        hit_stack[stack_ptr as usize] = 0;
        while stack_ptr >= 0 {
            let node = &tree[hit_stack[stack_ptr as usize] as usize];
            stack_ptr = stack_ptr - 1;

            if node.bounds.count > -1 { // Leaf node
                for i in 0..node.bounds.count {
                    let prim_id = prim_indices[(node.bounds.left_first + i) as usize];
                    if let Some(new_t) = intersection_test(prim_id as usize, t_min, t) {
                        t = new_t;
                    }
                }
            } else {
                let hit_left = tree[node.bounds.left_first as usize].bounds.intersect(origin, dir_inverse, t);
                let hit_right = tree[(node.bounds.left_first + 1) as usize].bounds.intersect(origin, dir_inverse, t);
                stack_ptr = Self::sort_nodes(hit_left, hit_right, hit_stack.as_mut(), stack_ptr, node.bounds.left_first);
            }
        }

        if t < t_max { Some(t) } else { None }
    }

    pub fn occludes<I>(
        tree: &[BVHNode],
        prim_indices: &[u32],
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        t_max: f32,
        mut intersection_test: I,
    ) -> bool
        where I: FnMut(usize, f32, f32) -> bool
    {
        let mut hit_stack = [0; 32];
        let mut stack_ptr: i32 = 0;

        let dir_inverse = Vec3::new(1.0, 1.0, 1.0) / dir;
        hit_stack[stack_ptr as usize] = 0;
        while stack_ptr >= 0 {
            let node = &tree[hit_stack[stack_ptr as usize] as usize];
            stack_ptr = stack_ptr - 1;

            if node.bounds.count > -1 { // Leaf node
                for i in 0..node.bounds.count {
                    let prim_id = prim_indices[(node.bounds.left_first + i) as usize];
                    if intersection_test(prim_id as usize, t_min, t_max) { return true; }
                }
            } else {
                let hit_left = tree[node.bounds.left_first as usize].bounds.intersect(origin, dir_inverse, t_max);
                let hit_right = tree[(node.bounds.left_first + 1) as usize].bounds.intersect(origin, dir_inverse, t_max);
                stack_ptr = Self::sort_nodes(hit_left, hit_right, hit_stack.as_mut(), stack_ptr, node.bounds.left_first);
            }
        }

        false
    }

    fn sort_nodes(
        left: Option<(f32, f32)>,
        right: Option<(f32, f32)>,
        hit_stack: &mut [i32],
        mut stack_ptr: i32,
        left_first: i32,
    ) -> i32 {
        if left.is_some() & &right.is_some() {
            let (t_near_left, _) = left.unwrap();
            let (t_near_right, _) = right.unwrap();

            if t_near_left < t_near_right {
                stack_ptr = stack_ptr + 1;
                hit_stack[stack_ptr as usize] = left_first;
                stack_ptr = stack_ptr + 1;
                hit_stack[stack_ptr as usize] = left_first + 1;
            } else {
                stack_ptr = stack_ptr + 1;
                hit_stack[stack_ptr as usize] = left_first + 1;
                stack_ptr = stack_ptr + 1;
                hit_stack[stack_ptr as usize] = left_first;
            }
        } else if left.is_some() {
            stack_ptr = stack_ptr + 1;
            hit_stack[stack_ptr as usize] = left_first;
        } else if right.is_some() {
            stack_ptr = stack_ptr + 1;
            hit_stack[stack_ptr as usize] = left_first + 1;
        }

        stack_ptr
    }
}