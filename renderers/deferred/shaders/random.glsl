#define M_PI 3.1415926535897932384626433832795
#define PI   3.1415926535897932384626433832795
#define PI2  6.2831853071795864769252867665590

uint WangHash(uint s)
{
    s = (s ^ 61u) ^ (s >> 16u);
    s *= 9u;
    s = s ^ (s >> 4u);
    s *= 0x27d4eb2du, s = s ^ (s >> 15u);
    return s;
}

uint RandomInt(inout uint s)
{
    s ^= s << 13u;
    s ^= s >> 17u;
    s ^= s << 5u;
    return s;
}

float RandomFloat(inout uint s) { return RandomInt(s) * 2.3283064365387e-10f; }

vec2 Hammersley(uint i, uint N)
{
    return vec2(float(i) / float(N), float(bitfieldReverse(i)) * 2.3283064365386963e-10);
}