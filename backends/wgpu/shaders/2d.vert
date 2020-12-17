#version 450

layout(location = 0) in vec3 V;
layout(location = 1) in uint HasTex;
layout(location = 2) in vec2 UV;
layout(location = 3) in vec4 C;

struct Instance {
    mat4 matrix;
    uvec4 aux;
};

layout(set = 0, binding = 0) buffer readonly Descriptors { Instance Instances[]; };

layout(location = 0) out vec3 UvTex;
layout(location = 1) out vec4 Color;

void main() {
    gl_Position = Instances[gl_InstanceIndex].matrix * vec4(V.xyz, 1.0);
    UvTex = vec3(UV, HasTex);
    Color = C;
}