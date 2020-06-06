#define PCF 0
#define VSM 1

float linearizeDepth(float depth, float farPlane) {
    float nearPlane = 0.1;
    return (2.0 * nearPlane) / (farPlane + nearPlane - depth * (farPlane - nearPlane));
}

struct LightInfo {
    mat4 MP;
    vec4 PosRange;
    
    vec4 padding0;
    vec4 padding1;
    vec4 padding2;
    mat4 padding3;
    mat4 padding4;
};

struct PointLight {
    vec4 position_energy;
    vec4 radiance;
};

struct AreaLight {
    vec4 position_energy;
    vec4 normal_tri_id;
    vec4 vertex0_inst_id;
    float vertex1_x;
    float vertex1_y;
    float vertex1_z;
    float radiance_x;
    float radiance_y;
    float radiance_z;
    float vertex2_x;
    float vertex2_y;
    float vertexw_z;
};

struct SpotLight {
    vec4 position_cos_inner;
    vec4 radiance_cos_outer;
    vec4 direction_energy;
};

struct DirectionalLight {
    vec4 direction_energy;
    vec4 radiance;
};
