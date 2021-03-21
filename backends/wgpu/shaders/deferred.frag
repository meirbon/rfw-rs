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

layout(set = 1, binding = 0) uniform texture2D textures[128];

layout(location = 0) out vec4 Albedo;
layout(location = 1) out vec4 Normal;
layout(location = 2) out vec4 WorldPos;
layout(location = 3) out vec4 SSPos;
layout(location = 4) out vec4 Params;

void main() {
    vec3 color = materials[MID].color.xyz;
    vec3 normal = N;

    const uint flags = materials[MID].flags;
    vec4 params = vec4(0);

    if (HAS_DIFFUSE_MAP(flags)) {
        vec4 t_color = texture(sampler2D(textures[materials[MID].diffuse_map], Sampler), TUV).rgba;
        if (t_color.a < 0.5) {
            discard;
        }
        color = t_color.xyz;
    }

    if (HAS_NORMAL_MAP(flags)) {
        const vec3 n = (texture(sampler2D(textures[materials[MID].normal_map], Sampler), TUV).rgb - 0.5) * 2.0;
        normal = normalize(mat3(T, B, normal) * n);
    }

    if (HAS_METAL_ROUGH_MAP(flags)) {
        params.xy = texture(sampler2D(textures[materials[MID].metallic_roughness_map], Sampler), TUV).gb;
    }

    if (HAS_SHEEN_MAP(flags)) {
        params.z = texture(sampler2D(textures[materials[MID].sheen_map], Sampler), TUV).r;
    }

    Albedo = vec4(color, MID);
    Normal = vec4(normal, 0.0);
    WorldPos = vec4(V.xyz, gl_FragCoord.z);
    SSPos = SSV;
    Params = params;
}