bool intersect(const RTTriangle triangle, const vec3 origin, const vec3 direction, float t_min, inout float t)
{
    const vec3 v0 = triangle.v0;
    const vec3 v1 = triangle.v1;
    const vec3 v2 = triangle.v2;

    const vec3 edge1 = v1 - v0;
    const vec3 edge2 = v2 - v0;

    const vec3 h = cross(direction, edge2);
    const float a = dot(edge1, h);
    if (a > -0.0001 && a < 0.0001) {
        return false;
    }

    const float f = 1.0 / a;
    const vec3 s = origin - v0;
    const float u = f * dot(s, h);
    if (u < 0.0 || u > 1.0) {
        return false;
    }

    const vec3 q = cross(s, edge1);
    const float v = f * dot(direction, q);
    if (v < 0.0 || (u + v) > 1.0) {
        return false;
    }

    const float _t = f * dot(edge2, q);
    if (_t > t_min && _t < t) {
        t = _t;
        return true;
    }

    return false;
}

bool intersect_node(const BVHNode node, const vec3 origin, const vec3 dir_inverse, const float t, inout float t_min, inout float t_max) {

    float tmin = (node.bmin_x - origin.x) * dir_inverse.x;
    float tmax = (node.bmax_x - origin.x) * dir_inverse.x;

    t_min = min(tmin, tmax);
    t_max = max(tmin, tmax);

    tmin = (node.bmin_y - origin.y) * dir_inverse.y;
    tmax = (node.bmax_y - origin.y) * dir_inverse.y;

    t_min = max(t_min, min(tmin, tmax));
    t_max = min(t_max, max(tmin, tmax));

    tmin = (node.bmin_z - origin.z) * dir_inverse.z;
    tmax = (node.bmax_z - origin.z) * dir_inverse.z;

    t_min = max(t_min, min(tmin, tmax));
    t_max = min(t_max, max(tmin, tmax));

    return t_max > 0.0 && t_max > t_min && t_min < t;
}

void swapf(inout float a, inout float b) {
    float tmp = a;
    a = b;
    b = a;
}

void swapu(inout uint a, inout uint b) {
    uint tmp = a;
    a = b;
    b = a;
}
bool intersect_mnode(const MBVHNode node, const vec3 origin, const vec3 dir_inverse, const float t, inout vec4 tmin, inout bvec4 result) {
    vec4 t1 = (node.min_x - origin.x) * dir_inverse.x;
    vec4 t2 = (node.max_x - origin.x) * dir_inverse.x;

    tmin = min(t1, t2);
    vec4 tmax = max(t1, t2);

    t1 = (node.min_y - origin.y) * dir_inverse.y;
    t2 = (node.max_y - origin.y) * dir_inverse.y;

    tmin = max(tmin, min(t1, t2));
    tmax = min(tmax, max(t1, t2));

    t1 = (node.min_z - origin.z) * dir_inverse.z;
    t2 = (node.max_z - origin.z) * dir_inverse.z;

    tmin = max(tmin, min(t1, t2));
    tmax = min(tmax, max(t1, t2));

    const bvec4 ge = greaterThanEqual(tmax, tmin);
    const bvec4 lt = lessThan(tmin, vec4(t));
    for (int i = 0; i < 4; i++) {
        result[i] = ge[i] && lt[i];
    }

    if (!any(result)) {
        return false;
    }

    tmin.x = intBitsToFloat((floatBitsToInt(tmin.x) & 0xFFFFFFFC));
    tmin.y = intBitsToFloat(((floatBitsToInt(tmin.y) & 0xFFFFFFFC) | 1));
    tmin.z = intBitsToFloat(((floatBitsToInt(tmin.z) & 0xFFFFFFFC) | 2));
    tmin.w = intBitsToFloat(((floatBitsToInt(tmin.w) & 0xFFFFFFFC) | 3));

    float tmp;
    if (tmin[0] > tmin[1]) {
        tmp = tmin[0];
        tmin[0] = tmin[1];
        tmin[1] = tmp;
    }
    if (tmin[2] > tmin[3]) {
        tmp = tmin[2];
        tmin[2] = tmin[3];
        tmin[3] = tmp;
    }
    if (tmin[0] > tmin[2]) {
        tmp = tmin[0];
        tmin[0] = tmin[2];
        tmin[2] = tmp;
    }
    if (tmin[1] > tmin[3]) {
        tmp = tmin[1];
        tmin[1] = tmin[3];
        tmin[3] = tmp;
    }
    if (tmin[2] > tmin[3]) {
        tmp = tmin[2];
        tmin[2] = tmin[3];
        tmin[3] = tmp;
    }

    return true;
}