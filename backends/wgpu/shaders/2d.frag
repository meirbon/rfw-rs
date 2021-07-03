#version 450

layout(location = 0) in vec3 UvTex;
layout(location = 1) in vec4 Color;

layout(set = 2, binding = 0) uniform texture2D Tex;
layout(set = 2, binding = 1) uniform sampler Sampler;

layout(location = 0) out vec4 C;

void main() {
    vec4 color = Color;
    if (UvTex.z > 0.0) {
        color = color * texture(sampler2D(Tex, Sampler), UvTex.xy).rgba;
    }

    if (color.a <= 0.5) {
        discard;
    }

    C = color;
}