/// Procedural Sky Shader
///
/// Renders a realistic sky with localized sunrise/sunset colors.
/// The sunset gradient is centered around the sun's position.

struct Uniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    sun_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_position: vec3<f32>,
    _padding: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Dummy bindings to match uniform bind group layout
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
    @location(0) ndc_pos: vec2<f32>,
};

const PI: f32 = 3.14159265359;

/// Vertex shader for fullscreen sky quad
@vertex
fn vs_sky(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Position the quad at far plane (z = 0.9999 so it's behind everything)
    out.clip_position = vec4<f32>(model.position.xy, 0.9999, 1.0);
    out.ndc_pos = model.position.xy;
    return out;
}

/// Calculate sky color with localized sunrise/sunset gradient
fn calculate_sky_color(view_dir: vec3<f32>, sun_dir: vec3<f32>) -> vec3<f32> {
    let sun_height = sun_dir.y;
    
    // Time-of-day factors
    let day_factor = clamp(sun_height, 0.0, 1.0);
    let night_factor = clamp(-sun_height, 0.0, 1.0);
    let sunset_factor = 1.0 - abs(sun_height);
    
    // Vertical gradient: darker at zenith, lighter at horizon
    let view_height = view_dir.y;
    
    // Angle between view direction and sun direction
    let view_horizontal_vec = vec3<f32>(view_dir.x, 0.0, view_dir.z);
    let sun_horizontal_vec = vec3<f32>(sun_dir.x, 0.0, sun_dir.z);
    
    let v_len = length(view_horizontal_vec);
    let s_len = length(sun_horizontal_vec);
    
    var cos_angle_horizontal = 0.0;
    if (v_len > 0.0001 && s_len > 0.0001) {
        cos_angle_horizontal = dot(view_horizontal_vec / v_len, sun_horizontal_vec / s_len);
    }
    
    // 3D angle to sun for zenith/nadir gradient and sun glow
    let cos_angle_3d = dot(normalize(view_dir), normalize(sun_dir));
    
    // --- BASE SKY COLORS ---
    let zenith_day = vec3<f32>(0.25, 0.45, 0.85);
    let horizon_day = vec3<f32>(0.6, 0.75, 0.95);
    let zenith_night = vec3<f32>(0.001, 0.001, 0.008);
    let horizon_night = vec3<f32>(0.01, 0.01, 0.02);
    
    // Interpolate based on how high we're looking
    let height_factor = clamp(view_height * 0.5 + 0.5, 0.0, 1.0);
    var sky_color = mix(horizon_day, zenith_day, height_factor) * day_factor;
    sky_color += mix(horizon_night, zenith_night, height_factor) * night_factor;
    
    // --- LOCALIZED SUNSET/SUNRISE EFFECT ---
    if sunset_factor > 0.01 && sun_height > -0.3 {
        // Sunset/sunrise colors
        let sunset_orange = vec3<f32>(1.0, 0.4, 0.1);
        let sunset_red = vec3<f32>(0.9, 0.2, 0.05);
        let sunset_yellow = vec3<f32>(1.0, 0.7, 0.3);
        let sunset_pink = vec3<f32>(0.95, 0.5, 0.6);
        
        // Use 3D angle for more accurate sunset positioning relative to the actual sun
        // but still use some horizontal bias for the "band" effect
        let sun_proximity_3d = max(0.0, cos_angle_3d);
        let sun_proximity_horiz = max(0.0, cos_angle_horizontal);
        
        // Mix 3D and horizontal proximity for a natural look
        let sun_proximity = mix(sun_proximity_horiz, sun_proximity_3d, 0.5);
        
        // Different falloff rates for varied color bands
        let glow_tight = pow(sun_proximity_3d, 32.0); // Very tight core follow 3D exactly
        let glow_medium = pow(sun_proximity, 4.0);
        let glow_wide = pow(sun_proximity, 1.5);
        
        // Intensity based on how close sun is to horizon
        let sunset_intensity = smoothstep(-0.2, 0.1, sun_height) * smoothstep(0.6, 0.0, sun_height);
        
        // Horizon band effect - sunset colors are stronger near horizon
        let horizon_band = 1.0 - abs(view_height);
        let horizon_boost = pow(horizon_band, 0.5) * smoothstep(0.0, 0.1, v_len);
        
        // Build up the sunset color
        var sunset_color = vec3<f32>(0.0);
        
        // Yellow core follow 3D position exactly
        sunset_color += sunset_yellow * glow_tight * 1.2;
        
        // Orange/Red bands spread along horizon but centered on sun
        sunset_color += sunset_orange * glow_medium * 0.8 * horizon_boost;
        sunset_color += sunset_red * glow_wide * 0.5 * horizon_boost;
        
        // Pink tones on opposite side
        let opposite_glow = max(0.0, -cos_angle_horizontal) * 0.2;
        sunset_color += sunset_pink * opposite_glow * horizon_band * smoothstep(0.0, 0.1, v_len);
        
        sky_color = mix(sky_color, sky_color + sunset_color, sunset_intensity);
    }
    
    // Add slight sun glow halo (Daytime halo)
    if day_factor > 0.1 {
        let sun_glow = pow(max(0.0, cos_angle_3d), 128.0) * day_factor;
        sky_color += vec3<f32>(1.0, 0.95, 0.9) * sun_glow;
    }
    
    return clamp(sky_color, vec3<f32>(0.0), vec3<f32>(1.0));
}

/// Fragment shader - compute sky color for each pixel
@fragment
fn fs_sky(in: VertexOutput) -> @location(0) vec4<f32> {
    let sun_dir = normalize(uniforms.sun_position);
    
    // Use inv_view_proj to get the world-space view direction
    // NDC position is in.ndc_pos (x, y), we assume z=1.0 for the far plane
    let ndc = vec4<f32>(in.ndc_pos, 1.0, 1.0);
    let world_pos_4 = uniforms.inv_view_proj * ndc;
    let world_pos = world_pos_4.xyz / world_pos_4.w;
    let view_dir = normalize(world_pos - uniforms.camera_pos);
    
    let sky_color = calculate_sky_color(view_dir, sun_dir);
    
    return vec4<f32>(sky_color, 1.0);
}
