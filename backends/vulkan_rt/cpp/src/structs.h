#ifndef STRUCTS_H
#define STRUCTS_H

typedef struct
{
	float x;
	float y;
} Vector2;

typedef struct
{
	float x;
	float y;
	float z;
} Vector3;

typedef struct
{
	float x;
	float y;
	float z;
	float w;
} Vector4;

typedef struct
{
	Vector4 columns[4];
} Vector4x4;

typedef struct
{
	Vector4 bmin;
	Vector4 bmax;
} Aabb;

typedef struct
{
	float v_x;
	float v_y;
	float v_z;
	unsigned int tex;

	float u;
	float v;
	float c_r;
	float c_g;
	float c_b;
	float c_a;
} Vertex2D;

typedef struct
{
	float v_x;
	float v_y;
	float v_z;
	float v_w;

	float n_x;
	float n_y;
	float n_z;
	unsigned int mat_id;

	float u;
	float v;
	float pad0;
	float pad1;

	float t_x;
	float t_y;
	float t_z;
	float t_w;
} Vertex3D;

typedef struct
{
	float pos_x;
	float pos_y;
	float pos_z;
	float right_x;
	float right_y;
	float right_z;
	float up_x;
	float up_y;
	float up_z;
	float p1_x;
	float p1_y;
	float p1_z;
	float direction_x;
	float direction_y;
	float direction_z;
	float lens_size;
	float spread_angle;
	float inv_width;
	float inv_height;
	float near_plane;
	float far_plane;
	float aspect_ratio;
	float fov;
} CameraView;

typedef struct
{
	Vector3 vertex0;
	float u0;
	Vector3 vertex1;
	float u1;
	Vector3 vertex2;
	float u2;
	Vector3 normal;
	float v0;
	Vector3 n0;
	float v1;
	Vector3 n1;
	float v2;
	Vector3 n2;
	int id;
	Vector4 tangent0;
	Vector4 tangent1;
	Vector4 tangent2;
	int light_id;
	int mat_id;
	float lod;
	float area;
} RTTriangle;

typedef struct
{
	Aabb bounds;
	unsigned int first;
	unsigned int last;
	unsigned int mat_id;
	unsigned int padding;
} VertexRange;

typedef struct
{
	unsigned int j_x, j_y, j_z, j_w;
	Vector4 weight;
} JointData;

typedef enum : unsigned int
{
	SHADOW_CASTER = 1,
	ALLOW_SKINNING = 2
} Mesh3dFlags;

typedef struct
{
	const Vertex3D *vertices;
	unsigned int num_vertices;
	const RTTriangle *triangles;
	unsigned int num_triangles;
	const VertexRange *ranges;
	unsigned int num_ranges;
	const JointData *skin_data;
	unsigned int flags;
	Aabb bounds;
} MeshData3D;

typedef enum : unsigned int
{
	TRANSFORMED = 1
} InstanceFlags3D;

typedef struct
{
	Aabb local_aabb;
	const Vector4x4 *matrices;
	unsigned int num_matrices;
	const int *skin_ids;
	unsigned int num_skin_ids;
	const unsigned int *flags;
	unsigned int num_flags;
} InstancesData3D;

typedef struct
{
	const Vertex2D *vertices;
	unsigned int num_vertices;
	int tex_id;
} MeshData2D;

typedef struct
{
	const Vector4x4 *matrices;
	unsigned int num_matrices;
} InstancesData2D;

typedef enum : unsigned int
{
	BGRA8 = 0,
	RGBA8 = 1
} DataFormat;

typedef struct
{
	unsigned int width;
	unsigned int height;
	unsigned int mip_levels;
	const unsigned char *bytes;
	DataFormat format;
} TextureData;

typedef struct
{
	Vector3 pos;
	Vector3 right;
	Vector3 up;
	Vector3 p1;
	Vector3 direction;
	float lens_size;
	float spread_angle;
	float epsilon;
	float inv_width;
	float inv_height;
	float near_plane;
	float far_plane;
	float aspect_ratio;
	float fov;
	Vector4 custom0;
	Vector4 custom1;
} CameraView3D;

typedef struct
{
	// color
	float c_r;
	float c_g;
	float c_b;
	float c_a;

	// absorption
	float a_r;
	float a_g;
	float a_b;
	float a_a;

	// specular
	float s_r;
	float s_g;
	float s_b;
	float s_a;

	unsigned int params_x;
	unsigned int params_y;
	unsigned int params_z;
	unsigned int params_w;

	unsigned int flags;
	int diffuse_map;
	int normal_map;
	int metallic_roughness_map;

	int emissive_map;
	int sheen_map;
	float pad1;
	float pad2;
} DeviceMaterial;
#endif