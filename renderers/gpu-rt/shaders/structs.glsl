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
    vec3 v1;
    float tu1;
    vec3 v2;
    float tu2;
    vec3 gn;
    float tv0;
    vec3 n0;
    float tv1;
    vec3 n1;
    float tv2;
    vec3 n2;
    int id;
    int light_id;
    int mat_id;
    int _dummy0;
    int _dummy1;
};

struct InstanceDescriptor {
    mat4 matrix;
    mat4 inverse;
    mat4 normal;

    uint bvh_offset;
    uint mbvh_offset;
    uint triangle_offset;
    uint prim_index_offset;
};