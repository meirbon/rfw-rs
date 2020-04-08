use glam::*;
use rayon::prelude::*;

use crate::objects::*;
use crate::scene::{PrimID, USE_MBVH};
use bvh::{Bounds, Ray, RayPacket4, AABB, BVH, MBVH};
use serde::{Deserialize, Serialize};

pub trait ToMesh {
    fn into_rt_mesh(self) -> RTMesh;
    fn into_mesh(self) -> RastMesh;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RTMesh {
    triangles: Vec<RTTriangle>,
    materials: Vec<u32>,
    bvh: BVH,
    mbvh: MBVH,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct VertexData {
    pub vertex: [f32; 4],
    pub normal: [f32; 3],
    pub mat_id: u32,
    pub uv: [f32; 2],
}

pub struct VertexBuffer {
    pub count: usize,
    pub size_in_bytes: usize,
    pub buffer: wgpu::Buffer,
    pub bounds: AABB,
}

impl VertexData {
    pub fn zero() -> VertexData {
        VertexData {
            vertex: [0.0, 0.0, 0.0, 1.0],
            normal: [0.0; 3],
            mat_id: 0,
            uv: [0.0; 2],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RastMesh {
    vertices: Vec<VertexData>,
    materials: Vec<u32>,
    bounds: AABB,
}

impl RastMesh {
    pub fn new(
        vertices: &[Vec3],
        normals: &[Vec3],
        uvs: &[Vec2],
        material_ids: &[u32],
    ) -> RastMesh {
        assert_eq!(vertices.len(), normals.len());
        assert_eq!(vertices.len(), uvs.len());
        assert_eq!(uvs.len(), material_ids.len() * 3);
        assert_eq!(vertices.len() % 3, 0);

        let mut bounds = AABB::new();
        let mut vertex_data = vec![VertexData::zero(); vertices.len()];

        for vertex in vertices {
            bounds.grow(*vertex);
        }

        vertex_data.par_iter_mut().enumerate().for_each(|(i, v)| {
            let vertex: [f32; 3] = vertices[i].into();
            let vertex = [vertex[0], vertex[1], vertex[2], 1.0];
            let normal = normals[i].into();
            *v = VertexData {
                vertex,
                normal,
                mat_id: material_ids[i / 3],
                uv: uvs[i].into(),
            };
        });

        RastMesh {
            vertices: vertex_data,
            materials: Vec::from(material_ids),
            bounds,
        }
    }

    pub fn empty() -> RastMesh {
        RastMesh {
            vertices: Vec::new(),
            materials: Vec::new(),
            bounds: AABB::new(),
        }
    }

    #[cfg(feature = "wgpu")]
    pub fn create_wgpu_buffer(&self, device: &wgpu::Device) -> VertexBuffer {
        use wgpu::*;

        let size = self.vertices.len() * std::mem::size_of::<VertexData>();
        let triangle_buffer = device.create_buffer_mapped(&BufferDescriptor {
            label: Some("mesh"),
            size: size as BufferAddress,
            usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
        });

        triangle_buffer.data.copy_from_slice(unsafe {
            std::slice::from_raw_parts(self.vertices.as_ptr() as *const u8, size)
        });

        VertexBuffer {
            count: self.vertices.len(),
            size_in_bytes: size,
            buffer: triangle_buffer.finish(),
            bounds: self.bounds(),
        }
    }
}

impl RTMesh {
    pub fn empty() -> RTMesh {
        RTMesh {
            triangles: Vec::new(),
            materials: Vec::new(),
            bvh: BVH::empty(),
            mbvh: MBVH::empty(),
        }
    }

    pub fn new(vertices: &[Vec3], normals: &[Vec3], uvs: &[Vec2], material_ids: &[u32]) -> RTMesh {
        assert_eq!(vertices.len(), normals.len());
        assert_eq!(vertices.len(), uvs.len());
        assert_eq!(uvs.len(), material_ids.len() * 3);
        assert_eq!(vertices.len() % 3, 0);

        let mut triangles = vec![RTTriangle::zero(); vertices.len() / 3];
        triangles.iter_mut().enumerate().for_each(|(i, triangle)| {
            let i0 = i * 3;
            let i1 = i0 + 1;
            let i2 = i0 + 2;

            let vertex0 = unsafe { *vertices.get_unchecked(i0) };
            let vertex1 = unsafe { *vertices.get_unchecked(i1) };
            let vertex2 = unsafe { *vertices.get_unchecked(i2) };

            let n0 = unsafe { *normals.get_unchecked(i0) };
            let n1 = unsafe { *normals.get_unchecked(i1) };
            let n2 = unsafe { *normals.get_unchecked(i2) };

            let uv0 = unsafe { *uvs.get_unchecked(i0) };
            let uv1 = unsafe { *uvs.get_unchecked(i1) };
            let uv2 = unsafe { *uvs.get_unchecked(i2) };

            let normal = RTTriangle::normal(vertex0, vertex1, vertex2);

            *triangle = RTTriangle {
                vertex0: vertex0.into(),
                u0: uv0.x(),
                vertex1: vertex1.into(),
                u1: uv1.x(),
                vertex2: vertex2.into(),
                u2: uv2.x(),
                normal: normal.into(),
                v0: uv0.y(),
                n0: n0.into(),
                v1: uv1.y(),
                n1: n1.into(),
                v2: uv2.y(),
                n2: n2.into(),
                id: i as i32,
                light_id: -1,
            };
        });

        let aabbs: Vec<AABB> = triangles.iter().map(|t| t.bounds()).collect();
        let bvh = BVH::construct(aabbs.as_slice());
        let mbvh = MBVH::construct(&bvh);

        RTMesh {
            triangles,
            bvh,
            mbvh,
            materials: Vec::from(material_ids),
        }
    }

    pub fn scale(mut self, scaling: f32) -> Self {
        let scaling = Mat4::from_scale(Vec3::new(scaling, scaling, scaling));

        self.triangles.par_iter_mut().for_each(|t| {
            let vertex0 = scaling * Vec4::new(t.vertex0[0], t.vertex0[1], t.vertex0[2], 1.0);
            let vertex1 = scaling * Vec4::new(t.vertex1[0], t.vertex1[1], t.vertex1[2], 1.0);
            let vertex2 = scaling * Vec4::new(t.vertex2[0], t.vertex2[1], t.vertex2[2], 1.0);

            t.vertex0 = vertex0.truncate().into();
            t.vertex1 = vertex1.truncate().into();
            t.vertex2 = vertex2.truncate().into();
        });

        let aabbs: Vec<AABB> = self.triangles.iter().map(|t| t.bounds()).collect();

        self.bvh = BVH::construct(aabbs.as_slice());
        self.mbvh = MBVH::construct(&self.bvh);

        self
    }
}

impl Intersect for RTMesh {
    fn occludes(&self, ray: Ray, t_min: f32, t_max: f32) -> bool {
        let (origin, direction) = ray.into();

        let intersection_test = |i, t_min, t_max| {
            let triangle: &RTTriangle = unsafe { self.triangles.get_unchecked(i) };
            triangle.occludes(ray, t_min, t_max)
        };

        unsafe {
            match USE_MBVH {
                true => self.mbvh.occludes(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
                _ => self.bvh.occludes(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
            }
        }
    }

    fn intersect(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<HitRecord> {
        let (origin, direction) = ray.into();

        let intersection_test = |i, t_min, t_max| {
            let triangle: &RTTriangle = unsafe { self.triangles.get_unchecked(i) };
            if let Some(mut hit) = triangle.intersect(ray, t_min, t_max) {
                hit.mat_id = self.materials[i];
                return Some((hit.t, hit));
            }
            None
        };

        unsafe {
            match USE_MBVH {
                true => self.mbvh.traverse(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
                _ => self.bvh.traverse(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
            }
        }
    }

    fn intersect_t(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<f32> {
        let (origin, direction) = ray.into();

        let intersection_test = |i, t_min, t_max| {
            let triangle: &RTTriangle = unsafe { self.triangles.get_unchecked(i) };
            if let Some(t) = triangle.intersect_t(ray, t_min, t_max) {
                return Some(t);
            }
            None
        };

        unsafe {
            match USE_MBVH {
                true => self.mbvh.traverse_t(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
                _ => self.bvh.traverse_t(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
            }
        }
    }

    fn depth_test(&self, ray: Ray, t_min: f32, t_max: f32) -> Option<(f32, u32)> {
        let (origin, direction) = ray.into();

        let intersection_test = |i, t_min, t_max| -> Option<(f32, u32)> {
            let triangle: &RTTriangle = unsafe { self.triangles.get_unchecked(i) };
            triangle.depth_test(ray, t_min, t_max)
        };

        let hit = unsafe {
            match USE_MBVH {
                true => self.mbvh.depth_test(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
                _ => self.bvh.depth_test(
                    origin.as_ref(),
                    direction.as_ref(),
                    t_min,
                    t_max,
                    intersection_test,
                ),
            }
        };

        Some(hit)
    }

    fn intersect4(&self, packet: &mut RayPacket4, t_min: &[f32; 4]) -> Option<[PrimID; 4]> {
        let mut prim_id = [-1 as PrimID; 4];
        let mut valid = false;
        let intersection_test = |i: usize, packet: &mut RayPacket4| {
            let triangle: &RTTriangle = unsafe { self.triangles.get_unchecked(i) };
            if let Some(hit) = triangle.intersect4(packet, t_min) {
                valid = true;
                for i in 0..4 {
                    if hit[i] >= 0 {
                        prim_id[i] = hit[i];
                    }
                }
            }
        };

        unsafe {
            match USE_MBVH {
                true => self.mbvh.traverse4(packet, intersection_test),
                _ => self.bvh.traverse4(packet, intersection_test),
            }
        };

        if valid {
            Some(prim_id)
        } else {
            None
        }
    }

    fn get_hit_record(&self, ray: Ray, t: f32, hit_data: u32) -> HitRecord {
        self.triangles[hit_data as usize].get_hit_record(ray, t, hit_data)
    }
}

impl Bounds for RTMesh {
    fn bounds(&self) -> AABB {
        self.bvh.nodes[0].bounds.clone()
    }
}

impl Bounds for RastMesh {
    fn bounds(&self) -> AABB {
        self.bounds.clone()
    }
}

impl<'a> SerializableObject<'a, RTMesh> for RTMesh {
    fn serialize<S: AsRef<std::path::Path>>(&self, path: S) -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        let encoded: Vec<u8> = bincode::serialize(self)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(encoded.as_ref())?;
        Ok(())
    }

    fn deserialize<S: AsRef<std::path::Path>>(path: S) -> Result<RTMesh, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let object: Self = bincode::deserialize_from(reader)?;
        Ok(object)
    }
}

impl<'a> SerializableObject<'a, RastMesh> for RastMesh {
    fn serialize<S: AsRef<std::path::Path>>(&self, path: S) -> Result<(), Box<dyn std::error::Error>> {
        use std::io::Write;
        let encoded: Vec<u8> = bincode::serialize(self)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(encoded.as_ref())?;
        Ok(())
    }

    fn deserialize<S: AsRef<std::path::Path>>(path: S) -> Result<RastMesh, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let object: Self = bincode::deserialize_from(reader)?;
        Ok(object)
    }
}