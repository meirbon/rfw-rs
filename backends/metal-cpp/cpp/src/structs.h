#ifndef METALCPP_BACKENDS_METAL_CPP_CPP_SRC_STRUCTS_H
#define METALCPP_BACKENDS_METAL_CPP_CPP_SRC_STRUCTS_H

#define VERTICES_ARG_INDEX 0
#define VERTICES_2D_ARG_INDEX 1
#define TEXTURES_ARG_INDEX 2
#define MATERIALS_ARG_INDEX 3
#define INSTANCES_ARG_INDEX 4
#define INSTANCES_2D_ARG_INDEX 5

#include <simd/simd.h>

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
    simd_float4x4 projection;
    simd_float4x4 view_matrix;
    simd_float4x4 combined;
    simd_float4x4 matrix_2d;
    CameraView view;
} UniformCamera;

typedef struct
{
    simd_float4x4 matrix;
    simd_float4x4 normal_matrix;
} InstanceTransform;

#endif // METALCPP_BACKENDS_METAL_CPP_CPP_SRC_STRUCTS_H
