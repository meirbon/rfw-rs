#version 450

layout(location = 0) in vec2 UV;

layout(set = 0, binding = 0) uniform sampler Sampler;
layout(set = 1, binding = 0) uniform texture2D ShadowMap;


layout(location = 0) out vec4 Color;


void main() {
	vec2 direction = vec2(1, 0);
	vec2 resolution = textureSize(sampler2D(ShadowMap, Sampler), 0).xy;

	vec4 color = vec4(0.0);
	vec2 off1 = vec2(1.411764705882353) * direction;
	vec2 off2 = vec2(3.2941176470588234) * direction;
	vec2 off3 = vec2(5.176470588235294) * direction;
	color += texture(sampler2D(ShadowMap, Sampler), UV) * 0.1964825501511404;
	color += texture(sampler2D(ShadowMap, Sampler), UV + (off1 / resolution)) * 0.2969069646728344;
	color += texture(sampler2D(ShadowMap, Sampler), UV - (off1 / resolution)) * 0.2969069646728344;
	color += texture(sampler2D(ShadowMap, Sampler), UV + (off2 / resolution)) * 0.09447039785044732;
	color += texture(sampler2D(ShadowMap, Sampler), UV - (off2 / resolution)) * 0.09447039785044732;
	color += texture(sampler2D(ShadowMap, Sampler), UV + (off3 / resolution)) * 0.010381362401148057;
	color += texture(sampler2D(ShadowMap, Sampler), UV - (off3 / resolution)) * 0.010381362401148057;
	Color = color;
}