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
    const vec3 vmin = node.bmin;
    const vec3 vmax = node.bmax;

    const vec3 t1 = (vmin - origin) * dir_inverse;
    const vec3 t2 = (vmax - origin) * dir_inverse;

    const vec3 tmin = min(t1, t2);
    const vec3 tmax = max(t1, t2);

    t_min = max(tmin[0], max(tmin[1], tmin[2]));
    t_max = min(tmax[0], min(tmax[1], tmax[2]));

    return t_max > 0.0 && t_max > t_min && t_min < t;
}