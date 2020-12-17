#version 450
#extension GL_GOOGLE_include_directive : require
#extension GL_ARB_separate_shader_objects : enable

#include "lights.glsl"

layout(location = 0) in vec4 Vertex;

layout(set = 0, binding = 0) uniform Locals {
    LightInfo info;
};

struct AABB {
    vec4 bmin;
    vec4 bmax;
};

struct Instance {
    mat4 Transform;
    mat4 InverseTransform;
    mat4 NormalTransform;
    AABB Bounds;
    AABB OriginalBounds;
};

layout(set = 1, binding = 0) buffer readonly Instances { Instance instances[]; };

layout (location = 0) out vec4 LightSpaceV;
layout (location = 1) out vec4 V;

void main() {
    const vec4 v = instances[gl_InstanceIndex].Transform * Vertex;
    V = v;

    const vec4 Light_V = info.MP * vec4(v.xyz, 1.0);
    LightSpaceV = Light_V;
    gl_Position = Light_V;
}