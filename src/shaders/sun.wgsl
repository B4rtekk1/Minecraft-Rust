/// Sun/Moon Billboard Shader
///
/// This shader renders the sun (or moon) as a billboard quad that always 
/// faces the camera and stays at a fixed "infinite" distance.

struct Uniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    sun_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_position: vec3<f32>,
    is_underwater: f32,
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
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
};

/// Sun Vertex Shader
///
/// Calculates a billboard orientation so the sun always faces the camera.
/// The sun is positioned at uniforms.camera_pos + sun_dir * 400.0 to simulate 
/// being in the skybox.
@vertex
fn vs_sun(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    let sun_dir = normalize(uniforms.sun_position);
    let sun_world_pos = uniforms.camera_pos + sun_dir * 400.0;
    
    // Construct orthonormal basis for billboarding
    let forward = normalize(uniforms.camera_pos - sun_world_pos);
    let world_up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(world_up, forward));
    let up = cross(forward, right);
    
    let size = 30.0;
    
    // Offset quad corners based on basis vectors
    let offset = right * model.position.x * size + up * model.position.y * size;
    let world_pos = sun_world_pos + offset;
    
    out.clip_position = uniforms.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = model.uv;
    out.color = model.color;
    
    return out;
}

/// Sun Fragment Shader
///
/// Renders a procedural sun disk with a bright core and a soft outer glow.
@fragment
fn fs_sun(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dist = length(in.uv - center);
    
    let core_radius = 0.2;
    let glow_radius = 0.5;
    
    if dist < core_radius {
        // Bright solar core
        let intensity = 1.0 - (dist / core_radius) * 0.2;
        return vec4<f32>(1.0, 0.98, 0.9, intensity);
    } else if dist < glow_radius {
        // Exponentially fading outer glow
        let glow_factor = 1.0 - (dist - core_radius) / (glow_radius - core_radius);
        let glow_intensity = glow_factor * glow_factor * 0.8;
        let glow_color = vec3<f32>(1.0, 0.9, 0.6);
        return vec4<f32>(glow_color, glow_intensity);
    }
    
    discard;
}
