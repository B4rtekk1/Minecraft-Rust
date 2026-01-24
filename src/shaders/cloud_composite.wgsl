// Cloud compositor shader
// Samples cloud texture and blends it with the sky

struct Uniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    csm_view_proj: array<mat4x4<f32>, 4>,
    csm_split_distances: vec4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_position: vec3<f32>,
    is_underwater: f32,
    screen_size: vec2<f32>,
    water_level: f32,
    reflection_mode: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var cloud_texture: texture_2d<f32>;
@group(0) @binding(2) var cloud_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle: 3 vertices covering entire screen
    let x = f32(i32(vertex_index) * 2 - 1);
    let y = f32(i32(vertex_index / 2u) * 4 - 1);
    let uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return VertexOutput(vec4<f32>(x, y, 1.0, 1.0), uv);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let cloud = textureSample(cloud_texture, cloud_sampler, in.uv);
    return vec4<f32>(cloud.aaa, cloud.a);

    
    // Return cloud color with alpha for blending
    //return cloud;
}
