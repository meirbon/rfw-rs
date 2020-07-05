struct CameraView {
    vec4 position;
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
    int extra0;
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

struct Material {
    vec4 color;
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

#define HAS_DIFFUSE_MAP(flags) ((flags & (1 << 0)) > 0)
#define HAS_NORMMAL_MAP(flags) ((flags & (1 << 1)) > 0)
#define HAS_ROUGHNESS_MAP(flags) ((flags & (1 << 2)) > 0)
#define HAS_METALLIC_MAP(flags) ((flags & (1 << 3)) > 0)
#define HAS_EMISSIVE_MAP(flags) ((flags & (1 << 4)) > 0)
#define HAS_SHEEN_MAP(flags) ((flags & (1 << 5)) > 0)

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
    vec3 radiance;
    vec3 vertex;
};

struct DirectionalLight {
    vec3 direction;
    float energy;

    vec3 radiance;
    int dummy;
};
