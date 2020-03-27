pub mod aabb;

pub use aabb::AABB;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::fmt::{Display, Formatter};

use crate::scene::RayHit;
use crate::math::*;

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

    pub fn build(&mut self, aabbs: &[AABB]) {
        assert_eq!(aabbs.len(), (self.nodes.len() / 2));
        assert_eq!(aabbs.len(), self.prim_indices.len());

        let mut pool_ptr = Arc::new(AtomicUsize::new(2));
        let depth = 1;

        self.nodes[0].bounds.left_first = 0;
        self.nodes[0].bounds.count = aabbs.len() as i32;
        for aabb in aabbs { self.nodes[0].bounds.grow_bb(aabb); }

        BVHNode::subdivide(0, aabbs, self.nodes.as_mut_slice(), self.prim_indices.as_mut_slice(), depth, pool_ptr.clone());
    }
}

#[derive(Debug, Clone)]
pub struct BVHNode {
    pub bounds: AABB
}

impl Display for BVHNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.bounds)
    }
}

struct NewNodeInfo {
    pub left: usize,
    pub left_box: AABB,
    pub right_box: AABB,
}

impl BVHNode {
    const BINS: usize = 9;
    const MAX_PRIMITIVES: i32 = 5;
    const MAX_DEPTH: u32 = 64;

    pub fn new() -> BVHNode {
        BVHNode {
            bounds: AABB::new()
        }
    }

    pub fn subdivide(index: usize, aabbs: &[aabb::AABB], tree: &mut [BVHNode], prim_indices: &mut [u32], depth: u32, pool_ptr: Arc<AtomicUsize>) {
        let depth = depth + 1;
        if tree[index].bounds.count <= Self::MAX_PRIMITIVES || depth >= Self::MAX_DEPTH {
            return;
        }

        if let Some(new_nodes) = tree[index].partition(aabbs, prim_indices, pool_ptr.clone()) {
            tree[index].bounds.left_first = new_nodes.left as i32;
            tree[index].bounds.count = -1;

            // TODO: Figure out how to split tree in multiple splices as well and enable multithreaded BVH building
            let (left_indices, right_indices) = prim_indices.split_at_mut(new_nodes.left_box.count as usize);

            {
                tree[new_nodes.left].bounds = new_nodes.left_box;
                if tree[new_nodes.left].bounds.count > 0 {
                    Self::subdivide(new_nodes.left, aabbs, tree, left_indices, depth, pool_ptr.clone());
                }
            }

            {
                tree[new_nodes.left + 1].bounds = new_nodes.right_box;
                if tree[new_nodes.left + 1].bounds.count > 0 {
                    Self::subdivide(new_nodes.left + 1, aabbs, tree, right_indices, depth, pool_ptr.clone());
                }
            }
        }
    }

