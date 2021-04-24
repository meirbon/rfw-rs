#version 450
#extension GL_GOOGLE_include_directive : require

layout(location = 0) in vec4 Vertex;
layout(location = 1) in vec3 Normal;
layout(location = 2) in uint MatID;
layout(location = 3) in vec2 UV;
layout(location = 4) in vec4 Tangent;

struct Transform {
    mat4 M;
    mat4 IM;
};

struct View2D {
    mat4 matrix;
};

struct View3D {
    float pos_x;// 4
    float pos_y;// 8
    float pos_z;// 12
    float right_x;// 16
    float right_y;// 20
    float right_z;// 24
    float up_x;// 28
    float up_y;// 32
    float up_z;// 36
    float p1_x;// 40
    float p1_y;// 44
    float p1_z;// 48
    float direction_x;// 52
    float direction_y;// 56
    float direction_z;// 60
    float lens_size;// 64
    float spread_angle;// 68
    float epsilon;// 72
    float inv_width;// 76
    float inv_height;// 80
    float near_plane;// 84
    float far_plane;// 88
    float aspect_ratio;// 92
    float fov;// 96
    float custom0;// 100
    float custom1;// 104
    float custom2;// 108
    float custom3;// 112
    float custom4;// 116
    float custom5;// 120
    float custom6;// 124
    float custom7;// 128
};

layout(set = 0, binding = 0) uniform Camera {
    View2D view_2d;
    View3D view_3d;
    mat4 view;
    mat4 projection;
    mat4 view_projection;
    uvec4 light_count;
};
layout(set = 0, binding = 1) buffer readonly Instances {
    Transform transforms[];
};
//layout(set = 0, binding = 2) sampler Sampler;
//layout(set = 0, binding = 3) texture2D textures[1024];
//layout(set = 0, binding = 4) uniform AreaLights {
//    AreaLight area_lights[128];
//};
//layout(set = 0, binding = 5) uniform PointLights {
//    PointLight point_lights[128];
//};
//layout(set = 0, binding = 6) uniform SpotLights {
//    SpotLight spot_lights[128];
//};
//layout(set = 0, binding = 7) uniform DirectionalLights {
//    DirectionalLight dir_lights[128];
//};

layout(location = 0) out vec4 V;
layout(location = 1) out vec4 SSV;
layout(location = 2) out vec3 N;
layout(location = 3) out uint MID;
layout(location = 4) out vec2 TUV;
layout(location = 5) out vec3 T;
layout(location = 6) out vec3 B;

void main() {
    const vec4 vertex = transforms[gl_InstanceIndex].M * Vertex;
    const vec4 cVertex = view * vec4(vertex.xyz, 1.0);

    gl_Position = projection * cVertex;

    V = vec4(vertex.xyz, cVertex.w);
    SSV = cVertex;
    N = normalize(vec3(transforms[gl_InstanceIndex].IM * vec4(Normal, 0.0)));
    T = normalize(vec3(transforms[gl_InstanceIndex].IM * vec4(Tangent.xyz, 0.0)));
    B = cross(N, T) * Tangent.w;
    MID = MatID;
    TUV = UV;
}