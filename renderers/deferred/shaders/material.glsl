#ifndef MATERIAL_H
#define MATERIAL_H

struct Material {
    vec4 color;
    vec4 absorption;
    vec4 specular;
    uvec4 parameters;

    uint flags;
    int diffuse_map;
    int normal_map;
    int metallic_roughness_map;

    int emissive_map;
    int sheen_map;
    int _dummy0;
    int _dummy1;
};

struct ShadingData {
    vec3 color;
    vec3 absorption;
    vec3 specular;
    float metallic;
    float subsurface;
    float specular_f;
    float roughness;
    float specular_tint;
    float anisotropic;
    float sheen;
    float sheen_tint;
    float clearcoat;
    float clearcoat_gloss;
    float transmission;
    float eta;
    float custom0;
    float custom1;
    float custom2;
    float custom3;
};

#define CHAR2FLT(x, s) ((float(((x >> s) & 255))) * (1.0f / 255.0f))

#define HAS_DIFFUSE_MAP(flags) ((flags & (1 << 0)) > 0)
#define HAS_NORMAL_MAP(flags) ((flags & (1 << 1)) > 0)
#define HAS_METAL_ROUGH_MAP(flags) ((flags & (1 << 2)) > 0)
#define HAS_EMISSIVE_MAP(flags) ((flags & (1 << 4)) > 0)
#define HAS_SHEEN_MAP(flags) ((flags & (1 << 5)) > 0)

#define IS_EMISSIVE(color) (color.x > 1.0 || color.y > 1.0 || color.z > 1.0)

#define METALLIC(parameters) CHAR2FLT(parameters.x, 0)
#define SUBSURFACE(parameters) CHAR2FLT(parameters.x, 8)
#define SPECULAR(parameters) CHAR2FLT(parameters.x, 16)
#define ROUGHNESS(parameters) (max(0.01f, CHAR2FLT(parameters.x, 24)))

#define SPECTINT(parameters) CHAR2FLT(parameters.y, 0)
#define ANISOTROPIC(parameters) CHAR2FLT(parameters.y, 8)
#define SHEEN(parameters) CHAR2FLT(parameters.y, 16)
#define SHEENTINT(parameters) CHAR2FLT(parameters.y, 24)

#define CLEARCOAT(parameters) CHAR2FLT(parameters.z, 0)
#define CLEARCOATGLOSS(parameters) CHAR2FLT(parameters.z, 8)
#define TRANSMISSION(parameters) CHAR2FLT(parameters.z, 16)
#define ETA(parameters) CHAR2FLT(parameters.z, 24)

#define CUSTOM0(parameters) CHAR2FLT(parameters.w, 0)
#define CUSTOM1(parameters) CHAR2FLT(parameters.w, 8)
#define CUSTOM2(parameters) CHAR2FLT(parameters.w, 16)
#define CUSTOM3(parameters) CHAR2FLT(parameters.w, 24)

ShadingData extractParameters(const vec3 color, const vec3 absorption, const vec3 specular, const uvec4 parameters) {
    ShadingData data;
    data.color = color;
    data.absorption = absorption;
    data.specular = specular;
    data.metallic = METALLIC(parameters);
    data.subsurface = SUBSURFACE(parameters);
    data.specular_f = SPECULAR(parameters);
    data.roughness = ROUGHNESS(parameters);
    data.specular_tint = SPECTINT(parameters);
    data.anisotropic = ANISOTROPIC(parameters);
    data.sheen = SHEEN(parameters);
    data.sheen_tint = SHEENTINT(parameters);
    data.clearcoat = CLEARCOAT(parameters);
    data.clearcoat_gloss = CLEARCOATGLOSS(parameters);
    data.transmission = TRANSMISSION(parameters);
    data.eta = ETA(parameters);
    data.custom0 = CUSTOM0(parameters);
    data.custom1 = CUSTOM1(parameters);
    data.custom2 = CUSTOM2(parameters);
    data.custom3 = CUSTOM3(parameters);
    return data;
}

#endif