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
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(model.position, 1.0);
    out.world_pos = model.position;
    out.normal = model.normal;
    out.color = model.color;
    out.uv = model.uv;
    out.tex_index = model.tex_index;
    return out;
}

@vertex
fn vs_shadow(model: VertexInput) -> @builtin(position) vec4<f32> {
    return uniforms.sun_view_proj * vec4<f32>(model.position, 1.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_sample = textureSample(texture_atlas, texture_sampler, in.uv, i32(in.tex_index + 0.5));
    
    if tex_sample.a < 0.5 {
        discard;
    }
    
    let tex_color = tex_sample.rgb;
    
    let sun_dir = normalize(uniforms.sun_position);
    let fill_dir = normalize(vec3<f32>(-sun_dir.x, 0.5, -sun_dir.z));
    
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
    let sun_diffuse = max(dot(in.normal, sun_dir), 0.0) * 0.5 * shadow;
    let fill_diffuse = max(dot(in.normal, fill_dir), 0.0) * 0.1;
    
    var face_shade = 1.0;
    if abs(in.normal.y) > 0.5 {
        if in.normal.y > 0.0 {
            face_shade = 1.0;
        } else {
            face_shade = 0.5;
        }
    } else if abs(in.normal.x) > 0.5 {
        face_shade = 0.7;
    } else {
        face_shade = 0.8;
    }
    
    let lighting = (ambient + sun_diffuse + fill_diffuse) * face_shade;
    var lit_color = tex_color * lighting;
    
    let dist = length(in.world_pos.xz - uniforms.camera_pos.xz);
    let fog_start = 150.0;
    let fog_end = 250.0;
    let fog_factor = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
    let sky_color = vec3<f32>(0.53, 0.81, 0.98);
    
    let final_color = mix(sky_color, lit_color, fog_factor);
    
    return vec4<f32>(final_color, 1.0);
}
