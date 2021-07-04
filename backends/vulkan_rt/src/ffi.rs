/* automatically generated by rust-bindgen 0.58.1 */

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Vector4x4 {
    pub columns: [Vector4; 4usize],
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Aabb {
    pub bmin: Vector4,
    pub bmax: Vector4,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Vertex2D {
    pub v_x: f32,
    pub v_y: f32,
    pub v_z: f32,
    pub tex: ::std::os::raw::c_uint,
    pub u: f32,
    pub v: f32,
    pub c_r: f32,
    pub c_g: f32,
    pub c_b: f32,
    pub c_a: f32,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct Vertex3D {
    pub v_x: f32,
    pub v_y: f32,
    pub v_z: f32,
    pub v_w: f32,
    pub n_x: f32,
    pub n_y: f32,
    pub n_z: f32,
    pub mat_id: ::std::os::raw::c_uint,
    pub u: f32,
    pub v: f32,
    pub pad0: f32,
    pub pad1: f32,
    pub t_x: f32,
    pub t_y: f32,
    pub t_z: f32,
    pub t_w: f32,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct CameraView {
    pub pos_x: f32,
    pub pos_y: f32,
    pub pos_z: f32,
    pub right_x: f32,
    pub right_y: f32,
    pub right_z: f32,
    pub up_x: f32,
    pub up_y: f32,
    pub up_z: f32,
    pub p1_x: f32,
    pub p1_y: f32,
    pub p1_z: f32,
    pub direction_x: f32,
    pub direction_y: f32,
    pub direction_z: f32,
    pub lens_size: f32,
    pub spread_angle: f32,
    pub inv_width: f32,
    pub inv_height: f32,
    pub near_plane: f32,
    pub far_plane: f32,
    pub aspect_ratio: f32,
    pub fov: f32,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct RTTriangle {
    pub vertex0: Vector3,
    pub u0: f32,
    pub vertex1: Vector3,
    pub u1: f32,
    pub vertex2: Vector3,
    pub u2: f32,
    pub normal: Vector3,
    pub v0: f32,
    pub n0: Vector3,
    pub v1: f32,
    pub n1: Vector3,
    pub v2: f32,
    pub n2: Vector3,
    pub id: ::std::os::raw::c_int,
    pub tangent0: Vector4,
    pub tangent1: Vector4,
    pub tangent2: Vector4,
    pub light_id: ::std::os::raw::c_int,
    pub mat_id: ::std::os::raw::c_int,
    pub lod: f32,
    pub area: f32,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct VertexRange {
    pub bounds: Aabb,
    pub first: ::std::os::raw::c_uint,
    pub last: ::std::os::raw::c_uint,
    pub mat_id: ::std::os::raw::c_uint,
    pub padding: ::std::os::raw::c_uint,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct JointData {
    pub j_x: ::std::os::raw::c_uint,
    pub j_y: ::std::os::raw::c_uint,
    pub j_z: ::std::os::raw::c_uint,
    pub j_w: ::std::os::raw::c_uint,
    pub weight: Vector4,
}
#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum Mesh3dFlags {
    SHADOW_CASTER = 1,
    ALLOW_SKINNING = 2,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MeshData3D {
    pub vertices: *const Vertex3D,
    pub num_vertices: ::std::os::raw::c_uint,
    pub triangles: *const RTTriangle,
    pub num_triangles: ::std::os::raw::c_uint,
    pub ranges: *const VertexRange,
    pub num_ranges: ::std::os::raw::c_uint,
    pub skin_data: *const JointData,
    pub flags: ::std::os::raw::c_uint,
    pub bounds: Aabb,
}
impl Default for MeshData3D {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum InstanceFlags3D {
    TRANSFORMED = 1,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct InstancesData3D {
    pub local_aabb: Aabb,
    pub matrices: *const Vector4x4,
    pub num_matrices: ::std::os::raw::c_uint,
    pub skin_ids: *const ::std::os::raw::c_int,
    pub num_skin_ids: ::std::os::raw::c_uint,
    pub flags: *const ::std::os::raw::c_uint,
    pub num_flags: ::std::os::raw::c_uint,
}
impl Default for InstancesData3D {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MeshData2D {
    pub vertices: *const Vertex2D,
    pub num_vertices: ::std::os::raw::c_uint,
    pub tex_id: ::std::os::raw::c_int,
}
impl Default for MeshData2D {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct InstancesData2D {
    pub matrices: *const Vector4x4,
    pub num_matrices: ::std::os::raw::c_uint,
}
impl Default for InstancesData2D {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(u32)]
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum DataFormat {
    BGRA8 = 0,
    RGBA8 = 1,
}
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TextureData {
    pub width: ::std::os::raw::c_uint,
    pub height: ::std::os::raw::c_uint,
    pub mip_levels: ::std::os::raw::c_uint,
    pub bytes: *const ::std::os::raw::c_uchar,
    pub format: DataFormat,
}
impl Default for TextureData {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct CameraView3D {
    pub pos: Vector3,
    pub right: Vector3,
    pub up: Vector3,
    pub p1: Vector3,
    pub direction: Vector3,
    pub lens_size: f32,
    pub spread_angle: f32,
    pub epsilon: f32,
    pub inv_width: f32,
    pub inv_height: f32,
    pub near_plane: f32,
    pub far_plane: f32,
    pub aspect_ratio: f32,
    pub fov: f32,
    pub custom0: Vector4,
    pub custom1: Vector4,
}
#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct DeviceMaterial {
    pub c_r: f32,
    pub c_g: f32,
    pub c_b: f32,
    pub c_a: f32,
    pub a_r: f32,
    pub a_g: f32,
    pub a_b: f32,
    pub a_a: f32,
    pub s_r: f32,
    pub s_g: f32,
    pub s_b: f32,
    pub s_a: f32,
    pub params_x: ::std::os::raw::c_uint,
    pub params_y: ::std::os::raw::c_uint,
    pub params_z: ::std::os::raw::c_uint,
    pub params_w: ::std::os::raw::c_uint,
    pub flags: ::std::os::raw::c_uint,
    pub diffuse_map: ::std::os::raw::c_int,
    pub normal_map: ::std::os::raw::c_int,
    pub metallic_roughness_map: ::std::os::raw::c_int,
    pub emissive_map: ::std::os::raw::c_int,
    pub sheen_map: ::std::os::raw::c_int,
    pub pad1: f32,
    pub pad2: f32,
}
extern "C" {
    pub fn create_instance(
        hwnd: *mut ::std::os::raw::c_void,
        hinstance: *mut ::std::os::raw::c_void,
        width: ::std::os::raw::c_uint,
        height: ::std::os::raw::c_uint,
        scale: f64,
    ) -> *mut ::std::os::raw::c_void;
}
extern "C" {
    pub fn destroy_instance(instance: *mut ::std::os::raw::c_void);
}
extern "C" {
    pub fn set_2d_mesh(
        instance: *mut ::std::os::raw::c_void,
        id: ::std::os::raw::c_uint,
        data: MeshData2D,
    );
}
extern "C" {
    pub fn set_2d_instances(
        instance: *mut ::std::os::raw::c_void,
        id: ::std::os::raw::c_uint,
        data: InstancesData2D,
    );
}
extern "C" {
    pub fn set_3d_mesh(
        instance: *mut ::std::os::raw::c_void,
        id: ::std::os::raw::c_uint,
        data: MeshData3D,
    );
}
extern "C" {
    pub fn unload_3d_meshes(
        instance: *mut ::std::os::raw::c_void,
        ids: *const ::std::os::raw::c_uint,
        num: ::std::os::raw::c_uint,
    );
}
extern "C" {
    pub fn set_3d_instances(
        instance: *mut ::std::os::raw::c_void,
        id: ::std::os::raw::c_uint,
        data: InstancesData3D,
    );
}
extern "C" {
    pub fn set_materials(
        instance: *mut ::std::os::raw::c_void,
        materials: *const DeviceMaterial,
        num_materials: ::std::os::raw::c_uint,
    );
}
extern "C" {
    pub fn set_textures(
        instance: *mut ::std::os::raw::c_void,
        data: *const TextureData,
        num_textures: ::std::os::raw::c_uint,
        changed: *const ::std::os::raw::c_uint,
    );
}
extern "C" {
    pub fn render(
        instance: *mut ::std::os::raw::c_void,
        matrix_2d: Vector4x4,
        view_3d: CameraView3D,
    );
}
extern "C" {
    pub fn synchronize(instance: *mut ::std::os::raw::c_void);
}
extern "C" {
    pub fn resize(
        instance: *mut ::std::os::raw::c_void,
        width: ::std::os::raw::c_uint,
        height: ::std::os::raw::c_uint,
        scale_factor: f64,
    );
}
