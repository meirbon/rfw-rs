#version 450

#extension GL_ARB_shader_viewport_layer_array : require

layout(location = 0) in vec4 Vertex;

layout(set = 0, binding = 0) uniform Locals {
    mat4 VP[6];
};

layout(set = 1, binding = 0) buffer readonly I {
    mat4 Transform[];
};

void main() {
    const vec4 V = Transform[gl_InstanceIndex] * Vertex;
    for (int face = 0; face < 6; ++face)
    {
        gl_Layer = face;
        gl_Position = VP[face] * V;
    }
}