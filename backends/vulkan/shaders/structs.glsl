struct PointLight {
    vec4 position_energy;
    vec4 radiance;
};

struct AreaLight {
    vec4 position_energy;
    vec4 normal_area;
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
