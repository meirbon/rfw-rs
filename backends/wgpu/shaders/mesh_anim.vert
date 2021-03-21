#version 450

layout(location = 0) in vec4 Vertex;
layout(location = 1) in vec3 Normal;
layout(location = 2) in uint MatID;
layout(location = 3) in vec2 UV;
layout(location = 4) in vec4 Tangent;
layout(location = 5) in uvec4 joints;
layout(location = 6) in vec4 weights;

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

layout(set = 2, binding = 0) buffer readonly Skin { mat4 M[]; };

layout(location = 0) out vec4 V;
layout(location = 1) out vec4 SSV;
layout(location = 2) out vec3 N;
layout(location = 3) out uint MID;
layout(location = 4) out vec2 TUV;
layout(location = 5) out vec3 T;
layout(location = 6) out vec3 B;

void main() {
    const mat4 skinMatrix = (weights.x * M[joints.x]) + (weights.y * M[joints.y]) + (weights.z * M[joints.z]) + (weights.w * M[joints.w]);
//    const mat4 skinMatrix = (weights.x * mat4(1.0)) + (weights.y * mat4(1.0)) + (weights.z * mat4(1.0)) + (weights.w * mat4(1.0));
    const mat4 inverseSkinMatrix = transpose(inverse(skinMatrix));
//    const mat4 skinMatrix = mat4(1.0f);
//    const mat4 inverseSkinMatrix = mat4(1.0f);

    const vec4 vertex = transforms[gl_InstanceIndex].M *  skinMatrix * Vertex;
    const vec4 cVertex = View * vec4(vertex.xyz, 1.0);

    gl_Position = Proj * cVertex;

    V = vec4(vertex.xyz, cVertex.w);
    SSV = cVertex;
    N = normalize(vec3(transforms[gl_InstanceIndex].IM * inverseSkinMatrix * vec4(Normal, 0.0)));
    T = normalize(vec3(transforms[gl_InstanceIndex].IM * inverseSkinMatrix * vec4(Tangent.xyz, 0.0)));
    B = cross(N, T) * Tangent.w;
    MID = MatID;
    TUV = UV;
}