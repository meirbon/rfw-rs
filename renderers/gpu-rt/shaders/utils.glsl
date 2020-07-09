#ifndef UTILS_H
#define UTILS_H

void create_tangent_space(const vec3 N, inout vec3 T, inout vec3 B)
{
    const float sign = sign(N.z);
    const float a = -1.0f / (sign + N.z);
    const float b = N.x * N.y * a;
    T = vec3(1.0f + sign * N.x * N.x * a, sign * b, -sign * N.x);
    B = vec3(b, sign + N.y * N.y * a, -N.y);
}

vec3 tangent_to_world(const vec3 s, const vec3 N, const vec3 T, const vec3 B) { return T * s.x + B * s.y + N * s.z; }

vec3 world_to_tangent(const vec3 s, const vec3 N, const vec3 T, const vec3 B) { return vec3(dot(T, s), dot(B, s), dot(N, s)); }

#endif