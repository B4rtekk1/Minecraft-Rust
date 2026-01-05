/// Composite Shader
///
/// Combines the scene color with SSAO.
/// Applies SSAO as ambient occlusion multiplier: final = scene * ssao

struct Uniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    sun_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_position: vec3<f32>,
    is_underwater: f32,
    screen_size: vec2<f32>,
    water_level: f32,
    reflection_mode: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var scene_texture: texture_2d<f32>;

@group(0) @binding(2)
var ssao_texture: texture_2d<f32>;

@group(0) @binding(3)
var composite_sampler: sampler;

// Full-screen triangle vertices
var<private> positions: array<vec2<f32>, 3> = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>(3.0, -1.0),
    vec2<f32>(-1.0, 3.0)
);

var<private> uvs: array<vec2<f32>, 3> = array<vec2<f32>, 3>(
    vec2<f32>(0.0, 1.0),
    vec2<f32>(2.0, 1.0),
    vec2<f32>(0.0, -1.0)
);

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene_color = textureSample(scene_texture, composite_sampler, in.uv);
    let ssao = textureSample(ssao_texture, composite_sampler, in.uv).r;
    
    // Apply SSAO as subtle ambient multiplier
    // Use a LOW ssao_strength to make the effect subtle (0.5 = 50% strength max)
    let ssao_strength = 0.5;  // How much SSAO affects the final image (0.0 = off, 1.0 = full)
    
    // Reduce AO effect in dark areas to prevent them becoming pitch black
    let brightness = dot(scene_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let ao_amount = mix(0.3, ssao_strength, brightness);  // Less in dark, more in bright

    // Blend between no-AO (1.0) and full-AO (ssao value)
    let ao_factor = mix(1.0, ssao, ao_amount);
    let final_color = scene_color.rgb * ao_factor;

    return vec4<f32>(final_color, scene_color.a);
}
