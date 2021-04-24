#version 450
#extension GL_GOOGLE_include_directive : require

layout(location = 0) in vec4 V;
layout(location = 1) in vec4 SSV;
layout(location = 2) in vec3 N;
layout(location = 3) in flat uint MID;
layout(location = 4) in vec2 TUV;
layout(location = 5) in vec3 T;
layout(location = 6) in vec3 B;

layout(location = 0) out vec4 Color;

void main() {
    Color = vec4(max(vec3(0.2), N), 1.0);
}