    pub fn partition(&mut self, aabbs: &[aabb::AABB], prim_indices: &mut [u32], pool_ptr: Arc<AtomicUsize>) -> Option<NewNodeInfo> {
        let mut lowest_cost = 1e34 as f32;
        let mut best_split = 0.0 as f32;
        let mut best_axis = 0;

        let mut best_left_box = AABB::new();
        let mut best_right_box = AABB::new();

        let mut parent_cost = self.bounds.area() * self.bounds.count as f32;
        let lengths = self.bounds.lengths();

        let bin_size = 1.0 / (Self::BINS + 2) as f32;

        for axis in 0..3 {
            for i in 1..(Self::BINS + 2) {
                let bin_offset = i as f32 * bin_size;
                let split_offset = self.bounds.min[axis] + lengths[axis] * bin_offset;

                let mut left_count = 0;
                let mut right_count = 0;

                let mut left_box = AABB::new();
                let mut right_box = AABB::new();

                let (left_area, right_area) = {
                    for idx in 0..self.bounds.count {
                        let idx = unsafe { *prim_indices.get_unchecked(idx as usize) as usize };
                        let aabb = unsafe { aabbs.get_unchecked(idx) };

                        let center = aabb.center()[axis];

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

        if parent_cost < lowest_cost {
            return None;
        }

        let left_first = self.bounds.left_first;
        let mut left_count = 0;
        let mut right_first = self.bounds.left_first;
        let mut right_count = self.bounds.count;
        for idx in 0..self.bounds.count {
            let aabb = unsafe { aabbs.get_unchecked(*prim_indices.get_unchecked(idx as usize) as usize) };
            let center = aabb.center()[best_axis];

            if center <= best_split {
                prim_indices.swap((idx) as usize, (left_count) as usize);
                left_count = left_count + 1;
                right_first = right_first + 1;
                right_count = right_count - 1;
            }
        }

        let left = pool_ptr.fetch_add(2, Ordering::SeqCst);

        best_left_box.left_first = left_first;
        best_left_box.count = left_count;
        best_left_box.offset_by(crate::constants::EPSILON);

        best_right_box.left_first = right_first;
        best_right_box.count = right_count;
        best_right_box.offset_by(crate::constants::EPSILON);

        Some(NewNodeInfo {
            left,
            left_box: best_left_box,
            right_box: best_right_box,
        })
    }

    pub fn depth_test_recursive<I>(&self, tree: &[BVHNode],
                                   prim_indices: &[u32],
                                   origin: Vec3,
                                   dir: Vec3,
                                   t_min: f32,
                                   t: &mut f32,
                                   intersection_test: I) -> u32
        where I: Fn(usize) -> Option<f32> + Copy
    {
        let dir_inverse = 1.0 / dir;

        let mut depth = 0;
        if self.bounds.count > -1 {
            for i in 0..self.bounds.count {
                let prim_id = prim_indices[(self.bounds.left_first + i) as usize];
                if let Some(new_t) = intersection_test(prim_id as usize) {
                    if new_t < *t {
                        *t = new_t;
                    }
                }
            }
        } else {
            let left = tree[self.bounds.left_first as usize].bounds.intersect(origin, dir_inverse, *t);
            let right = tree[(self.bounds.left_first + 1) as usize].bounds.intersect(origin, dir_inverse, *t);
            if left.is_some() & &right.is_some() {
                let (t_near_left, _) = left.unwrap();
                let (t_near_right, _) = right.unwrap();

                depth += 2;
                if t_near_left < t_near_right {
                    depth += tree[self.bounds.left_first as usize].depth_test_recursive(tree, prim_indices, origin, dir, t_min, t, intersection_test);
                    depth += tree[(self.bounds.left_first + 1) as usize].depth_test_recursive(tree, prim_indices, origin, dir, t_min, t, intersection_test);
                } else {
                    depth += tree[(self.bounds.left_first + 1) as usize].depth_test_recursive(tree, prim_indices, origin, dir, t_min, t, intersection_test);
                    depth += tree[self.bounds.left_first as usize].depth_test_recursive(tree, prim_indices, origin, dir, t_min, t, intersection_test);
                }
            } else if left.is_some() {
                depth += 1;
                depth += tree[self.bounds.left_first as usize].depth_test_recursive(tree, prim_indices, origin, dir, t_min, t, intersection_test);
            } else if right.is_some() {
                depth += 1;
                depth += tree[(self.bounds.left_first + 1) as usize].depth_test_recursive(tree, prim_indices, origin, dir, t_min, t, intersection_test);
            }
        }

        depth
    }

    pub fn depth_test<I>(
        tree: &[BVHNode],
        prim_indices: &[u32],
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        intersection_test: I,
    ) -> u32
        where I: Fn(usize) -> Option<f32>
    {
        let mut depth = 0;
        let mut hit_stack = [0; 32];
        let mut stack_ptr: i32 = 0;
        let mut t = 1e34;
        let dir_inverse = vec3(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

        if tree[0].bounds.intersect(origin, dir_inverse, t).is_none() {
            return depth;
        }

        while stack_ptr >= 0 {
            depth = depth + 1;
            let node = &tree[hit_stack[stack_ptr as usize] as usize];
            stack_ptr = stack_ptr - 1;

            if node.bounds.count > -1 { // Leaf node
                for i in 0..node.bounds.count {
                    let prim_id = prim_indices[(node.bounds.left_first + i) as usize];
                    if let Some(new_t) = intersection_test(prim_id as usize) {
                        if new_t < t && t > t_min {
                            t = new_t;
                        }
                    }
                }
            } else {
                let hit_left = tree[node.bounds.left_first as usize].bounds.intersect(origin, dir_inverse, t);
                let hit_right = tree[(node.bounds.left_first + 1) as usize].bounds.intersect(origin, dir_inverse, t);
                let new_stack_ptr = Self::sort_nodes(hit_left, hit_right, hit_stack.as_mut(), stack_ptr, node.bounds.left_first);
                depth += (new_stack_ptr - stack_ptr) as u32;
                stack_ptr = new_stack_ptr;
            }
        }

        depth
    }

    pub fn traverse<I, N, U>(
        &self,
        tree: &[BVHNode],
        prim_indices: &[u32],
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        intersection_test: I,
        get_normal: N,
        get_uv: U,
    ) -> Option<RayHit>
        where I: Fn(usize) -> Option<f32> + Copy,
              N: Fn(usize, f32, Vec3) -> Vec3,
              U: Fn(usize, f32, Vec3, Vec3) -> Vec2
    {
        let mut hit_index: i32 = -1;
        let mut t = 1e34;

        self.traverse_recursive(tree, prim_indices, origin, 1.0 / dir, t_min, &mut t, &mut hit_index, &intersection_test);

        if hit_index < 0 {
            return None;
        }

        let hit_index = hit_index as usize;
        let p: Vec3 = origin + dir * t;
        let normal = get_normal(hit_index, t, p);
        let uv = get_uv(hit_index, t, p, normal);

        Some(RayHit {
            normal,
            t,
            uv,
        })
    }

    fn traverse_recursive<I>(&self, tree: &[BVHNode],
                             prim_indices: &[u32],
                             origin: Vec3,
                             dir_inverse: Vec3,
                             t_min: f32,
                             t: &mut f32,
                             hit_id: &mut i32,
                             intersection_test: &I)
        where I: Fn(usize) -> Option<f32>
    {
        if self.bounds.count > -1 {
            for i in 0..self.bounds.count {
                let prim_id = prim_indices[(self.bounds.left_first + i) as usize];
                if let Some(new_t) = intersection_test(prim_id as usize) {
                    if new_t < *t && new_t > t_min {
                        *t = new_t;
                        *hit_id = prim_id as i32;
                    }
                }
            }
        } else {
            let left = tree[self.bounds.left_first as usize].bounds.intersect(origin, dir_inverse, *t);
            let right = tree[(self.bounds.left_first + 1) as usize].bounds.intersect(origin, dir_inverse, *t);
            if left.is_some() & &right.is_some() {
                let (t_near_left, _) = left.unwrap();
                let (t_near_right, _) = right.unwrap();

                if t_near_left < t_near_right {
                    tree[self.bounds.left_first as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, intersection_test);
                    tree[(self.bounds.left_first + 1) as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, intersection_test);
                } else {
                    tree[(self.bounds.left_first + 1) as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, intersection_test);
                    tree[self.bounds.left_first as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, intersection_test);
                }
            } else if left.is_some() {
                tree[self.bounds.left_first as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, intersection_test);
            } else if right.is_some() {
                tree[(self.bounds.left_first + 1) as usize].traverse_recursive(tree, prim_indices, origin, dir_inverse, t_min, t, hit_id, intersection_test);
            }
        }
    }

    pub fn traverse_stack<I, N, U>(
        tree: &[BVHNode],
        prim_indices: &[u32],
        origin: Vec3,
        dir: Vec3,
        t_min: f32,
        intersection_test: I,
        get_normal: N,
        get_uv: U,
    ) -> Option<RayHit>
        where I: Fn(usize) -> Option<f32>,
              N: Fn(usize, f32, Vec3) -> Vec3,
              U: Fn(usize, f32, Vec3, Vec3) -> Vec2
    {
        let mut hit_index: i32 = -1;
        let mut hit_stack = [0; 32];
        let mut stack_ptr: i32 = 0;
        let mut t = 1e34;

        let dir_inverse = vec3(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);
        hit_stack[stack_ptr as usize] = 0;
        while stack_ptr >= 0 {
            let node = &tree[hit_stack[stack_ptr as usize] as usize];
            stack_ptr = stack_ptr - 1;

            if node.bounds.count > -1 { // Leaf node
                for i in 0..node.bounds.count {
                    let prim_id = prim_indices[(node.bounds.left_first + i) as usize];
                    if let Some(new_t) = intersection_test(prim_id as usize) {
                        if new_t < t && new_t > t_min {
                            t = new_t;
                            hit_index = prim_id as i32;
                        }
                    }
                }
            } else {
                let hit_left = tree[node.bounds.left_first as usize].bounds.intersect(origin, dir_inverse, t);
                let hit_right = tree[(node.bounds.left_first + 1) as usize].bounds.intersect(origin, dir_inverse, t);
                stack_ptr = Self::sort_nodes(hit_left, hit_right, hit_stack.as_mut(), stack_ptr, node.bounds.left_first);
            }
        }

        if hit_index < 0 {
            return None;
        }

        let hit_index = hit_index as usize;
        let p: Vec3 = origin + dir * t;
        let normal = get_normal(hit_index, t, p);
        let uv = get_uv(hit_index, t, p, normal);

        Some(RayHit {
            normal,
            t,
            uv,
        })
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