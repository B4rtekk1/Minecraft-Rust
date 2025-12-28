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

const PI: f32 = 3.14159265359;
const SHADOW_MAP_SIZE: f32 = 2048.0;
const GOLDEN_ANGLE: f32 = 2.39996322972865332;
const PCF_SAMPLES: i32 = 16;

fn vogel_disk_sample(sample_index: i32, sample_count: i32, phi: f32) -> vec2<f32> {
    let r = sqrt(f32(sample_index) + 0.5) / sqrt(f32(sample_count));
    let theta = f32(sample_index) * GOLDEN_ANGLE + phi;
    return vec2<f32>(r * cos(theta), r * sin(theta));
}

fn interleaved_gradient_noise(position: vec2<f32>) -> f32 {
    let magic = vec3<f32>(0.06711056, 0.00583715, 52.9829189);
    return fract(magic.z * fract(dot(position, magic.xy)));
}

fn calculate_shadow(world_pos: vec3<f32>, normal: vec3<f32>, sun_dir: vec3<f32>) -> f32 {
    if sun_dir.y < 0.05 {
        return 0.0;
    }
    
    let normal_offset = normal * 0.25;
    let offset_world_pos = world_pos + normal_offset;
    
    let shadow_pos = uniforms.sun_view_proj * vec4<f32>(offset_world_pos, 1.0);
    let shadow_coords = shadow_pos.xyz / shadow_pos.w;
    
    let uv = vec2<f32>(
        shadow_coords.x * 0.5 + 0.5,
        1.0 - (shadow_coords.y * 0.5 + 0.5)
    );
    
    let edge_fade = 0.02;
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return 1.0;
    }
    let edge_factor = min(
        min(uv.x, 1.0 - uv.x),
        min(uv.y, 1.0 - uv.y)
    ) / edge_fade;
    let edge_shadow_blend = clamp(edge_factor, 0.0, 1.0);
    
    let receiver_depth = shadow_coords.z;
    
    let cos_theta = max(dot(normal, sun_dir), 0.0);
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    let base_bias = 0.003;
    let slope_bias = 0.006 * sin_theta / max(cos_theta, 0.1);
    let bias = base_bias + slope_bias;
    
    let noise_input = uv * SHADOW_MAP_SIZE;
    let noise = interleaved_gradient_noise(noise_input);
    let rotation_angle = noise * 2.0 * PI;
    
    let texel_size = 1.0 / SHADOW_MAP_SIZE;
    
    let filter_radius = 2.5 * texel_size;
    
    var shadow: f32 = 0.0;
    
    for (var i: i32 = 0; i < PCF_SAMPLES; i++) {
        let offset = vogel_disk_sample(i, PCF_SAMPLES, rotation_angle) * filter_radius;
        shadow += textureSampleCompare(
            shadow_map,
            shadow_sampler,
            uv + offset,
            receiver_depth - bias
        );
    }
    
    shadow /= f32(PCF_SAMPLES);
    
    shadow = smoothstep(0.15, 0.85, shadow);
    
    return mix(1.0, shadow, edge_shadow_blend);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_sample = textureSample(texture_atlas, texture_sampler, in.uv, i32(in.tex_index + 0.5));
    
    if tex_sample.a < 0.5 {
        discard;
    }
    
    let tex_color = tex_sample.rgb;
    
    let sun_dir = normalize(uniforms.sun_position);
    
    let day_factor = clamp(sun_dir.y, 0.0, 1.0); 
    let night_factor = clamp(-sun_dir.y, 0.0, 1.0); 
    let sunset_factor = 1.0 - abs(sun_dir.y); 
    
    let day_sky = vec3<f32>(0.53, 0.81, 0.98); 
    let sunset_sky = vec3<f32>(1.0, 0.5, 0.2);
    let night_sky = vec3<f32>(0.002, 0.002, 0.01); 
    
    var sky_color = day_sky * day_factor + sunset_sky * sunset_factor * 0.5 + night_sky * night_factor;
    sky_color = clamp(sky_color, vec3<f32>(0.0), vec3<f32>(1.0));
    
    let fill_dir = normalize(vec3<f32>(-sun_dir.x, 0.5, -sun_dir.z));
    
    var shadow = 1.0;
    if sun_dir.y > 0.0 {
        shadow = calculate_shadow(in.world_pos, in.normal, sun_dir);
    }
    
    let ambient_day = 0.4;
    let ambient_night = 0.005; 
    let ambient = mix(ambient_night, ambient_day, day_factor);
    
    let sun_diffuse = max(dot(in.normal, sun_dir), 0.0) * 0.5 * shadow * day_factor;
    
    let fill_diffuse = max(dot(in.normal, fill_dir), 0.0) * 0.1 * day_factor;
    
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
    
    let effective_face_shade = mix(1.0, face_shade, day_factor + 0.3);
    
    let lighting = (ambient + sun_diffuse + fill_diffuse) * effective_face_shade;
    
    var lit_color = tex_color * lighting;
    if sunset_factor > 0.3 && sun_dir.y > -0.2 {
        let sunset_tint = vec3<f32>(1.0, 0.85, 0.7);
        lit_color = lit_color * mix(vec3<f32>(1.0), sunset_tint, sunset_factor * 0.5);
    }
    
    let dist = length(in.world_pos.xz - uniforms.camera_pos.xz);
    
    let visibility_night = 12.0;
    let visibility_day = 250.0;
    let visibility_range = mix(visibility_night, visibility_day, day_factor);
    
    let fog_start = visibility_range * 0.2;
    let fog_end = visibility_range;
    
    let visibility = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
    
    let fog_color = mix(vec3<f32>(0.0, 0.0, 0.0), sky_color, day_factor);
    let final_color = mix(fog_color, lit_color, visibility);
    
    return vec4<f32>(final_color, 1.0);
}
