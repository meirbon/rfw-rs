use crate::{constants::EPSILON, objects_3d::*};
use l3d::mat::{Material, Texture};
use rtbvh::{Aabb, Bounds};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Plane {
    pos: [f32; 3],
    right: [f32; 3],
    mat_id: u32,
    up: [f32; 3],
    offset: f32,
    forward: [f32; 3],
    dims: [f32; 2],
}

impl Plane {
    pub fn new(pos: [f32; 3], up: [f32; 3], dims: [f32; 2], mat_id: u32) -> Plane {
        let pos = Vec3A::from(pos);
        let up = Vec3A::from(up).normalize();

        let offset = (pos - (pos - up)).length();

        let right = if up[0].abs() >= up[1].abs() {
            Vec3A::new(up.z, 0.0, -up.x) / (up.x * up.x + up.z * up.z).sqrt()
        } else {
            Vec3A::new(0.0, -up.z, up.y) / (up.y * up.y + up.z * up.z).sqrt()
        }
        .normalize();

        let forward = up.cross(right).normalize();

        Plane {
            pos: pos.into(),
            right: right.into(),
            mat_id,
            up: up.into(),
            offset,
            forward: forward.into(),
            dims,
        }
    }

    pub fn get_normal(&self) -> Vec3A {
        self.up.into()
    }

    pub fn get_uv(&self, p: Vec3A) -> Vec2 {
        let center_to_hit = p - Vec3A::from(self.pos);
        let dot_right = center_to_hit.dot(self.right.into());
        let dot_forward = center_to_hit.dot(self.forward.into());

        let u = dot_right % 1.0;
        let v = dot_forward % 1.0;

        let u = if u < 0.0 { 1.0 + u } else { u };
        let v = if v < 0.0 { 1.0 + v } else { v };

        Vec2::new(u, v)
    }
}

// impl Intersect for Plane {
//     fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool {
//         let (origin, direction) = ray.get_vectors::<Vec3A>();
//         let up = Vec3A::from(self.up);

//         let div = up.dot(direction);
//         let t = -(up.dot(origin) + self.offset) / div;

//         if t < t_min || t > t_max {
//             return false;
//         }

//         true
//     }

//     fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
//         let (origin, direction) = ray.get_vectors::<Vec3A>();
//         let up = Vec3A::from(self.up);

//         let div = up.dot(direction);
//         let t = -(up.dot(origin) + self.offset) / div;

//         if t < t_min || t > t_max {
//             return None;
//         }

//         let p = origin + t * direction;

//         Some(HitRecord {
//             normal: self.up.into(),
//             t,
//             p: p.into(),
//             mat_id: self.mat_id,
//             g_normal: self.up.into(),
//             uv: self.get_uv(p).into(),
//         })
//     }

//     fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32> {
//         let (origin, direction) = ray.get_vectors::<Vec3A>();
//         let up = Vec3A::from(self.up);

//         let div = up.dot(direction);
//         let t = -(up.dot(origin) + self.offset) / div;

//         if t < t_min || t > t_max {
//             return None;
//         }

//         Some(t)
//     }

//     fn depth_test(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<(f32, u32)> {
//         if let Some(t) = self.intersect_t(ray, t_min, t_max) {
//             return Some((t, 1));
//         }
//         None
//     }

//     #[allow(clippy::clippy::many_single_char_names)]
//     fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[i32; 4]> {
//         use rfw_math::*;

//         let (origin_x, origin_y, origin_z) = packet.origin_xyz::<Vec4>();
//         let (dir_x, dir_y, dir_z) = packet.direction_xyz::<Vec4>();

//         let up_x = Vec4::splat(self.up[0]);
//         let up_y = Vec4::splat(self.up[1]);
//         let up_z = Vec4::splat(self.up[2]);

//         let div_x = up_x * dir_x;
//         let div_y = up_y * dir_y;
//         let div_z = up_z * dir_z;
//         let div = div_x + div_y + div_z;

