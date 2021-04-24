#version 450

layout(location = 0) in vec4 Vertex;
layout(location = 1) in vec3 Normal;
layout(location = 2) in uint MatID;
layout(location = 3) in vec2 UV;
layout(location = 4) in vec4 Tangent;

layout(set = 0, binding = 0) uniform Locals {
    mat4 View;
    mat4 Proj;
    mat4 matrix_2d;
    uvec4 light_count;
    vec4 cam_pos;
};

struct Transform {
    mat4 M;
    mat4 IM;
};

layout(set = 0, binding = 4) buffer readonly Instances {
    Transform transforms[];
};

layout(location = 0) out vec4 V;
layout(location = 1) out vec4 SSV;
layout(location = 2) out vec3 N;
layout(location = 3) out uint MID;
layout(location = 4) out vec2 TUV;
layout(location = 5) out vec3 T;
layout(location = 6) out vec3 B;

void main() {
    const vec4 vertex = transforms[gl_InstanceIndex].M * Vertex;
    const vec4 cVertex = View * vec4(vertex.xyz, 1.0);

    gl_Position = Proj * cVertex;

    V = vec4(vertex.xyz, cVertex.w);
    SSV = cVertex;
    N = normalize(vec3(transforms[gl_InstanceIndex].IM * vec4(Normal, 0.0)));
    T = normalize(vec3(transforms[gl_InstanceIndex].IM * vec4(Tangent.xyz, 0.0)));
    B = cross(N, T) * Tangent.w;
    MID = MatID;
    TUV = UV;
}