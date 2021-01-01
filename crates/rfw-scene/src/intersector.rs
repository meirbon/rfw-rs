use crate::objects::*;
use crate::{InstanceID, PrimID};
use rfw_math::*;
use rtbvh::{Ray, RayPacket4, ShadowPacket4, MBVH};

pub struct TIntersector<'a> {
    meshes: &'a [Mesh],
    anim_meshes: &'a [AnimatedMesh],
    instances: &'a [Instance],
    mbvh: &'a MBVH,
}

impl<'a> TIntersector<'a> {
    pub fn new(
        meshes: &'a [Mesh],
        anim_meshes: &'a [AnimatedMesh],
        instances: &'a [Instance],
        mbvh: &'a MBVH,
    ) -> Self {
        Self {
            meshes,
            anim_meshes,
            instances,
            mbvh,
        }
    }

    pub fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool {
        let (origin, direction) = ray.get_vectors::<Vec3A>();

        let intersection = |i, t_min, t_max| {
            let instance = &self.instances[i as usize];
            let (origin, direction) = instance.transform(ray);

            match instance.object_id {
                ObjectRef::None => false,
                ObjectRef::Static(hit_id) => {
                    self.meshes[hit_id as usize].occludes((origin, direction).into(), t_min, t_max)
                }
                ObjectRef::Animated(hit_id) => self.anim_meshes[hit_id as usize].occludes(
                    (origin, direction).into(),
                    t_min,
                    t_max,
                ),
            }
        };

        let mbvh = self.mbvh;
        mbvh.occludes(
            origin.as_ref(),
            direction.as_ref(),
            t_min,
            t_max,
            intersection,
        )
    }

    pub fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let (origin, direction) = ray.get_vectors::<Vec3A>();

        let mut instance_id = -1;
        let intersection = |i, t_min, t_max| {
            let instance = &self.instances[i as usize];
            let (origin, direction) = instance.transform(ray);

            if let Some(hit) = match instance.object_id {
                ObjectRef::None => None,
                ObjectRef::Static(hit_id) => {
                    self.meshes[hit_id as usize].intersect((origin, direction).into(), t_min, t_max)
                }
                ObjectRef::Animated(hit_id) => self.anim_meshes[hit_id as usize].intersect(
                    (origin, direction).into(),
                    t_min,
                    t_max,
                ),
            } {
                instance_id = i as i32;
                Some((hit.t, hit))
            } else {
                None
            }
        };

        let hit = self.mbvh.traverse(
            origin.as_ref(),
            direction.as_ref(),
            t_min,
            t_max,
            intersection,
        );

