#version 450

layout(location = 0) in vec4 V;
layout(location = 1) in vec2 UV;

struct Instance {
    mat4 matrix;
    vec4 color;
    uvec4 aux;
};

layout(set = 0, binding = 0) buffer readonly Descriptors { Instance Instances[]; };

layout(location = 0) out vec3 UvTex;
layout(location = 1) out vec4 Color;

void main() {
    gl_Position = Instances[gl_InstanceIndex].matrix * vec4(V.xyz, 1.0);
    UvTex = vec3(UV, Instances[gl_InstanceIndex].aux.x > 0 ? 1.0 : 0.0);
    Color =  Instances[gl_InstanceIndex].color;
}