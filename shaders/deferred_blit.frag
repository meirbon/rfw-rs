#version 450

layout(location = 0) in vec2 UV;

layout(set = 0, binding = 0, rgba16f) uniform readonly image2D Albedo;
layout(set = 0, binding = 1, rgba16f) uniform readonly image2D Radiance;
layout(set = 0, binding = 2, rgba16f) uniform readonly image2D SSAO;

layout(location = 0) out vec4 OutColor;

void main() {
    const ivec2 pixel = ivec2(gl_FragCoord.xy - 0.5);
    const vec3 albedo = imageLoad(Albedo, pixel).xyz;
    const vec3 radiance = imageLoad(Radiance, pixel).xyz;
    const float ssao = imageLoad(SSAO, pixel).r;
    const vec3 ambient = albedo * 0.03 * ssao;

    OutColor = vec4(radiance + ambient, 1.0);
}