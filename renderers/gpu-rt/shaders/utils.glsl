#ifndef UTILS_H
#define UTILS_H

#define PI 3.14159265359f
#define TWOPI (2.0f * PI)
#define INVPI (1.0f / PI)
#define INV2PI (1.0f / (2.0f *PI))

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

uint PackNormal(const vec3 N)
{
    const float f = 65535.0f / sqrt(8.0f * N.z + 8.0f);
    return uint(N.x * f + 32767.0f) + (uint(N.y * f + 32767.0f) << 16);
}

vec3 UnpackNormal(uint p)
{
    vec4 nn = vec4(float(p & 65535u) * (2.0f / 65535.0f), float(p >> 16) * (2.0f / 65535.0f), 0, 0);
    nn += vec4(-1, -1, 1, -1);
    float l = dot(nn.xyz, -nn.xyz);
    nn.z = l, l = sqrt(l), nn.x *= l, nn.y *= l;
    return vec3(nn) * 2.0f + vec3(0, 0, -1);
}

// alternative method
uint PackNormal2(vec3 N)
{
    // simple, and good enough discrimination of normals for filtering.
    const uint x = clamp(uint((N.x + 1) * 511), 0u, 1023u);
    const uint y = clamp(uint((N.y + 1) * 511), 0u, 1023u);
    const uint z = clamp(uint((N.z + 1) * 511), 0u, 1023u);
    return (x << 2u) + (y << 12u) + (z << 22u);
}

vec3 UnpackNormal2(uint pi)
{
    const uint x = (pi >> 2u) & 1023u;
    const uint y = (pi >> 12u) & 1023u;
    const uint z = pi >> 22u;
    return vec3(x * (1.0f / 511.0f) - 1, y * (1.0f / 511.0f) - 1, z * (1.0f / 511.0f) - 1);
}

vec3 DiffuseReflectionUniform(const float r0, const float r1)
{
    const float term1 = TWOPI * r0, term2 = sqrt(1 - r1 * r1);
    float s = sin(term1);
    float c = cos(term1);
    return vec3(c * term2, s * term2, r1);
}

vec3 DiffuseReflectionCosWeighted(const float r0, const float r1)
{
    const float term1 = TWOPI * r0;
    const float term2 = sqrt(1.0 - r1);
    const float s = sin(term1);
    const float c = cos(term1);
    return vec3(c * term2, s * term2, sqrt(r1));
}

    #endif