        hit.and_then(|hit| Some(self.instances[instance_id as usize].transform_hit(hit)))
    }

    pub fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32> {
        let (origin, direction) = ray.get_vectors::<Vec3A>();

        let intersection = |i, t_min, t_max| {
            let instance = &self.instances[i as usize];
            let (origin, direction) = instance.transform(ray);

            match instance.object_id {
                ObjectRef::None => None,
                ObjectRef::Static(hit_id) => self.meshes[hit_id as usize].intersect_t(
                    (origin, direction).into(),
                    t_min,
                    t_max,
                ),
                ObjectRef::Animated(hit_id) => self.anim_meshes[hit_id as usize].intersect_t(
                    (origin, direction).into(),
                    t_min,
                    t_max,
                ),
            }
        };

        self.mbvh.traverse_t(
            origin.as_ref(),
            direction.as_ref(),
            t_min,
            t_max,
            intersection,
        )
    }

    pub fn depth_test(&self, ray: Ray, t_min: f32, t_max: f32) -> (f32, u32) {
        let (origin, direction) = ray.get_vectors::<Vec3A>();

        let intersection = |i, t_min, t_max| -> Option<(f32, u32)> {
            let instance = &self.instances[i as usize];

            let (origin, direction) = instance.transform(ray);
            match instance.object_id {
                ObjectRef::None => None,
                ObjectRef::Static(hit_id) => self.meshes[hit_id as usize].depth_test(
                    (origin, direction).into(),
                    t_min,
                    t_max,
                ),
                ObjectRef::Animated(hit_id) => self.anim_meshes[hit_id as usize].depth_test(
                    (origin, direction).into(),
                    t_min,
                    t_max,
                ),
            }
        };

        self.mbvh.depth_test(
            origin.as_ref(),
            direction.as_ref(),
            t_min,
            t_max,
            intersection,
        )
    }

    pub fn occludes4(&self, _packet: ShadowPacket4) -> [bool; 4] {
        [true; 4]
    }

    pub fn intersect4(
        &self,
        packet: &mut RayPacket4,
        t_min: [f32; 4],
    ) -> ([InstanceID; 4], [PrimID; 4]) {
        let mut instance_ids = [-1 as InstanceID; 4];
        let mut prim_ids = [-1 as PrimID; 4];

        let intersection = |instance_id, packet: &mut RayPacket4| {
            let instance_id = instance_id as usize;
            let instance = &self.instances[instance_id];
            let mut new_packet = instance.transform4(packet);
            match instance.object_id {
                ObjectRef::None => {}
                ObjectRef::Static(hit_id) => {
                    if let Some(hit) =
                        self.meshes[hit_id as usize].intersect4(&mut new_packet, &t_min)
                    {
                        for i in 0..4 {
                            if hit[i] >= 0 {
                                instance_ids[i] = instance_id as i32;
                                prim_ids[i] = hit[i];
                                packet.t[i] = new_packet.t[i];
                            }
                        }
                    }
                }
                ObjectRef::Animated(hit_id) => {
                    if let Some(hit) =
                        self.anim_meshes[hit_id as usize].intersect4(&mut new_packet, &t_min)
                    {
                        for i in 0..4 {
                            if hit[i] >= 0 {
                                instance_ids[i] = instance_id as i32;
                                prim_ids[i] = hit[i];
                                packet.t[i] = new_packet.t[i];
                            }
                        }
                    }
                }
            }
        };

        self.mbvh.traverse4(packet, intersection);

        (instance_ids, prim_ids)
    }

    pub fn get_mat_id(&self, instance_id: InstanceID, prim_id: PrimID) -> usize {
        let instance = self.instances.get(instance_id as usize).unwrap();
        match instance.object_id {
            ObjectRef::None => std::usize::MAX,
            ObjectRef::Static(hit_id) => self.meshes[hit_id as usize].get_mat_id(prim_id) as usize,
            ObjectRef::Animated(hit_id) => {
                self.anim_meshes[hit_id as usize].get_mat_id(prim_id) as usize
            }
        }
    }

    pub fn get_hit_record(
        &self,
        ray: Ray,
        t: f32,
        instance_id: InstanceID,
        prim_id: PrimID,
    ) -> HitRecord {
        debug_assert!(instance_id >= 0);
        debug_assert!(prim_id >= 0);

        let instance: &Instance = &self.instances[instance_id as usize];
        let ray = instance.transform_ray(ray);
        match instance.object_id {
            ObjectRef::None => HitRecord::default(),
            ObjectRef::Static(hit_id) => instance
                .transform_hit(self.meshes[hit_id as usize].get_hit_record(ray, t, prim_id as u32)),
            ObjectRef::Animated(hit_id) => instance.transform_hit(
                self.anim_meshes[hit_id as usize].get_hit_record(ray, t, prim_id as u32),
            ),
        }
    }

    pub fn get_hit_record4(
        &self,
        packet: &RayPacket4,
        instance_ids: [InstanceID; 4],
        prim_ids: [PrimID; 4],
    ) -> HitRecord4 {
        let hit0 = if instance_ids[0] >= 0 {
            let ray = packet.ray(0);
            let t = packet.t[0];
            self.get_hit_record(ray, t, instance_ids[0], prim_ids[0])
        } else {
            HitRecord::default()
        };

        let ray = packet.ray(1);
        let t = packet.t[1];
        let hit1 = if instance_ids[1] >= 0 {
            self.get_hit_record(ray, t, instance_ids[1], prim_ids[1])
        } else {
            HitRecord::default()
        };

        let ray = packet.ray(2);
        let t = packet.t[2];
        let hit2 = if instance_ids[2] >= 0 {
            self.get_hit_record(ray, t, instance_ids[2], prim_ids[2])
        } else {
            HitRecord::default()
        };

        let ray = packet.ray(3);
        let t = packet.t[3];
        let hit3 = if instance_ids[3] >= 0 {
            self.get_hit_record(ray, t, instance_ids[3], prim_ids[3])
        } else {
            HitRecord::default()
        };

        let hit: HitRecord4 = [hit0, hit1, hit2, hit3].into();

        let instances: [&Instance; 4] = [
            &self.instances[instance_ids[0].max(0) as usize],
            &self.instances[instance_ids[1].max(0) as usize],
            &self.instances[instance_ids[2].max(0) as usize],
            &self.instances[instance_ids[3].max(0) as usize],
        ];

        let inverse_matrices = [
            instances[0].get_inverse_transform(),
            instances[1].get_inverse_transform(),
            instances[2].get_inverse_transform(),
            instances[3].get_inverse_transform(),
        ];

        let normal_matrices = [
            instances[0].get_normal_transform(),
            instances[1].get_normal_transform(),
            instances[2].get_normal_transform(),
            instances[3].get_normal_transform(),
        ];

        let one = Vec4::one();

        let (p_x, p_y, p_z) = {
            let matrix_cols0 = inverse_matrices[0].to_cols_array();
            let matrix_cols1 = inverse_matrices[1].to_cols_array();
            let matrix_cols2 = inverse_matrices[2].to_cols_array();
            let matrix_cols3 = inverse_matrices[3].to_cols_array();

            // Col 0
            let m0_0 = Vec4::new(
                matrix_cols0[0],
                matrix_cols1[0],
                matrix_cols2[0],
                matrix_cols3[0],
            );
            let m0_1 = Vec4::new(
                matrix_cols0[1],
                matrix_cols1[1],
                matrix_cols2[1],
                matrix_cols3[1],
            );
            let m0_2 = Vec4::new(
                matrix_cols0[2],
                matrix_cols1[2],
                matrix_cols2[2],
                matrix_cols3[2],
            );

            // Col 1
            let m1_0 = Vec4::new(
                matrix_cols0[4],
                matrix_cols1[4],
                matrix_cols2[4],
                matrix_cols3[4],
            );
            let m1_1 = Vec4::new(
                matrix_cols0[5],
                matrix_cols1[5],
                matrix_cols2[5],
                matrix_cols3[5],
            );
            let m1_2 = Vec4::new(
                matrix_cols0[6],
                matrix_cols1[6],
                matrix_cols2[6],
                matrix_cols3[6],
            );

            // Col 2
            let m2_0 = Vec4::new(
                matrix_cols0[8],
                matrix_cols1[8],
                matrix_cols2[8],
                matrix_cols3[8],
            );
            let m2_1 = Vec4::new(
                matrix_cols0[9],
                matrix_cols1[9],
                matrix_cols2[9],
                matrix_cols3[9],
            );
            let m2_2 = Vec4::new(
                matrix_cols0[10],
                matrix_cols1[10],
                matrix_cols2[10],
                matrix_cols3[10],
            );

            // Col 3
            let m3_0 = Vec4::new(
                matrix_cols0[12],
                matrix_cols1[12],
                matrix_cols2[12],
                matrix_cols3[12],
            );
            let m3_1 = Vec4::new(
                matrix_cols0[13],
                matrix_cols1[13],
                matrix_cols2[13],
                matrix_cols3[13],
            );
            let m3_2 = Vec4::new(
                matrix_cols0[14],
                matrix_cols1[14],
                matrix_cols2[14],
                matrix_cols3[14],
            );

            let p_x = Vec4::from(hit.p_x);
            let p_y = Vec4::from(hit.p_y);
            let p_z = Vec4::from(hit.p_z);

            let mut new_p_x = m0_0 * p_x;
            let mut new_p_y = m0_1 * p_x;
            let mut new_p_z = m0_2 * p_x;

            new_p_x += m1_0 * p_y;
            new_p_y += m1_1 * p_y;
            new_p_z += m1_2 * p_y;

            new_p_x += m2_0 * p_z;
            new_p_y += m2_1 * p_z;
            new_p_z += m2_2 * p_z;

            new_p_x += m3_0 * one;
            new_p_y += m3_1 * one;
            new_p_z += m3_2 * one;

            (new_p_x, new_p_y, new_p_z)
        };

        let (n_x, n_y, n_z) = {
            let matrix_cols0 = normal_matrices[0].to_cols_array();
            let matrix_cols1 = normal_matrices[1].to_cols_array();
            let matrix_cols2 = normal_matrices[2].to_cols_array();
            let matrix_cols3 = normal_matrices[3].to_cols_array();

            // Col 0
            let m0_0 = Vec4::new(
                matrix_cols0[0],
                matrix_cols1[0],
                matrix_cols2[0],
                matrix_cols3[0],
            );
            let m0_1 = Vec4::new(
                matrix_cols0[1],
                matrix_cols1[1],
                matrix_cols2[1],
                matrix_cols3[1],
            );
            let m0_2 = Vec4::new(
                matrix_cols0[2],
                matrix_cols1[2],
                matrix_cols2[2],
                matrix_cols3[2],
            );

            // Col 1
            let m1_0 = Vec4::new(
                matrix_cols0[4],
                matrix_cols1[4],
                matrix_cols2[4],
                matrix_cols3[4],
            );
            let m1_1 = Vec4::new(
                matrix_cols0[5],
                matrix_cols1[5],
                matrix_cols2[5],
                matrix_cols3[5],
            );
            let m1_2 = Vec4::new(
                matrix_cols0[6],
                matrix_cols1[6],
                matrix_cols2[6],
                matrix_cols3[6],
            );

            // Col 2
            let m2_0 = Vec4::new(
                matrix_cols0[8],
                matrix_cols1[8],
                matrix_cols2[8],
                matrix_cols3[8],
            );
            let m2_1 = Vec4::new(
                matrix_cols0[9],
                matrix_cols1[9],
                matrix_cols2[9],
                matrix_cols3[9],
            );
            let m2_2 = Vec4::new(
                matrix_cols0[10],
                matrix_cols1[10],
                matrix_cols2[10],
                matrix_cols3[10],
            );

            let n_x = Vec4::from(hit.normal_x);
            let n_y = Vec4::from(hit.normal_y);
            let n_z = Vec4::from(hit.normal_z);

            let mut new_n_x = m0_0 * n_x;
            let mut new_n_y = m0_1 * n_x;
            let mut new_n_z = m0_2 * n_x;

            new_n_x += m1_0 * n_y;
            new_n_y += m1_1 * n_y;
            new_n_z += m1_2 * n_y;

            new_n_x += m2_0 * n_z;
            new_n_y += m2_1 * n_z;
            new_n_z += m2_2 * n_z;

            (new_n_x, new_n_y, new_n_z)
        };

        HitRecord4 {
            p_x: p_x.into(),
            p_y: p_y.into(),
            p_z: p_z.into(),
            normal_x: n_x.into(),
            normal_y: n_y.into(),
            normal_z: n_z.into(),
            ..hit
        }
    }
}
