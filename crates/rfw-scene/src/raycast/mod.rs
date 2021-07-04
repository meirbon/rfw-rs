use crate::{InstanceList2D, InstanceList3D, Mesh2D, Mesh3D};
use rfw_math::*;
use rtbvh::*;
use std::num::NonZeroUsize;

use bitflags::bitflags;

bitflags! {
    #[derive(Default)]
    pub struct HitMask: u32 {
        const ALL = u32::MAX;
        const MESH_2D = 1;
        const MESH_3D = 2;
    }
}

#[derive(Debug, Clone)]
pub struct Intersectable {
    pub mesh_mbvh: Mbvh,
    pub mbvh: Mbvh,
    pub aabbs: Vec<Aabb>,
    pub mask: HitMask,
}

impl Bounds for Intersectable {
    fn bounds(&self) -> Aabb<i32> {
        self.mbvh.bounds()
    }
}

impl Default for Intersectable {
    fn default() -> Self {
        Self {
            mesh_mbvh: Default::default(),
            mbvh: Default::default(),
            aabbs: Vec::new(),
            mask: HitMask::empty(),
        }
    }
}

impl Intersectable {
    pub fn from_2d_instance_list(mesh: &Mesh2D, list: &InstanceList2D) -> Result<Self, BuildError> {
        // Construct a performant bvh representation of the mesh itself
        let mesh_mbvh = Mbvh::from(
            Builder {
                aabbs: None,
                primitives: &mesh.triangles,
                primitives_per_leaf: None,
            }
            .construct_binned_sah()?,
        );

        // Construct a bvh over all instances
        let aabb = mesh_mbvh.bounds();
        let aabbs = list
            .iter()
            .map(|i| aabb.transformed(i.get_matrix()))
            .collect::<Vec<Aabb>>();

        let bvh = Builder {
            aabbs: Some(&aabbs),
            primitives: &aabbs,
            primitives_per_leaf: NonZeroUsize::new(1),
        }
        .construct_binned_sah()?;

        let mbvh = Mbvh::construct(&bvh);
        Ok(Self {
            mesh_mbvh,
            mbvh,
            aabbs,
            mask: HitMask::MESH_3D,
        })
    }

    pub fn from_3d_instance_list(mesh: &Mesh3D, list: &InstanceList3D) -> Result<Self, BuildError> {
        // Construct a performant bvh representation of the mesh itself
        let mesh_mbvh = Mbvh::from(
            Builder {
                aabbs: None,
                primitives: &mesh.triangles,
                primitives_per_leaf: None,
            }
            .construct_binned_sah()?,
        );

        // Construct a bvh over all instances
        let aabb = mesh_mbvh.bounds();
        let aabbs = list
            .iter()
            .map(|i| aabb.transformed(i.get_matrix()))
            .collect::<Vec<Aabb>>();
        let bvh = Builder {
            aabbs: Some(&aabbs),
            primitives: &aabbs,
            primitives_per_leaf: NonZeroUsize::new(1),
        }
        .construct_binned_sah()?;

        let mbvh = Mbvh::construct(&bvh);
        Ok(Self {
            mesh_mbvh,
            mbvh,
            aabbs,
            mask: HitMask::MESH_2D,
        })
    }

    pub fn rebuild_2d_instances_list(&mut self, list: &InstanceList2D) {
        // Construct a bvh over all instances
        let aabb = self.mesh_mbvh.bounds();
        let aabbs = list
            .iter()
            .map(|i| aabb.transformed(i.get_matrix()))
            .collect::<Vec<Aabb>>();
        let bvh = Builder {
            aabbs: Some(&aabbs),
            primitives: &aabbs,
            primitives_per_leaf: NonZeroUsize::new(1),
        }
        .construct_binned_sah()
        .unwrap_or_default();
        self.mbvh = Mbvh::from(bvh);
    }

    pub fn rebuild_3d_instances_list(&mut self, list: &InstanceList3D) {
        // Construct a bvh over all instances
        let aabb = self.mesh_mbvh.bounds();
        let aabbs = list
            .iter()
            .map(|i| aabb.transformed(i.get_matrix()))
            .collect::<Vec<Aabb>>();
        let bvh = Builder {
            aabbs: Some(&aabbs),
            primitives: &aabbs,
            primitives_per_leaf: NonZeroUsize::new(1),
        }
        .construct_binned_sah()
        .unwrap_or_default();
        self.mbvh = Mbvh::from(bvh);
    }

    pub fn intersect_3d(&self, ray: &mut Ray, mesh: &Mesh3D, list: &InstanceList3D) -> Option<u32> {
        let mut hit = None;
        let matrices = list.matrices();

        // Loop over every potential instance that was hit.
        for (id, ray) in self.mbvh.traverse_iter_indices(ray) {
            // TODO: Should we store the inverse somewhere?
            let instance_inverse_matrix = matrices[id as usize].inverse();

            let origin = (instance_inverse_matrix * ray.origin.extend(1.0)).truncate();
            let direction = (instance_inverse_matrix * ray.direction.extend(0.0)).truncate();

            let mut new_ray = Ray::new(origin, direction);
            new_ray.t_min = ray.t_min;
            new_ray.t = ray.t;

            for (triangle, new_ray) in self.mesh_mbvh.traverse_iter(&mut new_ray, &mesh.triangles) {
                if triangle.intersect(new_ray) {
                    ray.t = new_ray.t;
                    hit = Some(id);
                }
            }
        }
        hit
    }

