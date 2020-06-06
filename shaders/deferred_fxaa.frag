#version 450

layout(location = 0) in vec2 UV;

layout(set = 0, binding = 0) uniform sampler Sampler;
layout(set = 0, binding = 1) uniform texture2D Albedo;
layout(set = 0, binding = 2) uniform texture2D Normal;
layout(set = 0, binding = 3) uniform texture2D WorldPos;
layout(set = 0, binding = 4) uniform texture2D Radiance;
layout(set = 0, binding = 6) uniform texture2D RenderView;

layout(location = 0) out vec4 OutColor;

// http://www.geeks3d.com/20110405/fxaa-fast-approximate-anti-aliasing-demo-glsl-opengl-test-radeon-geforce/3/
const float FXAA_SPAN_MAX = 8.0;
const float FXAA_REDUCE_MUL = 1.0 / 8.0;
const float FXAA_SUBPIX_SHIFT = 1.0 / 4.0;

// Output of FxaaVertexShader interpolated across screen.
// Input texture.
// Constant {1.0/frameWidth, 1.0/frameHeight}.
vec3 FxaaPixelShader(vec4 posPos, vec2 rcpFrame)
{
    /*---------------------------------------------------------*/
    #define FXAA_REDUCE_MIN   (1.0 / 64.0)
    /*---------------------------------------------------------*/

    vec3 rgbNW = textureLod(sampler2D(RenderView, Sampler), posPos.zw, 0.0).xyz;
    vec3 rgbNE = textureLodOffset(sampler2D(RenderView, Sampler), posPos.zw, 0.0, ivec2(1, 0)).xyz;
    vec3 rgbSW = textureLodOffset(sampler2D(RenderView, Sampler), posPos.zw, 0.0, ivec2(0, 1)).xyz;
    vec3 rgbSE = textureLodOffset(sampler2D(RenderView, Sampler), posPos.zw, 0.0, ivec2(1, 1)).xyz;
    vec3 rgbM  = textureLod(sampler2D(RenderView, Sampler), posPos.xy, 0.0).xyz;

    vec3 luma = vec3(0.299, 0.587, 0.114);
    float lumaNW = dot(rgbNW, luma);
    float lumaNE = dot(rgbNE, luma);
    float lumaSW = dot(rgbSW, luma);
    float lumaSE = dot(rgbSE, luma);
    float lumaM  = dot(rgbM, luma);

    float lumaMin = min(lumaM, min(min(lumaNW, lumaNE), min(lumaSW, lumaSE)));
    float lumaMax = max(lumaM, max(max(lumaNW, lumaNE), max(lumaSW, lumaSE)));

    vec2 dir;
    dir.x = -((lumaNW + lumaNE) - (lumaSW + lumaSE));
    dir.y =  ((lumaNW + lumaSW) - (lumaNE + lumaSE));

    float dirReduce = max((lumaNW + lumaNE + lumaSW + lumaSE) * (0.25 * FXAA_REDUCE_MUL), FXAA_REDUCE_MIN);
    float rcpDirMin = 1.0 / (min(abs(dir.x), abs(dir.y)) + dirReduce);
    dir = min(vec2(FXAA_SPAN_MAX, FXAA_SPAN_MAX), max(vec2(-FXAA_SPAN_MAX, -FXAA_SPAN_MAX), dir * rcpDirMin)) * rcpFrame.xy;

    vec3 rgbA = 0.5 * (textureLod(sampler2D(RenderView, Sampler), posPos.xy + dir * (1.0/3.0 - 0.5), 0.0).xyz + textureLod(sampler2D(RenderView, Sampler), posPos.xy + dir * (2.0/3.0 - 0.5), 0.0).xyz);
    vec3 rgbB = rgbA * 0.5 + (1.0/4.0) * (textureLod(sampler2D(RenderView, Sampler), posPos.xy + dir * (0.0/3.0 - 0.5), 0.0).xyz + textureLod(sampler2D(RenderView, Sampler), posPos.xy + dir * (3.0/3.0 - 0.5), 0.0).xyz);
    float lumaB = dot(rgbB, luma);

    if ((lumaB < lumaMin) || (lumaB > lumaMax))
    {
        return rgbA;
    }

    return rgbB;
}

void main() {
    const vec2 resolution = textureSize(sampler2D(RenderView, Sampler), 0).xy;
    const vec2 rcpFrame = 1.0 / resolution;

    vec4 posPos;
    posPos.xy = UV.xy;
    posPos.zw = UV.xy - (rcpFrame * (0.5 + FXAA_SUBPIX_SHIFT));
    const vec3 color = FxaaPixelShader(posPos, rcpFrame);

    OutColor = vec4(color, 1.0);
}