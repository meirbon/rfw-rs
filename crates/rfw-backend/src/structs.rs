use rayon::prelude::*;
use rfw_math::*;
use rtbvh::*;
use std::{fmt::Debug, write};

#[derive(Debug, Copy, Clone)]
pub struct SkinData<'a> {
    pub name: &'a str,
    pub inverse_bind_matrices: &'a [Mat4],
    pub joint_matrices: &'a [Mat4],
}

#[derive(Debug, Copy, Clone)]
pub struct InstancesData2D<'a> {
    pub matrices: &'a [Mat4],
}

impl InstancesData2D<'_> {
    pub fn len(&self) -> usize {
        self.matrices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matrices.is_empty()
    }
}

bitflags::bitflags! {
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[repr(transparent)]
    pub struct InstanceFlags3D: u32 {
        const TRANSFORMED = 1;
    }
}

impl Default for InstanceFlags3D {
    fn default() -> Self {
        Self::all()
    }
}

#[derive(Debug, Copy, Clone)]
pub struct InstancesData3D<'a> {
    pub matrices: &'a [Mat4],
    pub skin_ids: &'a [SkinID],
    pub flags: &'a [InstanceFlags3D],
    pub local_aabb: Aabb,
}

impl InstancesData3D<'_> {
    pub fn len(&self) -> usize {
        debug_assert_eq!(self.matrices.len(), self.skin_ids.len());
        self.matrices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.matrices.is_empty()
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct MeshId2D(pub i32);

impl Default for MeshId2D {
    fn default() -> Self {
        Self::INVALID
    }
}

impl std::fmt::Display for MeshId2D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MeshId3D({})", self.0)
    }
}

impl MeshId2D {
    pub const INVALID: Self = MeshId2D(-1);

    pub fn is_valid(&self) -> bool {
        self.0 >= 0
    }

    pub fn as_index(&self) -> Option<usize> {
        if self.0 >= 0 {
            Some(self.0 as usize)
        } else {
            None
        }
    }
}

impl From<MeshId2D> for usize {
    fn from(val: MeshId2D) -> Self {
        val.0 as usize
    }
}

impl From<usize> for MeshId2D {
    fn from(i: usize) -> Self {
        MeshId2D(i as i32)
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct MeshId3D(pub i32);

impl Default for MeshId3D {
    fn default() -> Self {
        Self::INVALID
    }
}

impl std::fmt::Display for MeshId3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MeshId3D({})", self.0)
    }
}

impl MeshId3D {
    pub const INVALID: Self = MeshId3D(-1);

    pub fn is_valid(&self) -> bool {
        self.0 >= 0
    }

    pub fn as_index(&self) -> Option<usize> {
        if self.0 >= 0 {
            Some(self.0 as usize)
        } else {
            None
        }
    }
}

impl From<MeshId3D> for usize {
    fn from(val: MeshId3D) -> Self {
        val.0 as usize
    }
}

impl From<usize> for MeshId3D {
    fn from(i: usize) -> Self {
        MeshId3D(i as i32)
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct SkinID(pub i32);

impl Default for SkinID {
    fn default() -> Self {
        Self::INVALID
    }
}

impl std::fmt::Display for SkinID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SkinID {
    pub const INVALID: Self = SkinID(-1);

    pub fn is_valid(&self) -> bool {
        self.0 >= 0
    }

    pub fn as_index(&self) -> Option<usize> {
        if self.0 >= 0 {
            Some(self.0 as usize)
        } else {
            None
        }
    }
}

impl From<usize> for SkinID {
    fn from(i: usize) -> Self {
        SkinID(i as i32)
    }
}

impl From<SkinID> for usize {
    fn from(val: SkinID) -> Self {
        val.0 as usize
    }
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(C)]
pub enum DataFormat {
    BGRA8 = 0,
    RGBA8 = 1,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(C)]
pub struct TextureData<'a> {
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub bytes: &'a [u8],
    pub format: DataFormat,
}

impl TextureData<'_> {
    pub fn offset_for_level(&self, mip_level: usize) -> usize {
        assert!(mip_level <= self.mip_levels as usize);
        let mut offset = 0;
        for i in 0..mip_level {
            let (w, h) = self.mip_level_width_height(i);
            offset += w * h;
        }
        offset
    }

    pub fn mip_level_width(&self, mip_level: usize) -> usize {
        let mut w = self.width as usize;
        for _ in 0..mip_level {
            w >>= 1;
        }
        w
    }

