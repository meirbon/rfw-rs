use crate::objects::*;
use crate::PrimID;
use l3d::mat::{Material, Texture};
use rtbvh::{Bounds, Ray, RayPacket4, AABB};
use std::collections::HashMap;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub enum Quality {
    /// Generates a sphere (Icosahedron) consisting out of 20 triangles,
    Icosahedron = 0,
    /// Generates a sphere consisting out of 80 triangles,
    Low = 1,
    /// Generates a sphere consisting out of 320 triangles,
    Medium = 2,
    /// Generates a sphere consisting out of 1280 triangles,
    High = 3,
    /// Generates a sphere consisting out of 5120 triangles,
    VeryHigh = 4,
    /// Generates a sphere consisting out of 20480 triangles,
    Perfect = 5,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
pub struct Sphere {
    pos: [f32; 3],
    radius2: f32,
    pub mat_id: u32,
    pub quality: Quality,
}

#[allow(dead_code)]
impl Sphere {
    /// Creates new sphere with specified radius, material and segments.
    /// Segments is only used when this sphere is procedurally transformed into a mesh.
    pub fn new<T: Into<[f32; 3]>>(pos: T, radius: f32, mat_id: u32) -> Sphere {
        Sphere {
            pos: pos.into(),
            radius2: radius * radius,
            mat_id,
            quality: Quality::High,
        }
    }

    pub fn with_quality(mut self, quality: Quality) -> Self {
        self.quality = quality;
        self
    }

    pub fn normal(&self, p: Vec3) -> Vec3 {
        (p - self.pos.into()).normalize()
    }

    pub fn get_uv(&self, n: Vec3) -> Vec2 {
        let u = n.x.atan2(n.z) * (1.0 / (2.0 * std::f32::consts::PI)) + 0.5;
        let v = n.y * 0.5 + 0.5;
        vec2(u, v)
    }
}

impl Intersect for Sphere {
    fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool {
        let (origin, direction) = ray.get_vectors::<Vec3>();

        let a = direction.dot(direction);
        let r_pos = origin - self.pos.into();

        let b = (direction * 2.0).dot(r_pos);
        let r_pos2 = r_pos.dot(r_pos);
        let c = r_pos2 - self.radius2;

        let d: f32 = (b * b) - (4.0 * a * c);

        if d < 0.0 {
            return false;
        }

        let div_2a = 1.0 / (2.0 * a);
        let sqrt_d = if d > 0.0 { d.sqrt() } else { 0.0 };

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;
        let t = if t1 > t_min && t1 < t2 { t1 } else { t2 };

        t < t_max
    }

    fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let (origin, direction) = ray.get_vectors::<Vec3>();

        let a = direction.dot(direction);
        let r_pos = origin - self.pos.into();

        let b = (direction * 2.0).dot(r_pos);
        let r_pos2 = r_pos.dot(r_pos);
        let c = r_pos2 - self.radius2;

        let d: f32 = (b * b) - (4.0 * a * c);

        if d < 0.0 {
            return None;
        }

        let div_2a = 1.0 / (2.0 * a);

        let sqrt_d = if d > 0.0 { d.sqrt() } else { 0.0 };

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;

        let t = if t1 > t_min && t1 < t2 { t1 } else { t2 };
        if t <= t_min || t >= t_max {
            return None;
        }

        let p = origin + direction * t;
        let normal = self.normal(p);
        let uv = self.get_uv(normal);

        Some(HitRecord {
            g_normal: normal.into(),
            normal: normal.into(),
            t,
            p: p.into(),
            mat_id: self.mat_id,
            uv: uv.into(),
        })
    }

    fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32> {
        let (origin, direction) = ray.get_vectors::<Vec3>();

        let a = direction.dot(direction);
        let r_pos = origin - self.pos.into();

        let b = (direction * 2.0).dot(r_pos);
        let r_pos2 = r_pos.dot(r_pos);
        let c = r_pos2 - self.radius2;

        let d: f32 = (b * b) - (4.0 * a * c);

        if d < 0.0 {
            return None;
        }

        let div_2a = 1.0 / (2.0 * a);

        let sqrt_d = if d > 0.0 { d.sqrt() } else { 0.0 };

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;

        let t = if t1 > t_min && t1 < t2 { t1 } else { t2 };

        if t <= t_min || t >= t_max {
            None
        } else {
            Some(t)
        }
    }

    fn depth_test(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<(f32, u32)> {
        if let Some(t) = self.intersect_t(ray, t_min, t_max) {
            return Some((t, 1));
        }
        None
    }

    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[PrimID; 4]> {
        let origin_x = Vec4::from(packet.origin_x);
        let origin_y = Vec4::from(packet.origin_y);
        let origin_z = Vec4::from(packet.origin_z);

        let direction_x = Vec4::from(packet.direction_x);
        let direction_y = Vec4::from(packet.direction_y);
        let direction_z = Vec4::from(packet.direction_z);

        let a_x: Vec4 = direction_x * direction_x;
        let a_y: Vec4 = direction_y * direction_y;
        let a_z: Vec4 = direction_z * direction_z;
        let a: Vec4 = a_x + a_y + a_z;

        let r_pos_x: Vec4 = origin_x - Vec4::from([self.pos[0]; 4]);
        let r_pos_y: Vec4 = origin_y - Vec4::from([self.pos[1]; 4]);
        let r_pos_z: Vec4 = origin_z - Vec4::from([self.pos[2]; 4]);

        let b_x: Vec4 = direction_x * 2.0 * r_pos_x;
        let b_y: Vec4 = direction_y * 2.0 * r_pos_y;
        let b_z: Vec4 = direction_z * 2.0 * r_pos_z;
        let b: Vec4 = b_x + b_y + b_z;

        let r_pos2_x: Vec4 = r_pos_x * r_pos_x;
        let r_pos2_y: Vec4 = r_pos_y * r_pos_y;
        let r_pos2_z: Vec4 = r_pos_z * r_pos_z;
        let r_pos2: Vec4 = r_pos2_x + r_pos2_y + r_pos2_z;

        let radius: Vec4 = Vec4::from([self.radius2; 4]);
        let c: Vec4 = r_pos2 - radius;
        let d: Vec4 = b * b - 4.0 * a * c;

        let t_min = Vec4::from(*t_min);

        let mask = d.cmpge(Vec4::zero());
        // No hits
        if mask.bitmask() == 0 {
            return None;
        }

        let div_2a = Vec4::one() / (2.0 * a);

        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        let sqrt_d = unsafe {
            use std::arch::x86_64::_mm_sqrt_ps;
            Vec4::from(_mm_sqrt_ps(d.into())).max(Vec4::zero())
        };
        #[cfg(any(
            all(not(target_arch = "x86_64"), not(target_arch = "x86")),
            target_arch = "wasm32-unknown-unknown"
        ))]
        let sqrt_d = Vec4::new(
            (d[0].sqrt()).max(0.0),
            (d[1].sqrt()).max(0.0),
            (d[2].sqrt()).max(0.0),
            (d[3].sqrt()).max(0.0),
        );

        let t1 = ((-b) + sqrt_d) * div_2a;
        let t2 = ((-b) - sqrt_d) * div_2a;
        let pick_t1 = t1.cmpgt(t_min) & t1.cmplt(t2);
        let t = pick_t1.select(t1, t2);
        let mask = mask & (t.cmpgt(t_min) & t.cmplt(packet.t.into()));
        let bitmask = mask.bitmask();
        if bitmask == 0 {
            return None;
        }
        packet.t = mask.select(t, packet.t.into()).into();

        let x = if bitmask & 1 != 0 { 0 } else { -1 };
        let y = if bitmask & 2 != 0 { 0 } else { -1 };
        let z = if bitmask & 4 != 0 { 0 } else { -1 };
        let w = if bitmask & 8 != 0 { 0 } else { -1 };
        Some([x, y, z, w])
    }

    fn get_hit_record(&self, ray: Ray, t: f32, _: u32) -> HitRecord {
        let (origin, direction) = ray.get_vectors::<Vec3>();

        let p = origin + direction * t;
        let normal = self.normal(p);
        let uv = self.get_uv(normal);

        HitRecord {
            g_normal: normal.into(),
            normal: normal.into(),
            t,
            p: p.into(),
            mat_id: self.mat_id,
            uv: uv.into(),
        }
    }

    fn get_mat_id(&self, _prim_id: PrimID) -> u32 {
        self.mat_id
    }
}

