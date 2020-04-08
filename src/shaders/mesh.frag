#version 450
layout(location = 0) out vec4 color;

layout(location = 0) in vec4 V;
layout(location = 1) in vec3 N;
layout(location = 2) in flat uint MID;
layout(location = 3) in vec2 TUV;

struct Material {
    vec3 color;
    vec3 specular;
    float opacity;
    float roughness;
    int diffuse_tex;
    int normal_tex;
};

layout(set = 0, binding = 1) buffer readonly Materials {
    Material materials[];
};

void main() {
    Material mat = materials[MID];
    color = vec4(mat.color * N, V.w);
}