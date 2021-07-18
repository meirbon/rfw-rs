#version 450

layout(location = 0) in vec4 Vertex;
layout(location = 1) in vec3 Normal;
layout(location = 2) in uint MatID;
layout(location = 3) in vec2 UV;
layout(location = 4) in vec4 Tangent;

layout(set = 0, binding = 0) uniform Locals {
    mat4 matrix_2d;
    mat4 view;
    mat4 projection;
    mat4 combined;

    vec4 cameraPosition;
    vec4 cameraDirection;
};

layout(set = 0, binding = 1) buffer readonly Instances {
    mat4 matrices[];
};

layout(location = 0) out vec4 V;
layout(location = 1) out vec3 N;

void main()
{
    mat4 matrix = matrices[gl_InstanceIndex];
    vec4 vertex = combined * matrix * Vertex;
    V = vertex;
    gl_Position = vertex;
    N = Normal;
}