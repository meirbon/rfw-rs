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

void CLAMPINTENSITY(inout vec3 contribution, const float clampValue)
{
    const float v = max(contribution.x, max(contribution.y, contribution.z));
    if (v > clampValue)
    {
        const float m = clampValue / v;
        contribution.xyz = contribution.xyz * m;
    }
}

// Ray Tracing Gems 1: chapter 6; https://www.realtimerendering.com/raytracinggems/
vec3 safe_origin(vec3 O, vec3 R, vec3 N, float epsilon)
{
    const vec3 _N = dot(N, R) > 0 ? N : -N;
    ivec3 of_i = ivec3(256.0f * _N);
    vec3 p_i = vec3(intBitsToFloat(floatBitsToInt(O.x) + ((O.x < 0) ? -of_i.x : of_i.x)), intBitsToFloat(floatBitsToInt(O.y) + ((O.y < 0) ? -of_i.y : of_i.y)),
    intBitsToFloat(floatBitsToInt(O.z) + ((O.z < 0) ? -of_i.z : of_i.z)));

    return vec3(abs(O.x) < (1.0f / 32.0f) ? O.x + (1.0f / 65536.0f) * _N.x : p_i.x, abs(O.y) < (1.0f / 32.0f) ? O.y + (1.0f / 65536.0f) * _N.y : p_i.y,
    abs(O.z) < (1.0f / 32.0f) ? O.z + (1.0f / 65536.0f) * _N.z : p_i.z);
}


    #endif