    pub fn mip_level_height(&self, mip_level: usize) -> usize {
        let mut h = self.height as usize;
        for _ in 0..mip_level {
            h >>= 1;
        }
        h
    }

    pub fn mip_level_width_height(&self, mip_level: usize) -> (usize, usize) {
        let mut w = self.width as usize;
        let mut h = self.height as usize;

        if mip_level == 0 {
            return (w, h);
        }

        for _ in 0..mip_level {
            w >>= 1;
            h >>= 1
        }

        (w, h)
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[repr(C)]
pub struct Vertex3D {
    pub vertex: Vec4,
    // 16
    pub normal: Vec3,
    // 28
    pub mat_id: u32,
    // 32
    pub uv: Vec2,
    pub pad0: f32,
    pub pad1: f32,
    // 40
    pub tangent: Vec4,
    // 56
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[repr(C)]
pub struct JointData {
    pub joint: [u32; 4],
    pub weight: Vec4,
}

impl From<JointData> for ([u32; 4], Vec4) {
    fn from(val: JointData) -> Self {
        (val.joint, val.weight)
    }
}

impl<T: Into<[f32; 4]>> From<([u32; 4], T)> for JointData {
    fn from(data: ([u32; 4], T)) -> Self {
        Self {
            joint: data.0,
            weight: Vec4::from(data.1.into()),
        }
    }
}

impl<T: Into<[f32; 4]>> From<([u16; 4], T)> for JointData {
    fn from(data: ([u16; 4], T)) -> Self {
        Self {
            joint: [
                data.0[0] as u32,
                data.0[1] as u32,
                data.0[2] as u32,
                data.0[3] as u32,
            ],
            weight: Vec4::from(data.1.into()),
        }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct VertexMesh {
    pub bounds: Aabb,
    pub first: u32,
    pub last: u32,
    pub mat_id: u32,
    pub padding: u32,
}

bitflags::bitflags! {
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[repr(transparent)]
    pub struct Mesh3dFlags: u32 {
        const SHADOW_CASTER = 1;
        const ALLOW_SKINNING = 2;
    }
}

impl Default for Mesh3dFlags {
    fn default() -> Self {
        Self::SHADOW_CASTER | Self::ALLOW_SKINNING
    }
}

#[derive(Debug, Clone)]
pub struct MeshData3D<'a> {
    pub name: &'a str,
    pub bounds: Aabb,
    pub vertices: &'a [Vertex3D],
    pub triangles: &'a [RTTriangle],
    pub ranges: &'a [VertexMesh],
    pub skin_data: &'a [JointData],
    pub flags: Mesh3dFlags,
}

impl<'a> MeshData3D<'a> {
    pub fn apply_skin_vertices(&self, joint_matrices: &[Mat4]) -> SkinnedMesh3D {
        SkinnedMesh3D::apply(self.vertices, self.skin_data, self.ranges, joint_matrices)
    }

    pub fn apply_skin_triangles(&self, joint_matrices: &[Mat4]) -> SkinnedTriangles3D {
        SkinnedTriangles3D::apply(self.triangles, self.skin_data, joint_matrices)
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
#[repr(C)]
pub struct Vertex2D {
    pub vertex: Vec3,
    pub tex: u32,
    pub uv: Vec2,
    pub color: Vec4,
}

#[derive(Debug, Clone)]
pub struct MeshData2D<'a> {
    pub vertices: &'a [Vertex2D],
    pub tex_id: Option<usize>,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone)]
#[repr(C)]
pub struct DeviceMaterial {
    pub color: [f32; 4],
    // 16
    pub absorption: [f32; 4],
    // 32
    pub specular: [f32; 4],
    // 48
    pub parameters: [u32; 4], // 64

    pub flags: u32,
    // 68
    pub diffuse_map: i32,
    // 72
    pub normal_map: i32,
    // 76
    pub metallic_roughness_map: i32, // 80

    pub emissive_map: i32,
    // 84
    pub sheen_map: i32,
    // 88
    pub _dummy: [i32; 2], // 96
}

impl Default for DeviceMaterial {
    fn default() -> Self {
        Self {
            color: [0.0; 4],
            absorption: [0.0; 4],
            specular: [0.0; 4],
            parameters: [0; 4],
            flags: 0,
            diffuse_map: -1,
            normal_map: -1,
            metallic_roughness_map: -1,
            emissive_map: -1,
            sheen_map: -1,
            _dummy: [0; 2],
        }
    }
}

impl DeviceMaterial {
    pub fn get_metallic(&self) -> f32 {
        (self.parameters[0] & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_subsurface(&self) -> f32 {
        ((self.parameters[0].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_specular_f(&self) -> f32 {
        ((self.parameters[0].overflowing_shr(16)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_roughness(&self) -> f32 {
        ((self.parameters[0].overflowing_shr(24)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_specular_tint(&self) -> f32 {
        (self.parameters[1] & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_anisotropic(&self) -> f32 {
        ((self.parameters[1].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_sheen(&self) -> f32 {
        ((self.parameters[1].overflowing_shr(16)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_sheen_tint(&self) -> f32 {
        ((self.parameters[1].overflowing_shr(24)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_clearcoat(&self) -> f32 {
        (self.parameters[2] & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_clearcoat_gloss(&self) -> f32 {
        ((self.parameters[2].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }
    pub fn get_transmission(&self) -> f32 {
        ((self.parameters[2].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_eta(&self) -> f32 {
        ((self.parameters[2].overflowing_shr(24)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_custom0(&self) -> f32 {
        (self.parameters[3] & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_custom1(&self) -> f32 {
        ((self.parameters[3].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_custom2(&self) -> f32 {
        ((self.parameters[3].overflowing_shr(8)).0 & 255) as f32 * 1.0 / 255.0
    }

    pub fn get_custom3(&self) -> f32 {
        ((self.parameters[3].overflowing_shr(24)).0 & 255) as f32 * 1.0 / 255.0
    }
}

#[derive(Default, Debug, Copy, Clone)]
pub struct CameraView2D {
    pub matrix: Mat4,
}

#[derive(Default, Debug, Copy, Clone)]
#[repr(C)]
pub struct CameraView3D {
    pub pos: Vec3,
    // 12
    pub right: Vec3,
    // 24
    pub up: Vec3,
    // 36
    pub p1: Vec3,
    //48
    pub direction: Vec3,
    // 60
    pub lens_size: f32,
    // 64
    pub spread_angle: f32,
    pub epsilon: f32,
    pub inv_width: f32,
    pub inv_height: f32,
    // 80
    pub near_plane: f32,
    pub far_plane: f32,
    pub aspect_ratio: f32,
    // FOV in radians
    pub fov: f32,
    // 96
    pub custom0: Vec4,
    // 112
    pub custom1: Vec4,
    // 128
    // add dummy to align to 128
}

#[allow(dead_code)]
impl CameraView3D {
    pub fn generate_lens_ray(&self, x: u32, y: u32, r0: f32, r1: f32, r2: f32, r3: f32) -> Ray {
        let blade = (r0 * 9.0).round();
        let r2 = (r2 - blade * (1.0 / 9.0)) * 9.0;
        let pi_over_4dot5 = std::f32::consts::PI / 4.5;
        let blade_param = blade * pi_over_4dot5;

        let (x1, y1) = blade_param.sin_cos();
        let blade_param = (blade + 1.0) * pi_over_4dot5;
        let (x2, y2) = blade_param.sin_cos();

        let (r2, r3) = {
            if (r2 + r3) > 1.0 {
                (1.0 - r2, 1.0 - r3)
            } else {
                (r2, r3)
            }
        };

        let xr = x1 * r2 + x2 * r3;
        let yr = y1 * r2 + y2 * r3;

        let origin = self.pos + self.lens_size * (self.right * xr + self.up * yr);
        let u = (x as f32 + r0) * self.inv_width;
        let v = (y as f32 + r1) * self.inv_height;
        let point_on_pixel = self.p1 + u * self.right + v * self.up;
        let direction = (point_on_pixel - origin).normalize();

        Ray::new(origin, direction)
    }

    pub fn generate_ray(&self, x: u32, y: u32) -> Ray {
        let u = x as f32 * self.inv_width;
        let v = y as f32 * self.inv_height;
        let point_on_pixel = self.p1 + u * self.right + v * self.up;
        let direction = (point_on_pixel - self.pos).normalize();

        Ray::new(self.pos, direction)
    }

    pub fn generate_lens_ray4(
        &self,
        x: [u32; 4],
        y: [u32; 4],
        r0: [f32; 4],
        r1: [f32; 4],
        r2: [f32; 4],
        r3: [f32; 4],
    ) -> RayPacket4 {
        let r0 = Vec4::from(r0);
        let r1 = Vec4::from(r1);
        let r2 = Vec4::from(r2);
        let r3 = Vec4::from(r3);

        let blade: Vec4 = r0 * Vec4::splat(9.0);
        let r2: Vec4 = (r2 - blade * (1.0 / 9.0)) * 9.0;
        let pi_over_4dot5: Vec4 = Vec4::splat(std::f32::consts::PI / 4.5);
        let blade_param: Vec4 = blade * pi_over_4dot5;

        // #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        let (x1, y1) = {
            let mut x = [0.0_f32; 4];
            let mut y = [0.0_f32; 4];
            for i in 0..4 {
                let (cos, sin) = blade_param[i].sin_cos();
                x[i] = cos;
                y[i] = sin;
            }

            (Vec4::from(x), Vec4::from(y))
        };

        let blade_param = (blade + Vec4::ONE) * pi_over_4dot5;
        let (x2, y2) = {
            let mut x = [0.0_f32; 4];
            let mut y = [0.0_f32; 4];
            for i in 0..4 {
                let (cos, sin) = blade_param[i].sin_cos();
                x[i] = cos;
                y[i] = sin;
            }

            (Vec4::from(x), Vec4::from(y))
        };

        let (r2, r3) = {
            let mask: BVec4A = (r2 + r3).cmpgt(Vec4::ONE);
            (
                Vec4::select(mask, Vec4::ONE - r2, r2),
                Vec4::select(mask, Vec4::ONE - r3, r3),
            )
        };

        let x = UVec4::from(x).as_f32();
        let y = UVec4::from(y).as_f32();

        let xr = x1 * r2 + x2 * r2;
        let yr = y1 * r2 + y2 * r3;

        let u = (x + r0) * self.inv_width;
        let v = (y + r1) * self.inv_height;

        let p_x = Vec4::from([self.p1[0]; 4]) + u * self.p1[0] + v * self.up[0];
        let p_y = Vec4::from([self.p1[1]; 4]) + u * self.p1[1] + v * self.up[1];
        let p_z = Vec4::from([self.p1[2]; 4]) + u * self.p1[2] + v * self.up[2];

        let direction_x = p_x - Vec4::from([self.pos[0]; 4]);
        let direction_y = p_y - Vec4::from([self.pos[1]; 4]);
        let direction_z = p_z - Vec4::from([self.pos[2]; 4]);

        let length_squared = direction_x * direction_x;
        let length_squared = length_squared + direction_y * direction_y;
        let length_squared = length_squared + direction_z * direction_z;

        let length = vec4_sqrt(length_squared);

        let inv_length = Vec4::ONE / length;

        let direction_x = direction_x * inv_length;
        let direction_y = direction_y * inv_length;
        let direction_z = direction_z * inv_length;

        let origin_x = Vec4::splat(self.pos[0]);
        let origin_y = Vec4::splat(self.pos[1]);
        let origin_z = Vec4::splat(self.pos[2]);

        let lens_size = Vec4::splat(self.lens_size);
        let right_x = Vec4::splat(self.p1[0]);
        let right_y = Vec4::splat(self.p1[1]);
        let right_z = Vec4::splat(self.p1[2]);
        let up_x = Vec4::splat(self.up[0]);
        let up_y = Vec4::splat(self.up[1]);
        let up_z = Vec4::splat(self.up[2]);

        let origin_x = origin_x + lens_size * (right_x * xr + up_x * yr);
        let origin_y = origin_y + lens_size * (right_y * xr + up_y * yr);
        let origin_z = origin_z + lens_size * (right_z * xr + up_z * yr);

        RayPacket4 {
            origin_x,
            origin_y,
            origin_z,
            direction_x,
            direction_y,
            direction_z,
            t: [1e34_f32; 4].into(),
            inv_direction_x: 1.0 / direction_x,
            inv_direction_y: 1.0 / direction_y,
            inv_direction_z: 1.0 / direction_z,
        }
    }

    pub fn generate_ray4(&self, x: [u32; 4], y: [u32; 4]) -> RayPacket4 {
        let x = UVec4::from(x).as_f32();
        let y = UVec4::from(y).as_f32();

        let u = x * self.inv_width;
        let v = y * self.inv_height;

        let p_x = Vec4::from([self.p1[0]; 4]) + u * self.p1[0] + v * self.up[0];
        let p_y = Vec4::from([self.p1[1]; 4]) + u * self.p1[1] + v * self.up[1];
        let p_z = Vec4::from([self.p1[2]; 4]) + u * self.p1[2] + v * self.up[2];

        let direction_x = p_x - Vec4::from([self.pos[0]; 4]);
        let direction_y = p_y - Vec4::from([self.pos[1]; 4]);
        let direction_z = p_z - Vec4::from([self.pos[2]; 4]);

        let length_squared = direction_x * direction_x;
        let length_squared = length_squared + direction_y * direction_y;
        let length_squared = length_squared + direction_z * direction_z;

        let length = vec4_sqrt(length_squared);

        let inv_length = Vec4::ONE / length;

        let direction_x = direction_x * inv_length;
        let direction_y = direction_y * inv_length;
        let direction_z = direction_z * inv_length;

        let origin_x = [self.pos[0]; 4].into();
        let origin_y = [self.pos[1]; 4].into();
        let origin_z = [self.pos[2]; 4].into();

        RayPacket4 {
            origin_x,
            origin_y,
            origin_z,
            direction_x,
            direction_y,
            direction_z,
            t: [1e34_f32; 4].into(),
            inv_direction_x: 1.0 / direction_x,
            inv_direction_y: 1.0 / direction_y,
            inv_direction_z: 1.0 / direction_z,
        }
    }

    fn calculate_matrix(&self) -> (Vec3, Vec3, Vec3) {
        let y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
        let z: Vec3 = self.direction.normalize();
        let x: Vec3 = z.cross(y).normalize();
        let y: Vec3 = x.cross(z).normalize();
        (x, y, z)
    }

    pub fn get_rh_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);

        let projection =
            Mat4::perspective_rh_gl(self.fov, self.aspect_ratio, self.near_plane, self.far_plane);

        let pos = self.pos;
        let dir = self.direction;

        let view = Mat4::look_at_rh(pos, pos + dir, up);

        projection * view
    }

    pub fn get_lh_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);

        let projection =
            Mat4::perspective_lh(self.fov, self.aspect_ratio, self.near_plane, self.far_plane);

        let pos = self.pos;
        let dir = self.direction;

        let view = Mat4::look_at_lh(pos, pos + dir, up);

        projection * view
    }

    pub fn get_rh_projection(&self) -> Mat4 {
        Mat4::perspective_rh_gl(self.fov, self.aspect_ratio, self.near_plane, self.far_plane)
    }

    pub fn get_lh_projection(&self) -> Mat4 {
        Mat4::perspective_lh(self.fov, self.aspect_ratio, self.near_plane, self.far_plane)
    }

    pub fn get_rh_view_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);

        let pos = self.pos;
        let dir = self.direction;

        Mat4::look_at_rh(pos, pos + dir, up)
    }

    pub fn get_lh_view_matrix(&self) -> Mat4 {
        let up = Vec3::new(0.0, 1.0, 0.0);

        let pos = self.pos;
        let dir = self.direction;

        Mat4::look_at_lh(pos, pos + dir, up)
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct SkinnedMesh3D {
    pub vertices: Vec<Vertex3D>,
    pub ranges: Vec<VertexMesh>,
}

impl SkinnedMesh3D {
    pub fn apply(
        vertices: &[Vertex3D],
        skin_data: &[JointData],
        ranges: &[VertexMesh],
        joint_matrices: &[Mat4],
    ) -> Self {
        let mut vertices = vertices.to_vec();
        let ranges = ranges.to_vec();

        vertices.par_iter_mut().enumerate().for_each(|(i, v)| {
            let (joint, weight) = skin_data[i].into();
            let matrix = weight[0] * joint_matrices[joint[0] as usize];
            let matrix = matrix + (weight[1] * joint_matrices[joint[1] as usize]);
            let matrix = matrix + (weight[2] * joint_matrices[joint[2] as usize]);
            let matrix = matrix + (weight[3] * joint_matrices[joint[3] as usize]);

            v.vertex = matrix * v.vertex;
            let matrix = matrix.inverse().transpose();
            v.normal = (matrix * Vec3A::from(v.normal).extend(0.0)).truncate();
            let tangent =
                (matrix * Vec3A::new(v.tangent[0], v.tangent[1], v.tangent[2]).extend(0.0)).xyz();
            v.tangent = Vec4::new(tangent[0], tangent[1], tangent[2], v.tangent[3]);
        });

        SkinnedMesh3D { vertices, ranges }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct SkinnedTriangles3D {
    pub triangles: Vec<RTTriangle>,
}

impl SkinnedTriangles3D {
    pub fn apply(
        triangles: &[RTTriangle],
        skin_data: &[JointData],
        joint_matrices: &[Mat4],
    ) -> Self {
        let mut triangles = triangles.to_vec();

        triangles.iter_mut().enumerate().for_each(|(i, t)| {
            let i0 = i / 3;
            let i1 = i + 1;
            let i2 = i + 2;

            let (joint, weight) = skin_data[i0].into();
            let matrix: Mat4 = weight[0] * joint_matrices[joint[0] as usize];
            let matrix: Mat4 = matrix + (weight[1] * joint_matrices[joint[1] as usize]);
            let matrix: Mat4 = matrix + (weight[2] * joint_matrices[joint[2] as usize]);
            let matrix: Mat4 = matrix + (weight[3] * joint_matrices[joint[3] as usize]);
            let n_matrix: Mat4 = matrix.inverse().transpose();

            t.vertex0 = (matrix * t.vertex0.extend(1.0)).truncate();
            t.n0 = (n_matrix * t.n0.extend(0.0)).truncate();
            t.tangent0 = (n_matrix * t.tangent0.xyz().extend(0.0))
                .truncate()
                .extend(t.tangent2[3]);

            let (joint, weight) = skin_data[i1].into();
            let matrix: Mat4 = weight[0] * joint_matrices[joint[0] as usize];
            let matrix: Mat4 = matrix + (weight[1] * joint_matrices[joint[1] as usize]);
            let matrix: Mat4 = matrix + (weight[2] * joint_matrices[joint[2] as usize]);
            let matrix: Mat4 = matrix + (weight[3] * joint_matrices[joint[3] as usize]);
            let n_matrix: Mat4 = matrix.inverse().transpose();

            t.vertex1 = (matrix * t.vertex1.extend(1.0)).truncate();
            t.n1 = (n_matrix * t.n1.extend(0.0)).truncate();
            t.tangent1 = (n_matrix * t.tangent1.xyz().extend(0.0))
                .truncate()
                .extend(t.tangent2[3]);

            let (joint, weight) = skin_data[i2].into();
            let matrix: Mat4 = weight[0] * joint_matrices[joint[0] as usize];
            let matrix: Mat4 = matrix + (weight[1] * joint_matrices[joint[1] as usize]);
            let matrix: Mat4 = matrix + (weight[2] * joint_matrices[joint[2] as usize]);
            let matrix: Mat4 = matrix + (weight[3] * joint_matrices[joint[3] as usize]);
            let n_matrix: Mat4 = matrix.inverse().transpose();

            t.vertex2 = (matrix * t.vertex2.extend(1.0)).truncate();
            t.n2 = (n_matrix * t.n2.extend(0.0)).truncate();
            t.tangent2 = (n_matrix * t.tangent2.xyz().extend(0.0))
                .truncate()
                .extend(t.tangent2[3]);

            t.normal = RTTriangle::normal(t.vertex0, t.vertex1, t.vertex2);
        });

        SkinnedTriangles3D { triangles }
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct RTTriangle {
    pub vertex0: Vec3,
    pub u0: f32,
    // 16
    pub vertex1: Vec3,
    pub u1: f32,
    // 32
    pub vertex2: Vec3,
    pub u2: f32,
    // 48
    pub normal: Vec3,
    pub v0: f32,
    // 64
    pub n0: Vec3,
    pub v1: f32,
    // 80
    pub n1: Vec3,
    pub v2: f32,
    // 96
    pub n2: Vec3,
    pub id: i32,
    // 112
    pub tangent0: Vec4,
    // 128
    pub tangent1: Vec4,
    // 144
    pub tangent2: Vec4,
    // 160
    pub light_id: i32,
    pub mat_id: i32,
    pub lod: f32,
    pub area: f32,
    // 176

    // GLSL structs' size are rounded up to the base alignment of vec4s
    // Thus, we pad these triangles to become 160 bytes and 16-byte (vec4) aligned
}

impl Default for RTTriangle {
    fn default() -> Self {
        // assert_eq!(std::mem::size_of::<RTTriangle>() % 16, 0);
        Self {
            vertex0: Vec3::ZERO,
            u0: 0.0,
            vertex1: Vec3::ZERO,
            u1: 0.0,
            vertex2: Vec3::ZERO,
            u2: 0.0,
            normal: Vec3::ZERO,
            v0: 0.0,
            n0: Vec3::ZERO,
            v1: 0.0,
            n1: Vec3::ZERO,
            v2: 0.0,
            n2: Vec3::ZERO,
            id: 0,
            tangent0: Vec4::ZERO,
            tangent1: Vec4::ZERO,
            tangent2: Vec4::ZERO,
            light_id: 0,
            mat_id: 0,
            lod: 0.0,
            area: 0.0,
        }
    }
}

impl SpatialTriangle for RTTriangle {
    fn vertex0(&self) -> Vec3 {
        self.vertex0
    }

    fn vertex1(&self) -> Vec3 {
        self.vertex1
    }

    fn vertex2(&self) -> Vec3 {
        self.vertex2
    }
}

impl Primitive for RTTriangle {
    fn center(&self) -> Vec3 {
        (self.vertex0 + self.vertex1 + self.vertex2) / 3.0
    }

    fn aabb(&self) -> Aabb<i32> {
        aabb!(self.vertex0, self.vertex1, self.vertex2)
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, Copy)]
pub struct HitRecord {
    pub bary_centrics: Vec2,
    pub material_id: i32,
}

#[allow(dead_code)]
impl RTTriangle {
    pub fn vertices(&self) -> (Vec3, Vec3, Vec3) {
        (self.vertex0, self.vertex1, self.vertex2)
    }

    #[inline]
    pub fn normal(v0: Vec3, v1: Vec3, v2: Vec3) -> Vec3 {
        let a = v1 - v0;
        let b = v2 - v0;
        a.cross(b).normalize()
    }

    #[inline]
    pub fn area(v0: Vec3, v1: Vec3, v2: Vec3) -> f32 {
        let a = (v1 - v0).length();
        let b = (v2 - v1).length();
        let c = (v0 - v2).length();
        let s = (a + b + c) * 0.5;
        (s * (s - a) * (s - b) * (s - c)).sqrt()
    }

    #[inline]
    pub fn center(&self) -> Vec3 {
        let (v0, v1, v2) = self.vertices();
        (v0 + v1 + v2) * (1.0 / 3.0)
    }

    #[inline(always)]
    pub fn bary_centrics(
        v0: Vec3,
        v1: Vec3,
        v2: Vec3,
        edge1: Vec3,
        edge2: Vec3,
        p: Vec3,
        n: Vec3,
    ) -> (f32, f32) {
        let abc = n.dot((edge1).cross(edge2));
        let pbc = n.dot((v1 - p).cross(v2 - p));
        let pca = n.dot((v2 - p).cross(v0 - p));
        (pbc / abc, pca / abc)
    }

    // Transforms triangle using given matrix and normal_matrix (transposed of inverse of matrix)
    pub fn transform(&self, matrix: Mat4, normal_matrix: Mat3) -> RTTriangle {
        let vertex0 = self.vertex0.extend(1.0);
        let vertex1 = self.vertex1.extend(1.0);
        let vertex2 = self.vertex2.extend(1.0);

        let vertex0 = matrix * vertex0;
        let vertex1 = matrix * vertex1;
        let vertex2 = matrix * vertex2;

        let n0 = normal_matrix * self.n0;
        let n1 = normal_matrix * self.n1;
        let n2 = normal_matrix * self.n2;

        RTTriangle {
            vertex0: vertex0.truncate(),
            vertex1: vertex1.truncate(),
            vertex2: vertex2.truncate(),
            n0,
            n1,
            n2,
            ..(*self)
        }
    }

    #[inline(always)]
    pub fn intersect_hit(&self, ray: &mut Ray, epsilon: f32) -> Option<HitRecord> {
        let edge1 = self.vertex1 - self.vertex0;
        let edge2 = self.vertex2 - self.vertex0;

        let h = ray.direction.cross(edge2);
        let a = edge1.dot(h);
        if a > -epsilon && a < epsilon {
            return None;
        }

        let f = 1.0 / a;
        let s = ray.origin - self.vertex0;
        let u = f * s.dot(h);
        let q = s.cross(edge1);
        let v = f * ray.direction.dot(q);

        if !(0.0..=1.0).contains(&u) || v < 0.0 || (u + v) > 1.0 {
            return None;
        }

        let t_value = f * edge2.dot(q);
        if !(ray.t_min..ray.t).contains(&t_value) {
            return None;
        }

        ray.t = t_value;

        // Calculate barycentrics
        let inv_denom = 1.0 / self.normal.dot(self.normal);
        let bary_centrics = vec2(u, v) * inv_denom;

        Some(HitRecord {
            bary_centrics,
            material_id: self.mat_id,
        })
    }
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct RTTriangle2D {
    pub vertex0: Vec3,
    pub vertex1: Vec3,
    pub vertex2: Vec3,
    pub normal: Vec3,
}

impl SpatialTriangle for RTTriangle2D {
    fn vertex0(&self) -> Vec3 {
        self.vertex0
    }

    fn vertex1(&self) -> Vec3 {
        self.vertex1
    }

    fn vertex2(&self) -> Vec3 {
        self.vertex2
    }
}

impl Primitive for RTTriangle2D {
    fn center(&self) -> Vec3 {
        (self.vertex0 + self.vertex1 + self.vertex2) / 3.0
    }

    fn aabb(&self) -> Aabb<i32> {
        aabb!(self.vertex0, self.vertex1, self.vertex2)
    }
}

#[allow(dead_code)]
impl RTTriangle2D {
    pub fn vertices(&self) -> (Vec3, Vec3, Vec3) {
        (self.vertex0, self.vertex1, self.vertex2)
    }

    #[inline]
    pub fn normal(v0: Vec3, v1: Vec3, v2: Vec3) -> Vec3 {
        let a = v1 - v0;
        let b = v2 - v0;
        a.cross(b).normalize()
    }

    #[inline]
    pub fn area(v0: Vec3, v1: Vec3, v2: Vec3) -> f32 {
        let a = (v1 - v0).length();
        let b = (v2 - v1).length();
        let c = (v0 - v2).length();
        let s = (a + b + c) * 0.5;
        (s * (s - a) * (s - b) * (s - c)).sqrt()
    }

    #[inline]
    pub fn center(&self) -> Vec3 {
        let (v0, v1, v2) = self.vertices();
        (v0 + v1 + v2) * (1.0 / 3.0)
    }

    #[inline(always)]
    pub fn bary_centrics(
        v0: Vec3,
        v1: Vec3,
        v2: Vec3,
        edge1: Vec3,
        edge2: Vec3,
        p: Vec3,
        n: Vec3,
    ) -> (f32, f32) {
        let abc = n.dot((edge1).cross(edge2));
        let pbc = n.dot((v1 - p).cross(v2 - p));
        let pca = n.dot((v2 - p).cross(v0 - p));
        (pbc / abc, pca / abc)
    }

    // Transforms triangle using given matrix and normal_matrix (transposed of inverse of matrix)
    pub fn transform(&self, matrix: Mat4) -> Self {
        let vertex0 = self.vertex0.extend(1.0);
        let vertex1 = self.vertex1.extend(1.0);
        let vertex2 = self.vertex2.extend(1.0);

        let vertex0 = matrix * vertex0;
        let vertex1 = matrix * vertex1;
        let vertex2 = matrix * vertex2;

        Self {
            vertex0: vertex0.truncate(),
            vertex1: vertex1.truncate(),
            vertex2: vertex2.truncate(),
            normal: Self::normal(vertex0.xyz(), vertex1.xyz(), vertex2.xyz()),
        }
    }

    #[inline(always)]
    pub fn intersect_hit(&self, ray: &mut Ray, epsilon: f32) -> Option<HitRecord> {
        let edge1 = self.vertex1 - self.vertex0;
        let edge2 = self.vertex2 - self.vertex0;

        let h = ray.direction.cross(edge2);
        let a = edge1.dot(h);
        if a > -epsilon && a < epsilon {
            return None;
        }

        let f = 1.0 / a;
        let s = ray.origin - self.vertex0;
        let u = f * s.dot(h);
        let q = s.cross(edge1);
        let v = f * ray.direction.dot(q);

        if !(0.0..=1.0).contains(&u) || v < 0.0 || (u + v) > 1.0 {
            return None;
        }

        let t_value = f * edge2.dot(q);
        if !(ray.t_min..ray.t).contains(&t_value) {
            return None;
        }

        ray.t = t_value;

        // Calculate barycentrics
        let inv_denom = 1.0 / self.normal.dot(self.normal);
        let bary_centrics = vec2(u, v) * inv_denom;

        Some(HitRecord {
            bary_centrics,
            material_id: -1,
        })
    }
}
