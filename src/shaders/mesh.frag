#version 450

layout(location = 0) in vec4 V;
layout(location = 1) in vec3 N;
layout(location = 2) in flat uint MID;
layout(location = 3) in vec2 TUV;

layout(location = 0) out vec4 color;

void main() {
    color = vec4(N, V.w);
}