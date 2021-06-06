#version 450

layout(location = 0) in vec2 UvTex;
layout(location = 1) in flat uint TexID;
layout(location = 2) in vec4 Color;

layout(set = 0, binding = 2) uniform sampler Sampler;
layout(set = 1, binding = 0) uniform texture2D textures[128];

layout(location = 0) out vec4 C;

void main() {
    vec4 color = Color;
    if (TexID > 0) {
        color = color * texture(sampler2D(textures[TexID], Sampler), UvTex.xy).rgba;
    }

    if (color.a <= 0.0) {
        discard;
    }

    C = color;
}