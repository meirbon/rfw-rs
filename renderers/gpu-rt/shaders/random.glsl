#include "utils.glsl"
#ifndef RANDOM_H
#define RANDOM_H

uint wang_hash(uint s)
{
    s = (s ^ 61u) ^ (s >> 16u);
    s *= 9u;
    s = s ^ (s >> 4u);
    s *= 0x27d4eb2d;
    s = s ^ (s >> 15u);
    return s;
}

uint randi(inout uint s)
{
    s ^= s << 13;
    s ^= s >> 17;
    s ^= s << 5;
    return s;
}

float randf(inout uint s) { return randi(s) * 2.3283064365387e-10f; }

vec3 sample_hemisphere(const float r1, const float r2)
{
    const float r = sqrt(r1);
    const float theta = 2.0f * 3.14159265359f * r2;
    float x = cos(theta);
    float y = sin(theta);
    x = x * r;
    y = y * r;
    return vec3(x, y, sqrt(1.0f - r1));
}
#endif