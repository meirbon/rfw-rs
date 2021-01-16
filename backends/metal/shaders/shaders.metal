#include <metal_stdlib>

using namespace metal;

struct ColorInOut {
  float4 position [[position]];
  float4 color;
  float2 uv;
  uint tex;
  uint _dummy;
};

struct Vertex3D {
  float4 v;
  float n_x;
  float n_y;
  float n_z;
  uint mat_id;
  float t_u;
  float t_v;
  float t_x;
  float t_y;
  float t_z;
  float t_w;
};

struct Vertex2D {
  float v_x;
  float v_y;
  float v_z;
  uint has_tex;
  float u;
  float v;
  float c_x;
  float c_y;
  float c_z;
  float c_w;
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
vertex ColorInOut triangle_vertex_2d(const device Vertex2D *vertex_array
                                     [[buffer(0)]],
                                     const device float4x4 *instances
                                     [[buffer(1)]],
                                     unsigned int vid [[vertex_id]],
                                     unsigned int i_id [[instance_id]]) {
  ColorInOut out;

  const auto device &v = vertex_array[vid];
  const auto device &t = instances[i_id];

  out.position = t * float4(v.v_x, v.v_y, v.v_z, 1.0);
  out.color = float4(v.c_x, v.c_y, v.c_z, v.c_w);
  out.uv = float2(v.u, v.v);
  out.tex = v.has_tex;

  return out;
}

// fragment shader function
fragment float4 triangle_fragment_2d(ColorInOut in [[stage_in]],
                                     texture2d<float> tex [[texture(0)]]) {
  auto color = in.color;
  if (in.tex > 0) {
    constexpr sampler textureSampler(mag_filter::linear, min_filter::linear);
    color = color * tex.sample(textureSampler, in.uv);
  }

  if (color.w <= 0.0) {
    discard_fragment();
  }

  return color;
}

// vertex shader function
vertex ColorInOut triangle_vertex(
    const device Vertex3D *vertex_array [[buffer(0)]],
    const device UniformCamera *camera [[buffer(1)]],
    const device InstanceTransform *instances [[buffer(2)]],
    unsigned int vid [[vertex_id]], unsigned int i_id [[instance_id]]) {
  ColorInOut out;

  const auto device &v = vertex_array[vid];
  const auto device &t = instances[i_id];

  const float4 normal = t.normal_matrix * float4(v.n_x, v.n_y, v.n_z, 0.0);

  out.position = camera->combined * t.matrix * v.v;
  out.color = float4(normalize(normal.xyz), 0.2);
  out.uv = float2(v.t_u, v.t_v);

  return out;
}

// fragment shader function
fragment float4 triangle_fragment(ColorInOut in [[stage_in]]) {
  return in.color;
}