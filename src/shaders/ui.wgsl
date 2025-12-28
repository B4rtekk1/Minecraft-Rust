struct Uniforms {
    view_proj: mat4x4<f32>,
    sun_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_position: vec3<f32>,
    _padding: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var texture_atlas: texture_2d_array<f32>;
@group(0) @binding(2)
var texture_sampler: sampler;
@group(0) @binding(3)
var shadow_map: texture_depth_2d;
@group(0) @binding(4)
var shadow_sampler: sampler_comparison;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) tex_index: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) tex_index: f32,
};

@vertex
fn vs_ui(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(model.position.xy, 0.0, 1.0);
    out.world_pos = model.position;
    out.normal = model.normal;
    out.color = model.color;
    out.uv = model.uv;
    out.tex_index = model.tex_index;
    return out;
}

@fragment
fn fs_ui(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
