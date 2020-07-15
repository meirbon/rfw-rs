#version 450
#extension GL_GOOGLE_include_directive : require

#include "lights.glsl"
#include "material.glsl"

layout(location = 0) in vec4 V;
layout(location = 1) in vec4 SSV;
layout(location = 2) in vec3 N;
layout(location = 3) in flat uint MID;
layout(location = 4) in vec2 TUV;
layout(location = 5) in vec3 T;
layout(location = 6) in vec3 B;

layout(std430, set = 0, binding = 1) buffer readonly Materials { Material materials[]; };
layout(set = 0, binding = 2) uniform sampler Sampler;

layout(set = 2, binding = 0) uniform texture2D AlbedoT;
layout(set = 2, binding = 1) uniform texture2D NormalT;

layout(location = 0) out vec4 Albedo;
layout(location = 1) out vec4 Normal;
layout(location = 2) out vec4 WorldPos;
layout(location = 3) out vec4 SSPos;

void main() {
    vec3 color = materials[MID].color.xyz;
    vec3 normal = N;

    const uint flags = materials[MID].flags;

    if (HAS_DIFFUSE_MAP(flags)) {
        vec4 t_color = texture(sampler2D(AlbedoT, Sampler), TUV).rgba;
        if (t_color.a < 0.5) {
            discard;
        }
        color = t_color.xyz;
    }

    if (HAS_NORMAL_MAP(flags)) {
        const vec3 n = (texture(sampler2D(NormalT, Sampler), TUV).rgb - 0.5) * 2.0;
        normal = normalize(mat3(T, B, normal) * n);
    }

    Albedo = vec4(color, uintBitsToFloat(MID));
    Normal = vec4(normal, 0.0);
    WorldPos = vec4(V.xyz, gl_FragCoord.z);
    SSPos = SSV;
}