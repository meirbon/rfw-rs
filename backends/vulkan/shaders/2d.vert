#version 450

layout(location = 0) in vec3 V;
layout(location = 1) in uint HasTex;
layout(location = 2) in vec2 UV;
layout(location = 3) in vec4 C;

layout(set = 0, binding = 0) uniform Locals {
    mat4 View;
    mat4 Proj;
    mat4 matrix_2d;
    uvec4 light_count;
    vec4 cam_pos;
};
layout(set = 1, binding = 0) buffer readonly Instances { mat4 matrices[]; };

layout(location = 0) out vec3 UvTex;
layout(location = 1) out vec4 Color;

void main() {
    gl_Position = matrix_2d * matrices[gl_InstanceIndex] * vec4(V.xyz, 1.0);
    UvTex = vec3(UV, HasTex);
    Color = C;
}