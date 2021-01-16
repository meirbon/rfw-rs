#version 450

layout(location = 0) in vec3 V;
layout(location = 1) in uint HasTex;
layout(location = 2) in vec2 UV;
layout(location = 3) in vec4 C;

layout(set = 0, binding = 0) buffer readonly Instances { mat4 matrices[]; };

layout(location = 0) out vec3 UvTex;
layout(location = 1) out vec4 Color;

const vec2 positions[6] = vec2[6](
// Upper left triangle
vec2(-1.0, -1.0),
vec2(1.0, -1.0),
vec2(-1.0, 1.0),
// Lower right triangle
vec2(-1.0, 1.0),
vec2(1.0, -1.0),
vec2(1.0, 1.0)
);

const vec2 uv[6] = vec2[6](
// Upper left triangle
vec2(0.0, 1.0),
vec2(1.0, 1.0),
vec2(0.0, 0.0),

// Lower right triangle
vec2(0.0, 0.0),
vec2(1.0, 1.0),
vec2(1.0, 0.0)
);


void main() {
    gl_Position = matrices[gl_InstanceIndex] * vec4(V.xyz, 1.0);
    UvTex = vec3(UV, HasTex);
    Color = C;
}