/// SSAO Blur Shader
///
/// Bilateral blur to smooth SSAO while preserving edges.
/// Uses depth-aware weighting to avoid blurring across geometric edges.

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
var ssao_texture: texture_2d<f32>;

@group(0) @binding(2)
var depth_texture: texture_depth_2d;

@group(0) @binding(3)
var blur_sampler: sampler;

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
    let texel_size = 1.0 / uniforms.screen_size;

    let center_depth = textureSample(depth_texture, blur_sampler, in.uv);
    let center_ao = textureSample(ssao_texture, blur_sampler, in.uv).r;
    
    // Skip sky pixels
    if center_depth >= 0.9999 {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }

    var result: f32 = 0.0;
    var total_weight: f32 = 0.0;
    
    // 4x4 bilateral blur
    let blur_radius: i32 = 2;
    let sigma_spatial: f32 = 2.0;
    let sigma_range: f32 = 0.02;  // Depth difference threshold

    for (var x: i32 = -blur_radius; x <= blur_radius; x++) {
        for (var y: i32 = -blur_radius; y <= blur_radius; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            let sample_uv = in.uv + offset;
            
            // Skip out-of-bounds samples
            if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
                continue;
            }

            let sample_depth = textureSample(depth_texture, blur_sampler, sample_uv);
            let sample_ao = textureSample(ssao_texture, blur_sampler, sample_uv).r;
            
            // Spatial weight (Gaussian based on distance)
            let dist_sq = f32(x * x + y * y);
            let spatial_weight = exp(-dist_sq / (2.0 * sigma_spatial * sigma_spatial));
            
            // Range weight (Gaussian based on depth difference)
            let depth_diff = abs(center_depth - sample_depth);
            let range_weight = exp(-depth_diff * depth_diff / (2.0 * sigma_range * sigma_range));

            let weight = spatial_weight * range_weight;
            result += sample_ao * weight;
            total_weight += weight;
        }
    }

    let final_ao = result / max(total_weight, 0.001);

    return vec4<f32>(final_ao, final_ao, final_ao, 1.0);
}
