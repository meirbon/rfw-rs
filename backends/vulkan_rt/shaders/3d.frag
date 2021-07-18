#version 450

layout(location = 0) in vec4 V;
layout(location = 1) in vec3 N;

layout(location = 0) out vec4 Color;

void main()
{
    Color = vec4(N.xyz, 1.0);
}