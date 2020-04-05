#version 450

layout(location = 0) in vec2 UV;
layout(location = 0) out vec4 OutColor;
layout(set = 0, binding = 1) uniform texture2D Texture;
layout(set = 0, binding = 2) uniform sampler Sampler;

void main() {
    OutColor = texture(sampler2D(Texture, Sampler), UV);
}