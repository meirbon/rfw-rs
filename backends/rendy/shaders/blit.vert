#version 450

layout(location = 0) out vec2 UV;

const vec2 positions[6] = vec2[6](
// Upper left triangle
vec2(-1.0, -1.0),
vec2(1.0, -1.0),
vec2(-1.0, 1.0),
// Lower right triangle
vec2(-1.0, 1.0),
vec2(1.0, -1.0),
vec2(1.0, 1.0)
);

const vec2 uv[6] = vec2[6](
// Upper left triangle
vec2(0.0, 1.0),
vec2(1.0, 1.0),
vec2(0.0, 0.0),

// Lower right triangle
vec2(0.0, 0.0),
vec2(1.0, 1.0),
vec2(1.0, 0.0)
);

void main() {
    UV = uv[gl_VertexIndex % 6];
    gl_Position = vec4(positions[gl_VertexIndex  % 6], 0.0, 1.0);
}