    pub fn intersect_packet_3d(
        &self,
        ray: &mut RayPacket4,
        mesh: &Mesh3D,
        list: &InstanceList3D,
    ) -> [Option<u32>; 4] {
        let mut hit = [None; 4];
        let matrices = list.matrices();

        let origins = [
            vec4(ray.origin_x[0], ray.origin_y[0], ray.origin_z[0], 1.0),
            vec4(ray.origin_x[1], ray.origin_y[1], ray.origin_z[1], 1.0),
            vec4(ray.origin_x[2], ray.origin_y[2], ray.origin_z[2], 1.0),
            vec4(ray.origin_x[3], ray.origin_y[3], ray.origin_z[3], 1.0),
        ];

        let directions = [
            vec4(
                ray.direction_x[0],
                ray.direction_y[0],
                ray.direction_z[0],
                0.0,
            ),
            vec4(
                ray.direction_x[1],
                ray.direction_y[1],
                ray.direction_z[1],
                0.0,
            ),
            vec4(
                ray.direction_x[2],
                ray.direction_y[2],
                ray.direction_z[2],
                0.0,
            ),
            vec4(
                ray.direction_x[3],
                ray.direction_y[3],
                ray.direction_z[3],
                0.0,
            ),
        ];

        for (id, ray) in self.mbvh.traverse_iter_indices_packet(ray) {
            // TODO: Should we store the inverse somewhere?
            let instance_inverse_matrix = matrices[id as usize].inverse();

            let mut new_ray = RayPacket4::new(
                [
                    instance_inverse_matrix * origins[0],
                    instance_inverse_matrix * origins[1],
                    instance_inverse_matrix * origins[2],
                    instance_inverse_matrix * origins[3],
                ],
                [
                    instance_inverse_matrix * directions[0],
                    instance_inverse_matrix * directions[1],
                    instance_inverse_matrix * directions[2],
                    instance_inverse_matrix * directions[3],
                ],
            );

            for (id, new_ray) in self.mesh_mbvh.traverse_iter_indices_packet(&mut new_ray) {
                let triangle = &mesh.triangles[id as usize];
                if let Some(result) = triangle.intersect4(new_ray, Vec4::splat(1e-5)) {
                    for i in 0..4 {
                        if result[i] {
                            ray.t[i] = new_ray.t[i];
                            hit[i] = Some(id);
                        }
                    }
                }
            }
        }

        hit
    }

    pub fn intersect_2d(&self, ray: &mut Ray, mesh: &Mesh2D, list: &InstanceList2D) -> Option<u32> {
        let mut hit = None;
        let matrices = list.matrices();

        // Loop over every potential instance that was hit.
        for (id, ray) in self.mbvh.traverse_iter_indices(ray) {
            // TODO: Should we store the inverse somewhere?
            let instance_inverse_matrix = matrices[id as usize].inverse();

            let origin = (instance_inverse_matrix * ray.origin.extend(1.0)).truncate();
            let direction = (instance_inverse_matrix * ray.direction.extend(0.0)).truncate();

            let mut new_ray = Ray::new(origin, direction);
            new_ray.t_min = ray.t_min;
            new_ray.t = ray.t;

            for (triangle, new_ray) in self.mesh_mbvh.traverse_iter(&mut new_ray, &mesh.triangles) {
                if triangle.intersect(new_ray) {
                    ray.t = new_ray.t;
                    hit = Some(id);
                }
            }
        }
        hit
    }

    pub fn intersect_packet_2d(
        &self,
        ray: &mut RayPacket4,
        mesh: &Mesh2D,
        list: &InstanceList2D,
    ) -> Option<[u32; 4]> {
        let mut hit = [0_u32; 4];
        let matrices = list.matrices();

        let origins = [
            vec4(ray.origin_x[0], ray.origin_y[0], ray.origin_z[0], 1.0),
            vec4(ray.origin_x[1], ray.origin_y[1], ray.origin_z[1], 1.0),
            vec4(ray.origin_x[2], ray.origin_y[2], ray.origin_z[2], 1.0),
            vec4(ray.origin_x[3], ray.origin_y[3], ray.origin_z[3], 1.0),
        ];

        let directions = [
            vec4(
                ray.direction_x[0],
                ray.direction_y[0],
                ray.direction_z[0],
                0.0,
            ),
            vec4(
                ray.direction_x[1],
                ray.direction_y[1],
                ray.direction_z[1],
                0.0,
            ),
            vec4(
                ray.direction_x[2],
                ray.direction_y[2],
                ray.direction_z[2],
                0.0,
            ),
            vec4(
                ray.direction_x[3],
                ray.direction_y[3],
                ray.direction_z[3],
                0.0,
            ),
        ];

        for (id, ray) in self.mbvh.traverse_iter_indices_packet(ray) {
            // TODO: Should we store the inverse somewhere?
            let instance_inverse_matrix = matrices[id as usize].inverse();

            let mut new_ray = RayPacket4::new(
                [
                    instance_inverse_matrix * origins[0],
                    instance_inverse_matrix * origins[1],
                    instance_inverse_matrix * origins[2],
                    instance_inverse_matrix * origins[3],
                ],
                [
                    instance_inverse_matrix * directions[0],
                    instance_inverse_matrix * directions[1],
                    instance_inverse_matrix * directions[2],
                    instance_inverse_matrix * directions[3],
                ],
            );

            for (triangle, new_ray) in self
                .mesh_mbvh
                .traverse_iter_packet(&mut new_ray, &mesh.triangles)
            {
                if let Some(h) = triangle.intersect4(new_ray, Vec4::splat(1e-5)) {
                    for i in 0..4 {
                        if h[i] {
                            ray.t[i] = new_ray.t[i];
                            hit[i] = id;
                        }
                    }
                }
            }
        }

        if ray.t.cmplt(Vec4::splat(1e26)).all() {
            Some(hit)
        } else {
            None
        }
    }
}
