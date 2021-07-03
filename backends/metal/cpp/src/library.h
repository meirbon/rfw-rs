#ifndef CPP_LIBRARY_H
#define CPP_LIBRARY_H

#ifndef API
#define API
#endif

#include "structs.h"

typedef struct
{
    simd_float4 bmin;
    simd_float4 bmax;
} Aabb;

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
    simd_float4 tangent0;
    simd_float4 tangent1;
    simd_float4 tangent2;
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
    simd_float4 weight;
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
    const simd_float4x4 *matrices;
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
    const simd_float4x4 *matrices;
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

API void *create_instance(void *ns_window, void *ns_view, unsigned int width, unsigned int height, double scale);
API void destroy_instance(void *instance);

API void set_2d_mesh(void *instance, unsigned int id, MeshData2D data);
API void set_2d_instances(void *instance, unsigned int id, InstancesData2D data);

API void set_3d_mesh(void *instance, unsigned int id, MeshData3D data);
API void unload_3d_meshes(void *instance, const unsigned int *ids, unsigned int num);
API void set_3d_instances(void *instance, unsigned int id, InstancesData3D data);

API void set_materials(void *instance, const DeviceMaterial *materials, unsigned int num_materials);
API void set_textures(void *instance, const TextureData *data, unsigned int num_textures, const unsigned int *changed);

API void render(void *instance, simd_float4x4 matrix_2d, CameraView3D view_3d);
API void synchronize(void *instance);

API void resize(void *instance, unsigned int width, unsigned int height, double scale_factor);
#endif // CPP_LIBRARY_H
