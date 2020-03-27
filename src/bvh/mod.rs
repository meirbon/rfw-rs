use nalgebra_glm::*;

pub mod aabb;

pub use aabb::AABB;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct BVH {}

pub struct BVHNode {
    pub bounds: AABB
}

impl BVHNode {
    const BINS: usize = 9;

    pub fn new() -> BVHNode {
        BVHNode {
            bounds: AABB::new()
        }
    }

    pub fn subdivide(&mut self, aabbs: &[aabb::AABB], tree: &mut [BVHNode], prim_indices: &mut [u32], depth: u32, pool_ptr: Arc<u32>) {
        let depth = depth + 1;
        if self.bounds.count < 5 || depth >= 64 {
            return;
        }

        let mut left = -1;
        let mut right = -1;
    }

    pub fn partition(&mut self, aabbs: &[aabb::AABB], tree: &mut [BVHNode], prim_indices: &mut [u32], pool_ptr: Arc<AtomicUsize>) -> Option<(usize, usize)> {
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

                let split_offset = match axis {
                    0 => self.bounds.min.x + lengths.x * bin_offset,
                    1 => self.bounds.min.y + lengths.y * bin_offset,
                    2 => self.bounds.min.z + lengths.z * bin_offset,
                    _ => 0.0
                };

                let mut left_count = 0;
                let mut right_count = 0;

                let mut left_box = AABB::new();
                let mut right_box = AABB::new();

                let (left_area, right_area) = {
                    for idx in 0..self.bounds.count {
                        let aabb = unsafe { aabbs.get_unchecked(idx as usize) };

                        let center = aabb.center();
                        let is_left = match axis {
                            0 => center.x <= split_offset,
                            1 => center.y <= split_offset,
                            2 => center.z <= split_offset,
                            _ => true
                        };

                        if is_left {
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
            let aabb = unsafe { aabbs.get_unchecked(*prim_indices.get_unchecked((left_first + idx) as usize) as usize) };

            let center = aabb.center();
            let center = match best_axis {
                0 => center.x,
                1 => center.y,
                2 => center.z,
                _ => 0.0
            };

            if center <= best_split {
                prim_indices.swap((left_first + idx) as usize, (left_first + left_count) as usize);
                left_count = left_count + 1;
                right_first = right_first + 1;
                right_count = right_count - 1;
            }
        }

        let left = pool_ptr.fetch_add(2, Ordering::SeqCst);
        let right = left - 1;

        tree[left].bounds = best_left_box;
        tree[left].bounds.left_first = left_first;
        tree[left].bounds.count = left_count;

        tree[right].bounds = best_right_box;
        tree[right].bounds.left_first = right_first;
        tree[right].bounds.count = right_count;

        Some((left, right))
    }
}