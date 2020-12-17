#include "material.glsl"

#define PI 3.14159265359
#define INVPI 1.0 / PI

float GTR1(float NDotH, float a)
{
    if (a >= 1)
    return INVPI;
    float a2 = a * a;
    float t = 1 + (a2 - 1) * NDotH * NDotH;
    return (a2 - 1) / (PI * log(a2) * t);
}

float GTR2(float NDotH, float a)
{
    float a2 = a * a;
    float t = 1.0f + (a2 - 1.0f) * NDotH * NDotH;
    return a2 / (PI * t * t);
}

float SmithGGX(const float NDotv, const float alphaG)
{
    float a = alphaG * alphaG;
    float b = NDotv * NDotv;
    return 1 / (NDotv + sqrt(a + b - a * b));
}

float Fr(const float VDotN, const float eio)
{
    float SinThetaT2 = (eio * eio) * (1.0f - VDotN * VDotN);
    if (SinThetaT2 > 1.0f)
    return 1.0f;// TIR
    float LDotN = sqrt(1.0f - SinThetaT2);
    float eta = 1.0f / eio;
    float r1 = (VDotN - eta * LDotN) / (VDotN + eta * LDotN);
    float r2 = (LDotN - eta * VDotN) / (LDotN + eta * VDotN);
    return 0.5f * ((r1 * r1) + (r2 * r2));
}

float SchlickFresnel(const float u)
{
    float m = clamp(1.0f - u, 0.0f, 1.0f);
    return float(m * m) * (m * m) * m;
}

vec3 BSDFEval(ShadingData data, const vec3 N, const vec3 wo, const vec3 wi)
{
    float NDotL = dot(N, wi);
	float NDotV = dot(N, wo);
	vec3 H = normalize(wi + wo);
	float NDotH = dot(N, H);
	float LDotH = dot(wi, H);
	vec3 Cdlin = data.color.xyz;
	float Cdlum = .3f * Cdlin.x + .6f * Cdlin.y + .1f * Cdlin.z; // luminance approx.
	vec3 Ctint = Cdlum > 0.0f ? Cdlin / Cdlum : vec3(1.0f);	  // normalize lum. to isolate hue+sat
	vec3 Cspec0 = mix(data.specular * .08f * mix(vec3(1.0f), Ctint, data.specular_tint), Cdlin, data.metallic);
	vec3 bsdf = vec3(0);
	vec3 brdf = vec3(0);
	if (data.transmission > 0.0f)
	{
		// evaluate BSDF
		if (NDotL <= 0)
		{
			// transmission Fresnel
			float F = Fr(NDotV, data.eta);
			bsdf = vec3((1.0f - F) / abs(NDotL) * (1.0f - data.metallic) * data.transmission);
		}
		else
		{
			// specular lobe
			float a = data.roughness;
			float Ds = GTR2(NDotH, a);

			// Fresnel term with the microfacet normal
			float FH = Fr(LDotH, data.eta);
			vec3 Fs = mix(Cspec0, vec3(1.0f), FH);
			float Gs = SmithGGX(NDotV, a) * SmithGGX(NDotL, a);
			bsdf = (Gs * Ds) * Fs;
		}
	}
	if (data.transmission < 1.0f)
	{
		// evaluate BRDF
		if (NDotL <= 0)
		{
			if (data.subsurface > 0.0f)
			{
				// take sqrt to account for entry/exit of the ray through the medium
				// this ensures transmitted light corresponds to the diffuse model
				vec3 s = sqrt(data.color.xyz);
				float FL = SchlickFresnel(abs(NDotL)), FV = SchlickFresnel(NDotV);
				float Fd = (1.0f - 0.5f * FL) * (1.0f - 0.5f * FV);
				brdf = INVPI * s * data.subsurface * Fd * (1.0f - data.metallic);
			}
		}
		else
		{
			// specular
			float a = data.roughness;
			float Ds = GTR2(NDotH, a);

			// Fresnel term with the microfacet normal
			float FH = SchlickFresnel(LDotH);
			vec3 Fs = mix(Cspec0, vec3(1), FH);
			float Gs = SmithGGX(NDotV, a) * SmithGGX(NDotL, a);

			// Diffuse fresnel - go from 1 at normal incidence to .5 at grazing
			// and mix in diffuse retro-reflection based on roughness
			float FL = SchlickFresnel(NDotL), FV = SchlickFresnel(NDotV);
			float Fd90 = 0.5 + 2.0f * LDotH * LDotH * a;
			float Fd = mix(1.0f, Fd90, FL) * mix(1.0f, Fd90, FV);

			// clearcoat (ior = 1.5 -> F0 = 0.04)
			float Dr = GTR1(NDotH, mix(.1, .001, data.clearcoat_gloss));
			float Fc = mix(.04f, 1.0f, FH);
			float Gr = SmithGGX(NDotL, .25) * SmithGGX(NDotV, .25);

			brdf = INVPI * Fd * Cdlin * (1.0f - data.metallic) * (1.0f - data.subsurface) + Gs * Fs * Ds + data.clearcoat * Gr * Fc * Dr;
		}
	}

	return mix(brdf, bsdf, data.transmission);
}


vec3 evalLighting(ShadingData data, vec3 N, vec3 wo, vec3 wi)
{
    return BSDFEval(data, N, wo, wi);
}