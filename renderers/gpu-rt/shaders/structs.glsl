struct CameraView {
    vec3 position;
    int path_length;

    vec4 right;
    vec4 up;
    vec4 p1;
    
    float lens_size;
    float spread_angle;
    float epsilon;
    float inv_width;

    float inv_height;
    int path_count;
    int extensionId;
    int shadowId;

    int width;
    int height;
    int sample_count;
    float clamp_value;

    int point_light_count;
    int area_light_count;
    int spot_light_count;
    int directional_light_count;
};

struct BVHNode {
    float bmin_x;
    float bmin_y;
    float bmin_z;
    float bmax_x;
    float bmax_y;
    float bmax_z;
    int left_first;
    int count;
};

struct MBVHNode {
    vec4 min_x;
    vec4 max_x;
    vec4 min_y;
    vec4 max_y;
    vec4 min_z;
    vec4 max_z;
    ivec4 children;
    ivec4 counts;
};

struct RTTriangle {
    vec3 v0;
    float tu0;
    // 16

    vec3 v1;
    float tu1;
    // 32
    
    vec3 v2;
    float tu2;
    // 48
    
    vec3 gn;
    float tv0;
    // 64
    
    vec3 n0;
    float tv1;
    // 80
    
    vec3 n1;
    float tv2;
    // 96
    
    vec3 n2;
    int id;
    // 112
    
    vec4 T0;
    // 128
    vec4 T1;
    // 144
    vec4 T2;
    // 160

    int light_id;
    int mat_id;
    float lod;
    float area;
    // 176
};

struct InstanceDescriptor {
    uint bvh_offset;
    uint mbvh_offset;
    uint triangle_offset;
    uint prim_index_offset;
    vec4 _dummy0;
    vec4 _dummy1;
    vec4 _dummy2;

    mat4 matrix;
    mat4 inverse;
    mat4 normal;
};

struct PointLight {
    vec3 position;
    float energy;

    vec3 radiance;
    int dummy;
};

struct SpotLight {
    vec3 position;
    float cos_inner;

    vec3 radiance;
    float cos_outer;

    vec3 direction;
    float energy;
};


struct AreaLight {
    vec3 position;
    float energy;

    vec3 normal;
    float area;

    vec3 vertex0;
    int inst_id;

    vec3 vertex1;
    int _dummy0;
    
    vec3 radiance;
    int _dummy1;

    vec3 vertex2;
    int _dummy2;
};

struct DirectionalLight {
    vec3 direction;
    float energy;

    vec3 radiance;
    int dummy;
};

struct PotentialContribution {
    vec4 O;
	vec4 D;
	vec4 E_pixelId;
};

struct Material {
    vec4 color;
    vec4 absorption;
    vec4 specular;
    uvec4 parameters;

    uint flags;
    int diffuse_map;
    int normal_map;
    int roughness_map;

    int emissive_map;
    int sheen_map;
    int _dummy0;
    int _dummy1;
};

struct ShadingData {
    vec3 color;
    vec3 absorption;
    vec3 specular;
    float metallic;
    float subsurface;
    float specular_f;
    float roughness;
    float specular_tint;
    float anisotropic;
    float sheen;
    float sheen_tint;
    float clearcoat;
    float clearcoat_gloss;
    float transmission;
    float eta;
    float custom0;
    float custom1;
    float custom2;
    float custom3;
};

#define CHAR2FLT(x, s) ((float( ((x >> s) & 255)) ) * (1.0f / 255.0f))

#define HAS_DIFFUSE_MAP(flags) ((flags & (1 << 0)) > 0)
#define HAS_NORMMAL_MAP(flags) ((flags & (1 << 1)) > 0)
#define HAS_ROUGHNESS_MAP(flags) ((flags & (1 << 2)) > 0)
#define HAS_METALLIC_MAP(flags) ((flags & (1 << 3)) > 0)
#define HAS_EMISSIVE_MAP(flags) ((flags & (1 << 4)) > 0)
#define HAS_SHEEN_MAP(flags) ((flags & (1 << 5)) > 0)

#define IS_EMISSIVE(color) (color.x > 1.0 || color.y > 1.0 || color.z > 1.0)

#define METALLIC(parameters) CHAR2FLT(parameters.x, 0)
#define SUBSURFACE(parameters) CHAR2FLT(parameters.x, 8)
#define SPECULAR(parameters) CHAR2FLT(parameters.x, 16)
#define ROUGHNESS(parameters) (max(0.01f, CHAR2FLT( parameters.x, 24 )))

#define SPECTINT(parameters) CHAR2FLT(parameters.y, 0)
#define ANISOTROPIC(parameters) CHAR2FLT(parameters.y, 8)
#define SHEEN(parameters) CHAR2FLT(parameters.y, 16)
#define SHEENTINT(parameters) CHAR2FLT(parameters.y, 24)

#define CLEARCOAT(parameters) CHAR2FLT(parameters.z, 0)
#define CLEARCOATGLOSS(parameters) CHAR2FLT(parameters.z, 8)
#define TRANSMISSION(parameters) CHAR2FLT(parameters.z, 16)
#define ETA(parameters) CHAR2FLT(parameters.z, 24)

#define CUSTOM0(parameters) CHAR2FLT(parameters.w, 0)
#define CUSTOM1(parameters) CHAR2FLT(parameters.w, 8)
#define CUSTOM2(parameters) CHAR2FLT(parameters.w, 16)
#define CUSTOM3(parameters) CHAR2FLT(parameters.w, 24)

ShadingData extractParameters(const vec3 color, const vec3 absorption, const vec3 specular, const uvec4 parameters) {
    ShadingData data;
    data.color = color;
    data.absorption = absorption;
    data.specular = specular;
    data.metallic = METALLIC(parameters);
    data.subsurface = SUBSURFACE(parameters);
    data.specular_f = SPECULAR(parameters);
    data.roughness = ROUGHNESS(parameters);
    data.specular_tint = SPECTINT(parameters);
    data.anisotropic = ANISOTROPIC(parameters);
    data.sheen = SHEEN(parameters);
    data.sheen_tint = SHEENTINT(parameters);
    data.clearcoat = CLEARCOAT(parameters);
    data.clearcoat_gloss = CLEARCOATGLOSS(parameters);
    data.transmission = TRANSMISSION(parameters);
    data.eta = ETA(parameters);
    data.custom0 = CUSTOM0(parameters);
    data.custom1 = CUSTOM1(parameters);
    data.custom2 = CUSTOM2(parameters);
    data.custom3 = CUSTOM3(parameters);
    return data;
}