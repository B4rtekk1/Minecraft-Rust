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
fn vs_water(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    var pos = model.position;
    if model.normal.y > 0.5 {
        let wave1 = sin(pos.x * 0.5 + uniforms.time * 2.0) * 0.05;
        let wave2 = sin(pos.z * 0.7 + uniforms.time * 1.5) * 0.04;
        let wave3 = sin((pos.x + pos.z) * 0.3 + uniforms.time * 3.0) * 0.03;
        pos.y += wave1 + wave2 + wave3;
        pos.y -= 0.15;
    }
    
    out.clip_position = uniforms.view_proj * vec4<f32>(pos, 1.0);
    out.world_pos = pos;
    out.normal = model.normal;
    out.color = model.color;
    out.uv = model.uv;
    out.tex_index = model.tex_index;
    return out;
}

@fragment
fn fs_water(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(texture_atlas, texture_sampler, in.uv, i32(in.tex_index + 0.5)).rgb;
    let base_water = tex_color;
    
    let shimmer1 = sin(in.world_pos.x * 2.0 + uniforms.time * 3.0) * 0.5 + 0.5;
    let shimmer2 = sin(in.world_pos.z * 2.5 + uniforms.time * 2.5) * 0.5 + 0.5;
    let shimmer = shimmer1 * shimmer2 * 0.15;
    
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let fresnel = pow(1.0 - max(dot(view_dir, in.normal), 0.0), 3.0);
    
    let shadow_pos = uniforms.sun_view_proj * vec4<f32>(in.world_pos, 1.0);
    let shadow_coords = shadow_pos.xyz / shadow_pos.w;
    
    let light_local = vec2<f32>(
        shadow_coords.x * 0.5 + 0.5,
        1.0 - (shadow_coords.y * 0.5 + 0.5)
    );
    
    var shadow = 0.0;
    let texel_size = 1.0 / 2048.0;
    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow += textureSampleCompare(
                shadow_map,
                shadow_sampler,
                light_local + offset,
                shadow_coords.z - 0.0005
            );
        }
    }
    shadow /= 9.0;
    
    let ambient = 0.4;
    let sky_color = vec3<f32>(0.53, 0.81, 0.98);
    
    var water_color = mix(base_water, sky_color, fresnel * 0.6);
    water_color += vec3<f32>(shimmer * shadow);
    
    let sun_dir = normalize(uniforms.sun_position);
    let reflect_dir = reflect(-sun_dir, in.normal);
    let spec = pow(max(dot(view_dir, reflect_dir), 0.0), 64.0);
    water_color += vec3<f32>(1.0, 0.95, 0.8) * spec * 0.8 * shadow;
    
    water_color = water_color * (ambient + shadow * 0.6);
    
    let dist = length(in.world_pos.xz - uniforms.camera_pos.xz);
    let fog_start = 150.0;
    let fog_end = 250.0;
    let fog_factor = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
    
    let final_color = mix(sky_color, water_color, fog_factor);
    let alpha = 0.75 + fresnel * 0.2;
    
    return vec4<f32>(final_color, alpha);
}
