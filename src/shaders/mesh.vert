#version 450

layout(location = 0) in vec4 Vertex;
layout(location = 1) in vec3 Normal;
layout(location = 2) in uint MatID;
layout(location = 3) in vec2 UV;

layout(set = 0, binding = 0) uniform Locals {
    mat4 VP;
};

layout(set = 1, binding = 0) buffer readonly I {
    mat4 Transform[];
};

layout(set = 1, binding = 1) buffer readonly IN {
    mat4 InverseTransforms[];
};

layout(location = 0) out vec4 V;
layout(location = 1) out vec3 N;
layout(location = 2) out uint MID;
layout(location = 3) out vec2 TUV;

void main() {
    const vec4 vertex = VP * Transform[gl_InstanceIndex] * Vertex;
    V = vertex;
    N = normalize(vec3(InverseTransforms[gl_InstanceIndex] * vec4(Normal, 0.0)));
    MID = MatID;
    TUV = UV;
    gl_Position = vertex;
}