//         let offset = Vec4::splat(self.offset);
//         let up_dot_org_x = up_x * origin_x;
//         let up_dot_org_y = up_y * origin_y;
//         let up_dot_org_z = up_z * origin_z;
//         let up_dot_org = up_dot_org_x + up_dot_org_y + up_dot_org_z;
//         let t = -(up_dot_org + offset) / div;

//         let mask = t.cmple(packet.t()) & t.cmpge(Vec4::from(*t_min));
//         let mask = mask.bitmask();
//         if mask == 0 {
//             return None;
//         }

//         let x = if mask & 1 != 0 { 0 } else { -1 };
//         let y = if mask & 2 != 0 { 0 } else { -1 };
//         let z = if mask & 4 != 0 { 0 } else { -1 };
//         let w = if mask & 8 != 0 { 0 } else { -1 };
//         Some([x, y, z, w])
//     }

//     fn get_hit_record(&self, ray: Ray, t: f32, _: u32) -> HitRecord {
//         let (origin, direction) = ray.get_vectors::<Vec3A>();
//         let p = origin + direction * t;

//         HitRecord {
//             normal: self.up.into(),
//             t,
//             p: p.into(),
//             mat_id: self.mat_id,
//             g_normal: self.up.into(),
//             uv: self.get_uv(p).into(),
//         }
//     }

//     fn get_mat_id(&self, _prim_id: i32) -> u32 {
//         self.mat_id as u32
//     }
// }

impl Bounds for Plane {
    fn bounds(&self) -> Aabb {
        let right_offset = self.dims[0] * Vec3A::from(self.right);
        let forward_offset = self.dims[1] * Vec3A::from(self.forward);

        let min = Vec3A::from(self.pos) - right_offset - forward_offset - Vec3A::splat(EPSILON);
        let max = Vec3A::from(self.pos) + right_offset + forward_offset + Vec3A::splat(EPSILON);

        Aabb {
            min: min.into(),
            extra1: 0,
            max: max.into(),
            extra2: 0,
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
struct SerializedPlane {
    pub plane: Plane,
    pub material: Material,
    pub d_tex: Option<Texture>,
    pub n_tex: Option<Texture>,
}

#[cfg(feature = "serde")]
impl<'a> SerializableObject<'a, Plane> for Plane {
    fn serialize_object<S: AsRef<std::path::Path>>(
        &self,
        path: S,
        materials: &crate::MaterialList,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let material = materials.get(self.mat_id as usize).unwrap();
        let mut d_tex: Option<Texture> = None;
        let mut n_tex: Option<Texture> = None;

        if material.diffuse_tex >= 0 {
            d_tex = Some(
                materials
                    .get_texture(material.diffuse_tex as usize)
                    .unwrap()
                    .clone(),
            );
        }

        if material.normal_tex >= 0 {
            n_tex = Some(
                materials
                    .get_texture(material.normal_tex as usize)
                    .unwrap()
                    .clone(),
            );
        }

        let plane = SerializedPlane {
            plane: self.clone(),
            material: material.clone(),
            d_tex,
            n_tex,
        };

        use std::io::Write;
        let encoded: Vec<u8> = bincode::serialize(&plane)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(encoded.as_ref())?;
        Ok(())
    }

    fn deserialize_object<S: AsRef<std::path::Path>>(
        path: S,
        materials: &mut crate::MaterialList,
    ) -> Result<Plane, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let object: SerializedPlane = bincode::deserialize_from(reader)?;

        let plane = object.plane;
        let mut material = object.material;
        if let Some(d_tex) = object.d_tex {
            material.diffuse_tex = materials.push_texture(d_tex) as i16;
        }

        if let Some(n_tex) = object.n_tex {
            material.normal_tex = materials.push_texture(n_tex) as i16;
        }

        materials.push(material);
        Ok(plane)
    }
}

impl ToMesh3D for Plane {
    fn into_mesh_3d(self) -> Mesh3D {
        let normal: [f32; 3] = self.up;
        let position: [f32; 3] = self.pos;
        let (width, height) = (self.dims[0], self.dims[1]);
        let mat_id = self.mat_id;
        Quad3D::new(normal, position, width, height, mat_id).into_mesh_3d()
    }
}
