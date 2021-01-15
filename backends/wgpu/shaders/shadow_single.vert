#version 450

#include "lights.glsl"

layout(location = 0) in vec4 Vertex;

layout(set = 0, binding = 0) uniform Locals {
    LightInfo info;
};

struct Transform {
    mat4 M;
    mat4 IM;
};

layout(set = 1, binding = 0) buffer readonly Instances {
    Transform transforms[];
};

layout (location = 0) out vec4 LightSpaceV;
layout (location = 1) out vec4 V;

void main() {
    const vec4 v = transforms[gl_InstanceIndex].M * Vertex;
    V = v;

    const vec4 Light_V = info.MP * vec4(v.xyz, 1.0);
    LightSpaceV = Light_V;
    gl_Position = Light_V;
}