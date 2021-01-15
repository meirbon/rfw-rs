#include <metal_stdlib>

using namespace metal;

struct ColorInOut {
    float4 position [[position]];
    float4 color;
};

struct Vertex3D {
    float4 v;
    float n_x;
    float n_y;
    float n_z;
    uint mat_id;
    float2 uv;
    float4 tangent;
};

struct InstanceTransform {
  float4x4 matrix;
  float4x4 normal_matrix;
};

struct CameraView {
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
};

struct UniformCamera {
    float4x4 projection;
    float4x4 view_matrix;
    float4x4 combined;
    CameraView view;
};

// vertex shader function
vertex ColorInOut triangle_vertex(
    const device Vertex3D* vertex_array       [[ buffer(0) ]],
    const device UniformCamera* camera        [[ buffer(1) ]],
    const device InstanceTransform* instances [[ buffer(2) ]],
    unsigned int vid                          [[ vertex_id ]],
    unsigned int i_id                         [[ instance_id ]]
)
{
    ColorInOut out;

    const auto device &v = vertex_array[vid];
    const auto device &t = instances[i_id];

    const float4 normal = t.normal_matrix * float4(v.n_x, v.n_y, v.n_z, 0.0);

    out.position = camera->combined * t.matrix * v.v;
    out.color = float4(normal.xyz, 0.2);

    return out;
}

// fragment shader function
fragment float4 triangle_fragment(ColorInOut in [[stage_in]])
{
    return in.color;
}


struct Rect {
    float x;
    float y;
    float w;
    float h;
};

struct Color {
    float r;
    float g;
    float b;
    float a;
};

struct ClearRect {
    Rect rect;
    Color color;
};

float2 rect_vert(
    Rect rect,
    uint vid
)
{
    float2 pos;

    float left = rect.x;
    float right = rect.x + rect.w;
    float bottom = rect.y;
    float top = rect.y + rect.h;

    switch (vid) {
    case 0:
        pos = float2(right, top);
        break;
    case 1:
        pos = float2(left, top);
        break;
    case 2:
        pos = float2(right, bottom);
        break;
    case 3:
        pos = float2(left, bottom);
        break;
    }
    return pos;
}

vertex ColorInOut clear_rect_vertex(
    const device ClearRect *clear_rect  [[ buffer(0) ]],
    unsigned int vid                    [[ vertex_id ]]
)
{
    ColorInOut out;
    float4 pos = float4(rect_vert(clear_rect->rect, vid), 0, 1);
    auto col = clear_rect->color;

    out.position = pos;
    out.color = float4(col.r, col.g, col.b, col.a);
    return out;
}

fragment float4 clear_rect_fragment(
    ColorInOut in [[stage_in]]
)
{
    return in.color;
}