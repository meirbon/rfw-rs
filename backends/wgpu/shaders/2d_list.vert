#version 450

layout(location = 0) in vec3 V;
layout(location = 1) in uint TID;
layout(location = 2) in vec2 UV;
layout(location = 3) in vec4 C;

layout(set = 0, binding = 0) uniform Locals {
    mat4 View;
    mat4 Proj;
    mat4 matrix_2d;
    uvec4 light_count;
    vec4 cam_pos;
};

layout(set = 0, binding = 3) buffer readonly Instances {
    mat4 matrices[];
};

layout(location = 0) out vec2 UvTex;
layout(location = 1) out uint TexID;
layout(location = 2) out vec4 Color;

void main() {
    gl_Position = matrix_2d * matrices[gl_InstanceIndex] * vec4(V.xyz, 1.0);
    UvTex = UV;
    TexID = TID;
    Color = C;
}