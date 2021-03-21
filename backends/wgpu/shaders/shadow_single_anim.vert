#version 450

#include "lights.glsl"

layout(location = 0) in vec4 Vertex;
layout(location = 5) in uvec4 joints;
layout(location = 6) in vec4 weights;

layout(set = 0, binding = 0) uniform Locals {
    LightInfo info;
};

struct Transform {
    mat4 M;
    mat4 IM;
};

layout(set = 1, binding = 4) buffer readonly Instances {
    Transform transforms[];
};

layout(set = 2, binding = 0) buffer readonly Skin { mat4 M[]; };

layout (location = 0) out vec4 LightSpaceV;
layout (location = 1) out vec4 V;

void main() {
    const mat4 skinMatrix = (weights.x * M[joints.x]) + (weights.y * M[joints.y]) + (weights.z * M[joints.z]) + (weights.w * M[joints.w]);
    const vec4 v = transforms[gl_InstanceIndex].M * skinMatrix * Vertex;
    V = v;

    const vec4 Light_V = info.MP * vec4(v.xyz, 1.0);
    LightSpaceV = Light_V;
    gl_Position = Light_V;
}