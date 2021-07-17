#include <metal_stdlib>

#include "structs.h"

using namespace metal;

struct ColorInOut
{
	float4 position [[position]];
	float4 color;
	float2 uv;
	uint tex;
	uint _dummy;
};

struct Texture
{
	texture2d<float> tex [[id(0)]];
};

struct Scene
{
	const device Vertex3D *vertices [[id(VERTICES_ARG_INDEX)]];
	const device Vertex2D *vertices_2d [[id(VERTICES_2D_ARG_INDEX)]];
	const device Texture *textures [[id(TEXTURES_ARG_INDEX)]];
	const device DeviceMaterial *materials [[id(MATERIALS_ARG_INDEX)]];
	const device InstanceTransform *instances [[id(INSTANCES_ARG_INDEX)]];
	const device simd_float4x4 *instances_2d [[id(INSTANCES_2D_ARG_INDEX)]];
};

// vertex shader function
vertex ColorInOut triangle_vertex_2d(const device Scene &scene [[buffer(0)]],
									 const device UniformCamera *camera [[buffer(1)]], unsigned int vid [[vertex_id]],
									 unsigned int i_id [[instance_id]])
{
	ColorInOut out;

	const device Vertex2D &v = scene.vertices_2d[vid];
	const device simd_float4x4 &t = scene.instances_2d[i_id];

	out.position = camera->matrix_2d * t * float4(v.v_x, v.v_y, v.v_z, 1.0);
	out.color = float4(v.c_r, v.c_g, v.c_b, v.c_a);
	out.uv = float2(v.u, v.v);
	out.tex = v.tex;

	return out;
}

// fragment shader function
fragment float4 triangle_fragment_2d(ColorInOut in [[stage_in]], const device Scene &scene [[buffer(0)]])
{
	auto color = in.color;
	if (in.tex > 0)
	{
		constexpr sampler textureSampler(mag_filter::linear, min_filter::linear);
		color = color * scene.textures[in.tex].tex.sample(textureSampler, in.uv);
	}

	if (color.w <= 0.0)
		discard_fragment();

	return color;
}

struct VertexInOut
{
	float4 position [[position]];
	half4 color;
	half3 normal;
	ushort mat_id;
	float2 uv;
};

// vertex shader function
vertex VertexInOut triangle_vertex(const device Scene &scene [[buffer(0)]],
								   const device UniformCamera *camera [[buffer(1)]], unsigned int vid [[vertex_id]],
								   unsigned int i_id [[instance_id]])
{
	VertexInOut out;

	const device auto &v = scene.vertices[vid];
	const device auto &t = scene.instances[i_id];

	const float3 normal = (t.normal_matrix * float4(v.n_x, v.n_y, v.n_z, 0.0)).xyz;

	out.position = camera->combined * t.matrix * float4(v.v_x, v.v_y, v.v_z, v.v_w);
	out.color = (half4)(float4(normalize(normal.xyz), 0.2));
	out.normal = (half3)normal;
	out.uv = float2(v.u, v.v);
	out.mat_id = (ushort)v.mat_id;

	return out;
}

// fragment shader function
fragment half4 triangle_fragment(VertexInOut in [[stage_in]], const device Scene &scene [[buffer(0)]])
{
	const float4 color = float4(scene.materials[in.mat_id].c_r, scene.materials[in.mat_id].c_g,
								scene.materials[in.mat_id].c_b, scene.materials[in.mat_id].c_a);
	return (half4)color * half4(in.normal, 1.0);
}