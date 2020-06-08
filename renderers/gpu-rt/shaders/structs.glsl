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
    float min_x;
    float min_y;
    float min_z;
    float max_x;
    float max_y;
    float max_z;
    int left_first;
    int count;
};

struct MBVHNode {
    vec4 min_x;
    vec4 min_y;
    vec4 min_z;
    vec4 max_x;
    vec4 max_y;
    vec4 max_z;
    ivec4 children;
    ivec4 counts;
};