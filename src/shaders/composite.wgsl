/// Composite Shader
///
/// Final post-process pass:
/// 1) Apply SSAO to scene color
/// 2) Add lightweight bloom from bright pixels
/// 3) Filmic tonemapping (ACES fit) + gamma

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
    moon_position: vec3<f32>,
    _pad1_moon: f32,
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

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn aces_film(x: vec3<f32>) -> vec3<f32> {
    // Narkowicz ACES approximation
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn sample_scene(uv: vec2<f32>) -> vec3<f32> {
    return textureSample(scene_texture, composite_sampler, uv).rgb;
}

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
    let lit_color = scene_color.rgb * ao_factor;

    // --- Lightweight bloom from bright pixels ---
    let texel = 1.0 / uniforms.screen_size;
    let b0 = sample_scene(in.uv + vec2<f32>( texel.x * 2.0,  0.0));
    let b1 = sample_scene(in.uv + vec2<f32>(-texel.x * 2.0,  0.0));
    let b2 = sample_scene(in.uv + vec2<f32>(0.0,  texel.y * 2.0));
    let b3 = sample_scene(in.uv + vec2<f32>(0.0, -texel.y * 2.0));
    let b4 = sample_scene(in.uv + vec2<f32>( texel.x * 3.0,  texel.y * 3.0));
    let b5 = sample_scene(in.uv + vec2<f32>(-texel.x * 3.0,  texel.y * 3.0));
    let b6 = sample_scene(in.uv + vec2<f32>( texel.x * 3.0, -texel.y * 3.0));
    let b7 = sample_scene(in.uv + vec2<f32>(-texel.x * 3.0, -texel.y * 3.0));

    var bloom_source = (lit_color + b0 + b1 + b2 + b3 + b4 + b5 + b6 + b7) / 9.0;
    let bloom_threshold = 1.0;
    let bloom_luma = luminance(bloom_source);
    let bloom_mask = smoothstep(bloom_threshold, bloom_threshold + 0.8, bloom_luma);
    let bloom = bloom_source * bloom_mask;

    // --- Exposure adapts by time of day ---
    let sun_height = normalize(uniforms.sun_position).y;
    let day_exposure = 0.95;
    let sunset_exposure = 1.08;
    let night_exposure = 1.28;
    let day_factor = clamp(sun_height * 4.0, 0.0, 1.0);
    let night_factor = clamp(-sun_height * 4.0, 0.0, 1.0);
    let sunset_factor = smoothstep(0.30, 0.0, abs(sun_height));
    let exposure = day_exposure * day_factor + sunset_exposure * sunset_factor + night_exposure * night_factor;

    // Slightly stronger bloom near horizon for atmospheric glow.
    let bloom_strength = mix(0.10, 0.22, sunset_factor);
    let hdr = lit_color + bloom * bloom_strength;

    // --- Tonemap + display gamma ---
    let mapped = aces_film(hdr * exposure);
    let gamma_corrected = pow(mapped, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(gamma_corrected, scene_color.a);
}