impl Bounds for Sphere {
    fn bounds(&self) -> AABB {
        let radius = self.radius2.sqrt() + crate::constants::AABB_EPSILON;
        let min: [f32; 3] = [
            self.pos[0] - radius,
            self.pos[1] - radius,
            self.pos[2] - radius,
        ];
        let max: [f32; 3] = [
            self.pos[0] + radius,
            self.pos[1] + radius,
            self.pos[2] + radius,
        ];
        AABB { min, max }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
struct SerializedSphere {
    pub sphere: Sphere,
    pub material: Material,
    pub d_tex: Option<Texture>,
    pub n_tex: Option<Texture>,
}

#[cfg(feature = "serde")]
impl<'a> SerializableObject<'a, Sphere> for Sphere {
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

        let sphere = SerializedSphere {
            sphere: self.clone(),
            material: material.clone(),
            d_tex,
            n_tex,
        };

        use std::io::Write;
        let encoded: Vec<u8> = bincode::serialize(&sphere)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(encoded.as_ref())?;
        Ok(())
    }

    fn deserialize_object<S: AsRef<std::path::Path>>(
        path: S,
        materials: &mut crate::MaterialList,
    ) -> Result<Sphere, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let object: SerializedSphere = bincode::deserialize_from(reader)?;

        let sphere = object.sphere;
        let mut material = object.material;
        if let Some(d_tex) = object.d_tex {
            material.diffuse_tex = materials.push_texture(d_tex) as i16;
        }

        if let Some(n_tex) = object.n_tex {
            material.normal_tex = materials.push_texture(n_tex) as i16;
        }

        materials.push(material);
        Ok(sphere)
    }
}

