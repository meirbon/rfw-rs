#version 450
layout(location = 0) out vec4 Color;

layout(location = 0) in vec4 V;
layout(location = 1) in vec3 N;
layout(location = 2) in flat uint MID;
layout(location = 3) in vec2 TUV;

// struct Material {
//     vec3 color;
//     vec3 specular;
//     float opacity;
//     float roughness;
//     int diffuse_tex;
//     int normal_tex;
// };

// layout(set = 0, binding = 1) buffer readonly Materials {
//     Material materials[];
// };

void main() {
    // const vec3 color = materials[MID].color;
    // Color = vec4(color * N, V.w);
    Color = vec4(N, V.w);
}