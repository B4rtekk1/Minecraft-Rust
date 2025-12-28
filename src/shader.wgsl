struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var texture_atlas: texture_2d_array<f32>;
@group(0) @binding(2)
var texture_sampler: sampler;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_sample = textureSample(texture_atlas, texture_sampler, in.uv, i32(in.tex_index + 0.5));
    
    if tex_sample.a < 0.5 {
        discard;
    }
    
    let tex_color = tex_sample.rgb;
    
    let sun_dir = normalize(vec3<f32>(0.4, 0.8, 0.3));
    let fill_dir = normalize(vec3<f32>(-0.3, 0.5, -0.4));
    
    let ambient = 0.5;
    
    let sun_diffuse = max(dot(in.normal, sun_dir), 0.0) * 0.4;
    
    let fill_diffuse = max(dot(in.normal, fill_dir), 0.0) * 0.15;
    
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
    
    var base_color = tex_color;
    
    var lit_color = base_color * lighting;
    
    let dist = length(in.world_pos.xz - uniforms.camera_pos.xz);
    let fog_start = 120.0;
    let fog_end = 200.0;
    let fog_factor = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
    let sky_color = vec3<f32>(0.53, 0.81, 0.98);
    
    let final_color = mix(sky_color, lit_color, fog_factor);
    
    return vec4<f32>(final_color, 1.0);
}

@vertex
fn vs_water(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    var pos = model.position;
    
    let dist_to_camera = length(pos.xz - uniforms.camera_pos.xz);
    let simulation_radius = 80.0; 
    
    if model.normal.y > 0.5 && dist_to_camera < simulation_radius {
        let wave1 = sin(pos.x * 0.5 + uniforms.time * 2.0) * 0.05;
        let wave2 = sin(pos.z * 0.7 + uniforms.time * 1.5) * 0.04;
        let wave3 = sin((pos.x + pos.z) * 0.3 + uniforms.time * 3.0) * 0.03;
        pos.y += wave1 + wave2 + wave3;
        pos.y -= 0.15;
    } else if model.normal.y > 0.5 {
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
    
    let sky_color = vec3<f32>(0.53, 0.81, 0.98);
    
    var water_color = mix(base_water, sky_color, fresnel * 0.6);
    
    water_color += vec3<f32>(shimmer);
    
    let sun_dir = normalize(vec3<f32>(0.4, 0.8, 0.3));
    let reflect_dir = reflect(-sun_dir, in.normal);
    let spec = pow(max(dot(view_dir, reflect_dir), 0.0), 64.0);
    water_color += vec3<f32>(1.0, 0.95, 0.8) * spec * 0.8;
    
    let dist = length(in.world_pos.xz - uniforms.camera_pos.xz);
    let fog_start = 50.0;
    let fog_end = 100.0;
    let fog_factor = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
    
    let final_color = mix(sky_color, water_color, fog_factor);
    
    let alpha = 0.75 + fresnel * 0.2;
    
    return vec4<f32>(final_color, alpha);
}

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