impl ToMesh for Sphere {
    fn into_mesh(self) -> Mesh3D {
        use std::f32::consts::PI;

        let mut faces: Vec<[u32; 3]> = Vec::with_capacity(20);
        let mut vertices: Vec<Vec3> = Vec::with_capacity(12);
        let mut normals: Vec<Vec3> = Vec::with_capacity(12);
        let mut uvs: Vec<Vec2> = Vec::with_capacity(12);

        let s = ((5.0 - 5.0_f32.sqrt()) / 10.0).sqrt();
        let t: f32 = ((5.0 + 5.0_f32.sqrt()) / 10.0).sqrt();

        // First, create an Icosahedron
        let add_vertex = |v: Vec3,
                          vertices: &mut Vec<Vec3>,
                          normals: &mut Vec<Vec3>,
                          uvs: &mut Vec<Vec2>|
         -> usize {
            let v = v.normalize();
            normals.push(v);
            vertices.push(v);

            let u = v.x.atan2(v.z) / (2.0 * PI) + 0.5;
            let v = v.y * 0.5 + 0.5;
            uvs.push(Vec2::new(u, v));
            vertices.len() - 1
        };

        let mut index_hash: HashMap<usize, usize> = HashMap::new();
        let mut get_middle_point = |p1: usize,
                                    p2: usize,
                                    vertices: &mut Vec<Vec3>,
                                    normals: &mut Vec<Vec3>,
                                    uvs: &mut Vec<Vec2>|
         -> usize {
            let is_smaller = p1 < p2;
            let (smaller_idx, greater_idx) = if is_smaller { (p1, p2) } else { (p2, p1) };
            let key = smaller_idx.overflowing_shl(32).0 + greater_idx;

            if let Some(idx) = index_hash.get(&key) {
                *idx
            } else {
                let p1 = vertices[p1];
                let p2 = vertices[p2];
                let middle = (p1 + p2) / 2.0;

                let idx = add_vertex(middle, vertices, normals, uvs);
                index_hash.insert(key, idx);
                idx
            }
        };

        add_vertex(Vec3::new(-s, t, 0.0), &mut vertices, &mut normals, &mut uvs);
        add_vertex(Vec3::new(s, t, 0.0), &mut vertices, &mut normals, &mut uvs);
        add_vertex(
            Vec3::new(-s, -t, 0.0),
            &mut vertices,
            &mut normals,
            &mut uvs,
        );
        add_vertex(Vec3::new(s, -t, 0.0), &mut vertices, &mut normals, &mut uvs);

        add_vertex(Vec3::new(0.0, -s, t), &mut vertices, &mut normals, &mut uvs);
        add_vertex(Vec3::new(0.0, s, t), &mut vertices, &mut normals, &mut uvs);
        add_vertex(
            Vec3::new(0.0, -s, -t),
            &mut vertices,
            &mut normals,
            &mut uvs,
        );
        add_vertex(Vec3::new(0.0, s, -t), &mut vertices, &mut normals, &mut uvs);

        add_vertex(Vec3::new(t, 0.0, -s), &mut vertices, &mut normals, &mut uvs);
        add_vertex(Vec3::new(t, 0.0, s), &mut vertices, &mut normals, &mut uvs);
        add_vertex(
            Vec3::new(-t, 0.0, -s),
            &mut vertices,
            &mut normals,
            &mut uvs,
        );
        add_vertex(Vec3::new(-t, 0.0, s), &mut vertices, &mut normals, &mut uvs);

        // 5 faces around point 0
        faces.push([0, 11, 5]);
        faces.push([0, 5, 1]);
        faces.push([0, 1, 7]);
        faces.push([0, 7, 10]);
        faces.push([0, 10, 11]);

        // 5 adjacent faces
        faces.push([1, 5, 9]);
        faces.push([5, 11, 4]);
        faces.push([11, 10, 2]);
        faces.push([10, 7, 6]);
        faces.push([7, 1, 8]);

        // 5 faces around point 3
        faces.push([3, 9, 4]);
        faces.push([3, 4, 2]);
        faces.push([3, 2, 6]);
        faces.push([3, 6, 8]);
        faces.push([3, 8, 9]);

        // 5 adjacent faces
        faces.push([4, 9, 5]);
        faces.push([2, 4, 11]);
        faces.push([6, 2, 10]);
        faces.push([8, 6, 7]);
        faces.push([9, 8, 1]);

        let quality = self.quality as usize;
        for _ in 0..quality {
            let mut new_faces = Vec::with_capacity(faces.len() * 4);
            for face in faces.iter() {
                let i0 = face[0] as usize;
                let i1 = face[1] as usize;
                let i2 = face[2] as usize;

                // replace triangle by 4 triangles
                let a = get_middle_point(i0, i1, &mut vertices, &mut normals, &mut uvs) as u32;
                let b = get_middle_point(i1, i2, &mut vertices, &mut normals, &mut uvs) as u32;
                let c = get_middle_point(i2, i0, &mut vertices, &mut normals, &mut uvs) as u32;

                new_faces.push([i0 as u32, a, c]);
                new_faces.push([i1 as u32, b, a]);
                new_faces.push([i2 as u32, c, b]);
                new_faces.push([a, b, c]);
            }
            faces = new_faces;
        }

        let material_ids = vec![self.mat_id; faces.len()];

        let origin = Vec3::from(self.pos);
        let radius = self.radius2.sqrt();
        let radius = Vec3::splat(radius);

        use rayon::prelude::*;
        vertices.par_iter_mut().for_each(|v| {
            *v = origin + (*v) * radius;
        });

        Mesh3D::new_indexed(
            faces,
            vertices,
            normals,
            Vec::new(),
            Vec::new(),
            uvs,
            material_ids,
            Some(String::from("sphere")),
        )
    }
}
