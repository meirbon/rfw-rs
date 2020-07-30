#version 450

layout(location = 0) in vec4 Vertex;
layout(location = 1) in vec3 Normal;
layout(location = 2) in uint MatID;
layout(location = 3) in vec2 UV;
layout(location = 4) in vec4 Tangent;
layout(location = 5) in uvec4 Joints;
layout(location = 6) in vec4 Weights;

layout(set = 0, binding = 0) uniform Locals {
    mat4 View;
    mat4 Proj;
    uvec4 light_count;
    vec4 cam_pos;
};

layout(set = 1, binding = 0) uniform I {
    mat4 Transform;
    mat4 InverseTransform;
};

layout(location = 0) out vec4 V;
layout(location = 1) out vec4 SSV;
layout(location = 2) out vec3 N;
layout(location = 3) out uint MID;
layout(location = 4) out vec2 TUV;
layout(location = 5) out vec3 T;
layout(location = 6) out vec3 B;

layout(set = 3, binding = 0) buffer readonly SkinMatrices { mat4 jointMatrices[]; };

void main() {
    const mat4 skinMatrix = (Weights.x * jointMatrices[Joints.x]) + (Weights.y * jointMatrices[Joints.y]) + (Weights.z * jointMatrices[Joints.z]) + (Weights.w * jointMatrices[Joints.w]);
    const mat4 inverseSkinMatrix = transpose(inverse(skinMatrix));

    const vec4 vertex = Transform * skinMatrix * Vertex;
    const vec4 cVertex = View * vec4(vertex.xyz, 1.0);

    gl_Position = Proj * cVertex;

    V = vec4(vertex.xyz, cVertex.w);
    SSV = cVertex;
    N = normalize(vec3(InverseTransform * inverseSkinMatrix * vec4(Normal, 0.0)));
    T = normalize(vec3(InverseTransform * inverseSkinMatrix * vec4(Tangent.xyz, 0.0)));
    B = cross(N, T) * Tangent.w;
    MID = MatID;
    TUV = UV;
}