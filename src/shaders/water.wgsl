/// Water Rendering Shader
///
/// This shader handles the rendering of transparent water surfaces.
/// It includes support for:
/// - Procedural vertex wave displacement (sinusoidal)
/// - Fresnel-based sky reflections
/// - Screen Space Reflections (SSR)
/// - Specular highlights for sun and moon
/// - Procedural shimmer/glitter effects
/// - Depth-based fog and alpha blending

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
    reflection_mode: f32, // 0=off, 1=SSR
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

// SSR textures (scene rendered before water)
@group(0) @binding(5)
var ssr_color: texture_2d<f32>;
@group(0) @binding(6)
var ssr_depth: texture_depth_2d;
@group(0) @binding(7)
var ssr_sampler: sampler;

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

/// Water Vertex Shader
///
/// Displaces the y-coordinate of top-facing faces using multiple sine waves
/// to create a dynamic liquid surface.
@vertex
fn vs_water(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    var pos = model.position;
    if model.normal.y > 0.5 {
        // Multi-layered sine wave displacement
        let wave1 = sin(pos.x * 0.4 + uniforms.time * 2.1) * 0.05;
        let wave2 = sin(pos.z * 0.5 + uniforms.time * 1.8) * 0.04;
        let wave3 = sin((pos.x + pos.z) * 0.25 + uniforms.time * 2.8) * 0.035;
        let wave4 = sin((pos.x * 0.3 - pos.z * 0.4) + uniforms.time * 2.3) * 0.025;
        pos.y += wave1 + wave2 + wave3 + wave4;
        
        // Slightly lower water level to prevent z-fighting with adjacent solid blocks
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

const PI: f32 = 3.14159265359;
const SHADOW_MAP_SIZE: f32 = 2048.0;
const GOLDEN_ANGLE: f32 = 2.39996322972865332;
const PCF_SAMPLES: i32 = 24;
const SSR_MAX_STEPS: i32 = 64;
const SSR_BINARY_SEARCH_STEPS: i32 = 8;
const SSR_MAX_DISTANCE: f32 = 60.0;
const SSR_THICKNESS: f32 = 0.15; // Lower tolerance to prevent "underwater" artifacts

/// Screen Space Reflections ray marching
/// Returns vec4 where xyz = reflected color, w = confidence (0 = no hit, 1 = solid hit)
fn ssr_trace(
    world_pos: vec3<f32>,
    reflect_dir: vec3<f32>,
    screen_pos: vec4<f32>
) -> vec4<f32> {
    var ray_pos = world_pos;
    let step_size = SSR_MAX_DISTANCE / f32(SSR_MAX_STEPS);
    
    var hit_found = false;
    var uv_hit = vec2<f32>(0.0);
    
    // 1. Linear Ray Marching
    for (var i: i32 = 0; i < SSR_MAX_STEPS; i++) {
        ray_pos += reflect_dir * step_size;

        // Project ray position to screen space
        let ray_clip = uniforms.view_proj * vec4<f32>(ray_pos, 1.0);
        let ray_ndc = ray_clip.xyz / ray_clip.w;

        // Convert to UV coordinates
        let ray_uv = vec2<f32>(
            ray_ndc.x * 0.5 + 0.5,
            1.0 - (ray_ndc.y * 0.5 + 0.5)
        );

        // Check if outside screen bounds
        if ray_uv.x < 0.0 || ray_uv.x > 1.0 || ray_uv.y < 0.0 || ray_uv.y > 1.0 {
            break;
        }

        // Sample depth at ray position
        let scene_depth = textureSample(ssr_depth, ssr_sampler, ray_uv);
        let ray_depth = ray_ndc.z;

        // Check for intersection
        if ray_depth > scene_depth && ray_depth < scene_depth + SSR_THICKNESS {
            hit_found = true;
            uv_hit = ray_uv;
            
            // 2. Binary Search Refinement
            // Go back one step and refine
            var start_pos = ray_pos - reflect_dir * step_size;
            var end_pos = ray_pos;
            
            for (var j: i32 = 0; j < SSR_BINARY_SEARCH_STEPS; j++) {
                let mid_pos = (start_pos + end_pos) * 0.5;
                
                let mid_clip = uniforms.view_proj * vec4<f32>(mid_pos, 1.0);
                let mid_ndc = mid_clip.xyz / mid_clip.w;
                let mid_uv = vec2<f32>(
                    mid_ndc.x * 0.5 + 0.5,
                    1.0 - (mid_ndc.y * 0.5 + 0.5)
                );
                
                let mid_scene_depth = textureSample(ssr_depth, ssr_sampler, mid_uv);
                let mid_ray_depth = mid_ndc.z;
                
                if mid_ray_depth > mid_scene_depth {
                    end_pos = mid_pos; // Hit is closer
                    uv_hit = mid_uv;
                } else {
                    start_pos = mid_pos; // No hit, move forward
                }
            }
            break;
        }
    }

    if hit_found {
        // Sample scene color at refined UV
        let scene_color = textureSample(ssr_color, ssr_sampler, uv_hit).rgb;

        // Calculate confidence based on edge fadeout (vignette)
        let edge_x = min(uv_hit.x, 1.0 - uv_hit.x);
        let edge_y = min(uv_hit.y, 1.0 - uv_hit.y);
        let edge_fade = min(edge_x, edge_y) * 10.0;
        let confidence = clamp(edge_fade, 0.0, 1.0);

        return vec4<f32>(scene_color, confidence);
    }

    // No hit
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}

/// Calculate sky color with localized sunrise/sunset gradient
fn calculate_sky_color(view_dir: vec3<f32>, sun_dir: vec3<f32>) -> vec3<f32> {
    let sun_height = sun_dir.y;
    
    // Time-of-day factors
    let day_factor = clamp(sun_height, 0.0, 1.0);
    let night_factor = clamp(-sun_height, 0.0, 1.0);
    let sunset_factor = 1.0 - abs(sun_height);
    
    // Vertical gradient
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
    
    // 3D angle to sun
    let cos_angle_3d = dot(normalize(view_dir), normalize(sun_dir));
    
    // --- BASE SKY COLORS ---
    let zenith_day = vec3<f32>(0.25, 0.45, 0.85);
    let horizon_day = vec3<f32>(0.6, 0.75, 0.95);
    let zenith_night = vec3<f32>(0.001, 0.001, 0.008);
    let horizon_night = vec3<f32>(0.01, 0.01, 0.02);
    
    let height_factor = clamp(view_height * 0.5 + 0.5, 0.0, 1.0);
    var sky_color = mix(horizon_day, zenith_day, height_factor) * day_factor;
    sky_color += mix(horizon_night, zenith_night, height_factor) * night_factor;
    
    // --- LOCALIZED SUNSET/SUNRISE EFFECT ---
    if sunset_factor > 0.01 && sun_height > -0.3 {
        let sunset_orange = vec3<f32>(1.0, 0.4, 0.1);
        let sunset_red = vec3<f32>(0.9, 0.2, 0.05);
        let sunset_yellow = vec3<f32>(1.0, 0.7, 0.3);
        let sunset_pink = vec3<f32>(0.95, 0.5, 0.6);
        
        // Match 3D/Horizontal mix from sky.wgsl
        let sun_proximity_3d = max(0.0, cos_angle_3d);
        let sun_proximity_horiz = max(0.0, cos_angle_horizontal);
        let sun_proximity = mix(sun_proximity_horiz, sun_proximity_3d, 0.5);
        
        let glow_tight = pow(sun_proximity_3d, 32.0);
        let glow_medium = pow(sun_proximity, 4.0);
        let glow_wide = pow(sun_proximity, 1.5);
        
        let sunset_intensity = smoothstep(-0.2, 0.1, sun_height) * smoothstep(0.6, 0.0, sun_height);
        
        let horizon_band = 1.0 - abs(view_height);
        let horizon_boost = pow(horizon_band, 0.5) * smoothstep(0.0, 0.1, v_len);
        
        var sunset_color = vec3<f32>(0.0);
        sunset_color += sunset_yellow * glow_tight * 1.2;
        sunset_color += sunset_orange * glow_medium * 0.8 * horizon_boost;
        sunset_color += sunset_red * glow_wide * 0.5 * horizon_boost;
        
        let opposite_glow = max(0.0, -cos_angle_horizontal) * 0.2;
        sunset_color += sunset_pink * opposite_glow * horizon_band * smoothstep(0.0, 0.1, v_len);
        
        sky_color = mix(sky_color, sky_color + sunset_color, sunset_intensity);
    }
    
    if day_factor > 0.1 {
        let sun_glow = pow(max(0.0, cos_angle_3d), 128.0) * day_factor;
        sky_color += vec3<f32>(1.0, 0.95, 0.9) * sun_glow;
    }
    
    return clamp(sky_color, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn vogel_disk_sample(sample_index: i32, sample_count: i32, phi: f32) -> vec2<f32> {
    let r = sqrt(f32(sample_index) + 0.5) / sqrt(f32(sample_count));
    let theta = f32(sample_index) * GOLDEN_ANGLE + phi;
    return vec2<f32>(r * cos(theta), r * sin(theta));
}

fn world_space_noise(world_pos: vec3<f32>) -> f32 {
    let p = world_pos * 0.5;
    return fract(sin(dot(floor(p.xz), vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

/// Calculate perturbed water normal for realistic ripple reflections
/// Uses derivative of wave functions with noise and crossing wave patterns
fn calculate_water_normal(world_pos: vec3<f32>, time: f32) -> vec3<f32> {
    // Add noise offset to break up regularity
    let noise1 = fract(sin(dot(world_pos.xz * 0.1, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let noise2 = fract(sin(dot(world_pos.xz * 0.15, vec2<f32>(39.346, 11.135))) * 43758.5453);
    let noise_offset = (noise1 - 0.5) * 0.3;
    
    // Primary waves with noise-varied phases
    let wave1_dx = cos(world_pos.x * 0.5 + time * 2.0 + noise_offset) * 0.025;
    let wave2_dz = cos(world_pos.z * 0.7 + time * 1.5 + noise_offset) * 0.02;
    
    // Diagonal crossing waves (45 degree angles) - breaks up parallel lines
    let diag1 = cos((world_pos.x + world_pos.z) * 0.4 + time * 2.5) * 0.018;
    let diag2 = cos((world_pos.x - world_pos.z) * 0.35 + time * 2.2) * 0.015;
    
    // Different angle waves (30, 60 degrees) for more organic look
    let angle1 = cos((world_pos.x * 0.866 + world_pos.z * 0.5) * 0.5 + time * 1.8) * 0.012;
    let angle2 = cos((world_pos.x * 0.5 + world_pos.z * 0.866) * 0.45 + time * 2.1) * 0.012;
    let angle3 = cos((world_pos.x * 0.866 - world_pos.z * 0.5) * 0.55 + time * 1.9) * 0.01;
    
    // Fine detail ripples with noise variation
    let ripple_scale = 1.0 + noise2 * 0.5;
    let ripple1 = cos(world_pos.x * 3.0 * ripple_scale + time * 4.0) * 0.008;
    let ripple2 = cos(world_pos.z * 2.8 * ripple_scale + time * 3.5) * 0.007;
    let ripple3 = cos((world_pos.x + world_pos.z) * 2.0 + time * 5.0) * 0.005;
    let ripple4 = cos((world_pos.x - world_pos.z) * 2.2 + time * 4.5) * 0.005;
    
    // Sum all derivatives for X and Z
    // Diagonal waves contribute equally to both X and Z
    let dx = wave1_dx 
           + diag1 + diag2 
           + angle1 * 0.866 + angle2 * 0.5 + angle3 * 0.866
           + ripple1 + ripple3 + ripple4;
    let dz = wave2_dz 
           + diag1 - diag2 
           + angle1 * 0.5 + angle2 * 0.866 - angle3 * 0.5
           + ripple2 + ripple3 - ripple4;
    
    // Normal from tangent plane: n = normalize((-dx, 1, -dz))
    return normalize(vec3<f32>(-dx, 1.0, -dz));
}

fn calculate_shadow(world_pos: vec3<f32>, normal: vec3<f32>, sun_dir: vec3<f32>) -> f32 {
    if sun_dir.y < 0.05 {
        return 0.0;
    }
    
    // No normal-based offset - rely on pipeline depth bias to prevent shadow acne
    let offset_world_pos = world_pos;
    
    let shadow_pos = uniforms.sun_view_proj * vec4<f32>(offset_world_pos, 1.0);
    let shadow_coords = shadow_pos.xyz / shadow_pos.w;
    
    let uv = vec2<f32>(
        shadow_coords.x * 0.5 + 0.5,
        1.0 - (shadow_coords.y * 0.5 + 0.5)
    );
    
    let edge_fade = 0.03;
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return 1.0;
    }
    let edge_factor = min(
        min(uv.x, 1.0 - uv.x),
        min(uv.y, 1.0 - uv.y)
    ) / edge_fade;
    let edge_shadow_blend = clamp(edge_factor, 0.0, 1.0);
    
    let receiver_depth = shadow_coords.z;
    
    // Minimal adaptive bias to prevent shadow acne without causing visible offset
    let cos_theta = max(dot(normal, sun_dir), 0.0);
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    let base_bias = 0.0005;
    let slope_bias = 0.001 * sin_theta / max(cos_theta, 0.1);
    let bias = base_bias + slope_bias;
    
    let noise = world_space_noise(world_pos);
    let rotation_angle = noise * 2.0 * PI;
    
    let texel_size = 1.0 / SHADOW_MAP_SIZE;
    let filter_radius = 3.5 * texel_size;
    
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
    shadow = smoothstep(0.05, 0.95, shadow);
    
    return mix(1.0, shadow, edge_shadow_blend);
}

/// Water Fragment Shader
///
/// Implements a water lighting model with Fresnel reflections, specular highlights,
/// and procedural animated shimmer.
@fragment
fn fs_water(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(texture_atlas, texture_sampler, in.uv, i32(in.tex_index + 0.5)).rgb;
    let base_water = tex_color;
    
    // Procedural shimmer/sparkle effect
    let shimmer1 = sin(in.world_pos.x * 2.0 + uniforms.time * 3.0) * 0.5 + 0.5;
    let shimmer2 = sin(in.world_pos.z * 2.5 + uniforms.time * 2.5) * 0.5 + 0.5;
    let shimmer = shimmer1 * shimmer2 * 0.15;
    
    // Calculate perturbed normal for realistic ripple reflections
    let perturbed_normal = calculate_water_normal(in.world_pos, uniforms.time);
    
    // Blend between vertex normal and perturbed normal based on distance
    // (less perturbation at distance to avoid noise)
    let dist_to_camera = length(in.world_pos - uniforms.camera_pos);
    let normal_blend = clamp(1.0 - dist_to_camera / 100.0, 0.3, 1.0);
    let water_normal = normalize(mix(in.normal, perturbed_normal, normal_blend));
    
    // Fresnel effect: water is more reflective when viewed at a grazing angle
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let fresnel = pow(1.0 - max(dot(view_dir, water_normal), 0.0), 3.0);
    
    let sun_dir = normalize(uniforms.sun_position);
    
    // Time of day factors
    let day_factor = clamp(sun_dir.y, 0.0, 1.0);
    let night_factor = clamp(-sun_dir.y, 0.0, 1.0);
    let sunset_factor = 1.0 - abs(sun_dir.y);
    
    // Calculate view direction from camera to this fragment (for localized sky gradient)
    let fragment_view_dir = normalize(in.world_pos - uniforms.camera_pos);
    
    // --- SCREEN SPACE REFLECTIONS ---
    // Calculate reflection direction using perturbed normal for realistic ripples
    let reflect_dir_ssr = reflect(fragment_view_dir, water_normal);

    // Calculate sky color with localized sunset effect based on REFLECTION direction
    // This ensures that when SSR fails, we fall back to the sky color being reflected, not the sky below us
    let sky_color = calculate_sky_color(reflect_dir_ssr, sun_dir);
    
    // Reflection mode: 0=off (sky only), 1=SSR
    let reflection_mode = i32(uniforms.reflection_mode);
    var reflection_color = sky_color;
    
    let ssr_distance_fade = clamp(1.0 - dist_to_camera / 150.0, 0.0, 1.0);
    
    // Mode 0: Off - use sky only
    if reflection_mode == 0 {
        reflection_color = sky_color;
    }
    // Mode 1: SSR
    else {
        let ssr_result = ssr_trace(in.world_pos, reflect_dir_ssr, in.clip_position);
        if ssr_result.w > 0.0 {
            let ssr_blend = ssr_result.w * 0.85 * ssr_distance_fade;
            reflection_color = mix(sky_color, ssr_result.rgb, ssr_blend);
        }
    }
    
    var shadow = 1.0;
    if sun_dir.y > 0.0 {
        shadow = calculate_shadow(in.world_pos, in.normal, sun_dir);
    }
    
    let ambient_day = 0.4;
    let ambient_night = 0.008; 
    let ambient = mix(ambient_night, ambient_day, day_factor);
    
    // Mix base texture color with reflected color (SSR + sky) based on Fresnel
    var water_color = mix(base_water, reflection_color, fresnel * 0.6);
    water_color += vec3<f32>(shimmer * shadow * day_factor);
    
    // Solar specular highlights (sun glisten)
    // Use less perturbed normal for specular so it follows sun, not player
    if sun_dir.y > 0.0 {
        let spec_normal = normalize(mix(in.normal, water_normal, 0.2)); // Very subtle perturbation for solar spec
        let reflect_dir = reflect(-sun_dir, spec_normal);
        let spec = pow(max(dot(view_dir, reflect_dir), 0.0), 64.0); // Softer highlight to avoid stripe artifacts
        water_color += vec3<f32>(1.0, 0.95, 0.8) * spec * 1.0 * shadow * day_factor;
    }
    
    // Lunar specular highlights - slightly more perturbation for softer moonlight
    if night_factor > 0.2 {
        let moon_dir = normalize(vec3<f32>(0.3, 0.5, -0.8));
        let spec_normal = normalize(mix(in.normal, water_normal, 0.4));
        let moon_reflect = reflect(-moon_dir, spec_normal);
        let moon_spec = pow(max(dot(view_dir, moon_reflect), 0.0), 64.0);
        water_color += vec3<f32>(0.7, 0.8, 1.0) * moon_spec * 0.3 * night_factor;
    }
    
    water_color = water_color * (ambient + shadow * 0.6 * day_factor);
    
    // --- FOG ---
    let dist = length(in.world_pos.xz - uniforms.camera_pos.xz);
    
    // Check if underwater
    let is_underwater = uniforms.is_underwater > 0.5;
    
    var visibility_range: f32;
    var fog_color_final: vec3<f32>;
    
    if is_underwater {
        // Underwater: very short visibility
        visibility_range = 20.0;
        fog_color_final = vec3<f32>(0.05, 0.15, 0.3);
    } else {
        let visibility_night = 20.0;  // Match terrain visibility
        let visibility_day = 250.0;
        visibility_range = mix(visibility_night, visibility_day, day_factor);
        // Night fog must match night sky color to hide silhouettes
        let night_fog_color = vec3<f32>(0.001, 0.001, 0.008);  // Match zenith_night
        fog_color_final = mix(night_fog_color, sky_color, day_factor);
    }
    
    let fog_start = visibility_range * 0.2;
    let fog_end = visibility_range;
    
    let visibility = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
    
    var final_color = mix(fog_color_final, water_color, visibility);
    
    // Apply underwater color filter
    if is_underwater {
        let water_tint = vec3<f32>(0.5, 0.8, 1.0);
        final_color = final_color * water_tint;
        
        // Caustic effect
        let caustic = sin(in.world_pos.x * 0.5 + uniforms.time * 2.0) * 
                      sin(in.world_pos.z * 0.5 + uniforms.time * 1.5) * 0.15 + 0.85;
        final_color = final_color * caustic;
    }
    
    // Increase opacity at sharp angles (fresnel); more opaque underwater
    var alpha: f32;
    if is_underwater {
        alpha = 0.9;
    } else {
        alpha = 0.75 + fresnel * 0.2;
    }
    
    return vec4<f32>(final_color, alpha);
}
