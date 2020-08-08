#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(location = 0) in vec4 V;
layout(location = 1) in vec4 SSV;
layout(location = 2) in vec3 N;
layout(location = 3) flat in uint MID;
layout(location = 4) in vec2 TUV;
layout(location = 5) in vec3 T;
layout(location = 6) in vec3 B;

layout(location = 0) out vec4 target0;

void main() {
    target0 = vec4(abs(N), 1.0);
}