#version 450
#extension GL_GOOGLE_include_directive : require
#extension GL_ARB_separate_shader_objects : enable

#include "lights.glsl"

layout(set = 0, binding = 0) uniform Locals {
    LightInfo info;
};

layout (location = 0) in vec4 LightSpaceV;
layout (location = 1) in vec4 V;

layout (location = 0) out vec2 Depth;

void main() {
    float d = LightSpaceV.z / LightSpaceV.w;
    d = linearizeDepth(d, info.PosRange.w);
    const float moment1 = d;
    const float dx = dFdx(moment1);
    const float dy = dFdy(moment1);
    const float moment2 = d * d + 0.25 * (dx * dx + dy * dy);
    Depth = vec2(moment1, moment2);
}