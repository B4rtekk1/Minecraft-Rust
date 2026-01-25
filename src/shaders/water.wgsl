/// Water Rendering Shader - Heavily Optimized
/// 
/// Optimizations:
/// - Pre-computed wave parameters (no sqrt in loops)
/// - Single Gerstner calculation with dual normals
/// - Removed unused vertex attributes
/// - Reduced register pressure
/// - Better cache utilization

// ============================================================================
// CONSTANTS
// ============================================================================
const PI: f32 = 3.14159265359;
const TWO_PI: f32 = 6.28318530718;
const SHADOW_MAP_SIZE: f32 = 2048.0;
const PCF_SAMPLES: i32 = 8;
const SSR_MAX_STEPS: i32 = 32;
const SSR_MAX_DISTANCE: f32 = 60.0;
const SSR_THICKNESS_BASE: f32 = 0.05;
const SSR_THICKNESS_SCALE: f32 = 0.001;
const SSR_EARLY_EXIT_CONFIDENCE: f32 = 0.95;

// Distances and fading
const LOD_NEAR: f32 = 0.0;
const LOD_FAR: f32 = 100.0;
const NORMAL_BLEND_DISTANCE: f32 = 100.0;
const NORMAL_BLEND_MIN: f32 = 0.3;
const SSR_FADE_DISTANCE: f32 = 150.0;

// Water appearance
const WATER_LEVEL_OFFSET: f32 = 0.15;
const FRESNEL_R0: f32 = 0.02;
const REFRACTION_STRENGTH: f32 = 0.02;
const REFRACTION_MIX: f32 = 0.4;

// Lighting
const AMBIENT_DAY: f32 = 0.4;
const AMBIENT_NIGHT: f32 = 0.008;
const SHADOW_CONTRIBUTION: f32 = 0.6;

// Fog
const UNDERWATER_VISIBILITY: f32 = 20.0;
const FOG_VISIBILITY_NIGHT: f32 = 20.0;
const FOG_VISIBILITY_DAY: f32 = 250.0;
const FOG_START_RATIO: f32 = 0.2;

// Shadow bias
const SHADOW_BASE_BIAS: f32 = 0.0005;
const SHADOW_SLOPE_BIAS: f32 = 0.001;
const SHADOW_EDGE_FADE: f32 = 0.03;

// ============================================================================
// UNIFORMS
// ============================================================================
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
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var texture_atlas: texture_2d_array<f32>;
@group(0) @binding(2) var texture_sampler: sampler;
@group(0) @binding(3) var shadow_map: texture_depth_2d_array;
@group(0) @binding(4) var shadow_sampler: sampler_comparison;
@group(0) @binding(5) var ssr_color: texture_2d<f32>;
@group(0) @binding(6) var ssr_depth: texture_depth_2d;
@group(0) @binding(7) var ssr_sampler: sampler;

// ============================================================================
// VERTEX/FRAGMENT STRUCTS (OPTIMIZED)
// ============================================================================
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
    @location(5) view_depth: f32,
};

// ============================================================================
// GERSTNER WAVES - OPTIMIZED
// ============================================================================

// Pre-computed wave parameters (avoid sqrt and divisions in loop)
struct WaveData {
    k: f32,              // 2π / wavelength
    c: f32,              // sqrt(g/k) * speed (wave phase velocity)
    amplitude: f32,
    dir: vec2<f32>,
    phase_offset: f32,   // Random phase offset to break up banding
}

// Cache wave data at compile time
fn get_wave_data(idx: i32) -> WaveData {
    var w: WaveData;
    switch (idx) {
        case 0: {
            let wavelength = 12.0;
            let speed = 0.08;
            let k = TWO_PI / wavelength;
            w.k = k;
            w.c = sqrt(9.8 / k) * speed;
            w.amplitude = 0.06;  // Reduced from 0.08
            w.dir = normalize(vec2(1.0, 0.23));
            w.phase_offset = 0.0;
        }
        case 1: {
            let wavelength = 9.2;
            let speed = 0.06;
            let k = TWO_PI / wavelength;
            w.k = k;
            w.c = sqrt(9.8 / k) * speed;
            w.amplitude = 0.032;  // Reduced from 0.04
            w.dir = normalize(vec2(-0.73, 0.68));
            w.phase_offset = 1.57;
        }
        case 2: {
            let wavelength = 6.3;
            let speed = 0.1;
            let k = TWO_PI / wavelength;
            w.k = k;
            w.c = sqrt(9.8 / k) * speed;
            w.amplitude = 0.027;  // Reduced from 0.03
            w.dir = normalize(vec2(0.41, -0.91));
            w.phase_offset = 3.14;
        }
        case 3: {
            let wavelength = 3.7;
            let speed = 0.18;
            let k = TWO_PI / wavelength;
            w.k = k;
            w.c = sqrt(9.8 / k) * speed;
            w.amplitude = 0.012;  // Reduced from 0.015
            w.dir = normalize(vec2(-0.89, 0.46));
            w.phase_offset = 4.71;
        }
        case 4: {
            let wavelength = 2.1;  // New mid-frequency wave
            let speed = 0.25;
            let k = TWO_PI / wavelength;
            w.k = k;
            w.c = sqrt(9.8 / k) * speed;
            w.amplitude = 0.011;
            w.dir = normalize(vec2(0.15, 0.99));
            w.phase_offset = 2.09;
        }
        case 5: {
            let wavelength = 1.4;
            let speed = 0.3;
            let k = TWO_PI / wavelength;
            w.k = k;
            w.c = sqrt(9.8 / k) * speed;
            w.amplitude = 0.009;
            w.dir = normalize(vec2(0.62, 0.79));
            w.phase_offset = 5.23;
        }
        default: {
            w.k = 1.0;
            w.c = 0.0;
            w.amplitude = 0.0;
            w.dir = vec2(1.0, 0.0);
            w.phase_offset = 0.0;
        }
    }
    return w;
}

struct GerstnerDualResult {
    displacement: vec3<f32>,
    normal: vec3<f32>,
    spark_normal: vec3<f32>,  // High-frequency normal for sparkles
}

/// Unified Gerstner calculation - computes both normals in single pass
fn calculate_gerstner_dual(pos: vec3<f32>, time: f32, camera_pos: vec3<f32>) -> GerstnerDualResult {
    let dist = length(pos.xz - camera_pos.xz);
    let lod_factor = 1.0 - clamp((dist - LOD_NEAR) / (LOD_FAR - LOD_NEAR), 0.0, 1.0);
    let smooth_lod = lod_factor * lod_factor;

    var result: GerstnerDualResult;
    result.displacement = vec3(0.0);
    result.normal = vec3(0.0, 1.0, 0.0);
    result.spark_normal = vec3(0.0, 1.0, 0.0);

    if smooth_lod < 0.005 {
        return result;
    }

    var y_offset: f32 = 0.0;
    var dx: f32 = 0.0;
    var dz: f32 = 0.0;
    var spark_dx: f32 = 0.0;
    var spark_dz: f32 = 0.0;

    let p = pos.xz;
    
    // Add subtle spatial noise to break up repetitive patterns
    let noise = fract(sin(dot(p * 0.01, vec2(12.9898, 78.233))) * 43758.5453);
    let noise_offset = (noise - 0.5) * 0.15;  // Reduced from 0.3
    
    // Single loop computes both normal and spark normal
    for (var i: i32 = 0; i < 6; i++) {
        let w = get_wave_data(i);
        let f = w.k * (dot(w.dir, p) - w.c * time) + w.phase_offset + noise_offset;
        let intensity = w.amplitude * smooth_lod;
        
        // Displacement (only first 4 waves)
        if i < 4 {
            y_offset += intensity * sin(f);
        }
        
        // Normal derivative (all 6 waves)
        let df = intensity * w.k * cos(f);
        dx += w.dir.x * df;
        dz += w.dir.y * df;
        
        // Spark normal - higher frequency (2x scale, 1.5x speed)
        let spark_f = w.k * (dot(w.dir, p * 2.0) - w.c * time * 1.5) + w.phase_offset + noise_offset;
        let spark_df = intensity * w.k * cos(spark_f);
        spark_dx += w.dir.x * spark_df;
        spark_dz += w.dir.y * spark_df;
    }

    result.displacement = vec3(0.0, y_offset, 0.0);
    result.normal = normalize(vec3(-dx, 1.0, -dz));
    result.spark_normal = normalize(vec3(-spark_dx, 1.0, -spark_dz));

    return result;
}

// ============================================================================
// VERTEX SHADER
// ============================================================================
@vertex
fn vs_water(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    var pos = model.position;
    let original_pos = model.position; // Zachowaj oryginalną pozycję przed falą

    if model.normal.y > 0.5 {
        let waves = calculate_gerstner_dual(pos, uniforms.time, uniforms.camera_pos);
        pos.y += waves.displacement.y - WATER_LEVEL_OFFSET;
    }

    out.clip_position = uniforms.view_proj * vec4(pos, 1.0);
    out.world_pos = pos;
    out.normal = model.normal;
    out.color = model.color;
    out.uv = model.uv;
    out.tex_index = model.tex_index;
    out.view_depth = out.clip_position.w;

    return out;
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================
fn schlick_fresnel(cos_theta: f32, r0: f32) -> f32 {
    let x = 1.0 - cos_theta;
    let x2 = x * x;
    return r0 + (1.0 - r0) * x2 * x2 * x;
}

fn interleaved_gradient_noise(frag_coord: vec2<f32>) -> f32 {
    let magic = vec3(0.06711056, 0.00583715, 52.9829189);
    return fract(magic.z * fract(dot(frag_coord, magic.xy)));
}

fn get_poisson_sample(idx: i32, rotation: f32) -> vec2<f32> {
    var p: vec2<f32>;
    switch (idx) {
        case 0: { p = vec2(-0.94201624, -0.39906216); }
        case 1: { p = vec2(0.94558609, -0.76890725); }
        case 2: { p = vec2(-0.094184101, -0.92938870); }
        case 3: { p = vec2(0.34495938, 0.29387760); }
        case 4: { p = vec2(-0.91588581, 0.45771432); }
        case 5: { p = vec2(-0.81544232, -0.87912464); }
        case 6: { p = vec2(-0.38277543, 0.27676845); }
        case 7: { p = vec2(0.97484398, 0.75648379); }
        default: { p = vec2(0.0, 0.0); }
    }
    let s = sin(rotation);
    let c = cos(rotation);
    return vec2(p.x * c - p.y * s, p.x * s + p.y * c);
}

fn select_cascade(view_depth: f32) -> i32 {
    if view_depth < uniforms.csm_split_distances.x { return 0; } else if view_depth < uniforms.csm_split_distances.y { return 1; } else if view_depth < uniforms.csm_split_distances.z { return 2; }
    return 3;
}

// ============================================================================
// SKY COLOR
// ============================================================================
fn calculate_sky_color(view_dir: vec3<f32>, sun_dir: vec3<f32>) -> vec3<f32> {
    let sun_height = sun_dir.y;
    let day_factor = clamp(sun_height, 0.0, 1.0);
    let night_factor = clamp(-sun_height, 0.0, 1.0);
    let sunset_factor = 1.0 - abs(sun_height);

    let view_height = view_dir.y;

    let view_horizontal_vec = vec3(view_dir.x, 0.0, view_dir.z);
    let sun_horizontal_vec = vec3(sun_dir.x, 0.0, sun_dir.z);
    let v_len = length(view_horizontal_vec);
    let s_len = length(sun_horizontal_vec);

    var cos_angle_horizontal = 0.0;
    if v_len > 0.0001 && s_len > 0.0001 {
        cos_angle_horizontal = dot(view_horizontal_vec / v_len, sun_horizontal_vec / s_len);
    }

    let cos_angle_3d = dot(normalize(view_dir), normalize(sun_dir));
    
    // Base sky colors
    let zenith_day = vec3(0.25, 0.45, 0.85);
    let horizon_day = vec3(0.65, 0.82, 0.98);
    let zenith_night = vec3(0.001, 0.001, 0.008);
    let horizon_night = vec3(0.015, 0.015, 0.03);

    let height_factor = clamp(view_height * 0.5 + 0.5, 0.0, 1.0);
    let curved_height = pow(height_factor, 0.8);

    var sky_color = mix(horizon_day, zenith_day, curved_height) * day_factor;
    sky_color += mix(horizon_night, zenith_night, height_factor) * night_factor;
    
    // Sunset/sunrise
    if sunset_factor > 0.01 && sun_height > -0.3 {
        let sunset_orange = vec3(1.0, 0.4, 0.1);
        let sunset_red = vec3(0.9, 0.2, 0.05);
        let sunset_yellow = vec3(1.0, 0.7, 0.3);
        let sunset_pink = vec3(0.95, 0.5, 0.6);

        let sun_proximity_3d = max(0.0, cos_angle_3d);
        let sun_proximity_horiz = max(0.0, cos_angle_horizontal);
        let sun_proximity = mix(sun_proximity_horiz, sun_proximity_3d, 0.5);

        let glow_tight = pow(sun_proximity_3d, 32.0);
        let glow_medium = pow(sun_proximity, 4.0);
        let glow_wide = pow(sun_proximity, 1.5);

        let sunset_intensity = smoothstep(-0.2, 0.1, sun_height) * smoothstep(0.6, 0.0, sun_height);
        let horizon_band = 1.0 - abs(view_height);
        let horizon_boost = pow(horizon_band, 0.5) * smoothstep(0.0, 0.1, v_len);

        var sunset_color = vec3(0.0);
        sunset_color += sunset_yellow * glow_tight * 1.2;
        sunset_color += sunset_orange * glow_medium * 0.8 * horizon_boost;
        sunset_color += sunset_red * glow_wide * 0.5 * horizon_boost;

        let opposite_glow = max(0.0, -cos_angle_horizontal) * 0.2;
        sunset_color += sunset_pink * opposite_glow * horizon_band * smoothstep(0.0, 0.1, v_len);

        sky_color = mix(sky_color, sky_color + sunset_color, sunset_intensity);
    }
    
    // Sun glow
    if day_factor > 0.1 {
        let sun_glow = pow(max(0.0, cos_angle_3d), 128.0) * day_factor;
        sky_color += vec3(1.0, 0.95, 0.9) * sun_glow;
    }

    return clamp(sky_color, vec3(0.0), vec3(1.5));
}

// ============================================================================
// SCREEN SPACE REFLECTIONS
// ============================================================================
fn ssr_trace(
    world_pos: vec3<f32>,
    reflect_dir: vec3<f32>,
    clip_pos: vec4<f32>
) -> vec4<f32> {
    var ray_pos = world_pos + reflect_dir * 0.1;
    let dir = normalize(reflect_dir);

    var best_uv = vec2(0.0);
    var best_depth_diff = 1e10;
    var best_confidence = 0.0;

    let step_count = f32(SSR_MAX_STEPS);

    for (var i: i32 = 0; i < SSR_MAX_STEPS; i++) {
        let step_progress = f32(i) / step_count;
        let step_dist = mix(0.3, 2.0 / step_count * 4.0, step_progress);
        ray_pos += dir * step_dist;

        let ray_clip = uniforms.view_proj * vec4(ray_pos, 1.0);
        if ray_clip.w <= 0.0 { break; }

        let ray_ndc = ray_clip.xyz / ray_clip.w;
        let ray_uv = vec2(ray_ndc.x * 0.5 + 0.5, 1.0 - (ray_ndc.y * 0.5 + 0.5));

        if ray_uv.x < 0.0 || ray_uv.x > 1.0 || ray_uv.y < 0.0 || ray_uv.y > 1.0 {
            break;
        }

        let scene_depth = textureSample(ssr_depth, ssr_sampler, ray_uv);
        let ray_depth = ray_ndc.z;
        let thickness = SSR_THICKNESS_BASE + abs(ray_depth) * SSR_THICKNESS_SCALE;
        let depth_diff = ray_depth - scene_depth;

        if depth_diff > 0.0 && depth_diff < thickness {
            if depth_diff < best_depth_diff {
                best_depth_diff = depth_diff;
                best_uv = ray_uv;
                best_confidence = 1.0 - (depth_diff / thickness);
            }
        }

        if best_confidence > SSR_EARLY_EXIT_CONFIDENCE {
            break;
        }
    }

    if best_confidence > 0.01 {
        let scene_color = textureSample(ssr_color, ssr_sampler, best_uv).rgb;
        
        // USUNIĘTE: edge_fade - teraz odbicia będą pełnej mocy aż do krawędzi
        // Opcjonalnie możesz dodać delikatne zanikanie tylko bardzo blisko krawędzi:
        // let edge_x = min(best_uv.x, 1.0 - best_uv.x);
        // let edge_y = min(best_uv.y, 1.0 - best_uv.y);
        // let edge_fade = min(edge_x, edge_y) * 1000.0; // Bardzo stroma krzywa
        // let confidence = best_confidence * clamp(edge_fade, 0.0, 1.0);
        
        // Bez edge_fade:
        let confidence = best_confidence;

        return vec4(scene_color, confidence);
    }

    return vec4(0.0, 0.0, 0.0, 0.0);
}

// ============================================================================
// SHADOWS
// ============================================================================
fn calculate_shadow(
    world_pos: vec3<f32>,
    normal: vec3<f32>,
    sun_dir: vec3<f32>,
    view_depth: f32,
    frag_coord: vec2<f32>
) -> f32 {
    if sun_dir.y < 0.05 { return 0.0; }

    let cascade_idx = select_cascade(view_depth);
    let shadow_pos = uniforms.csm_view_proj[cascade_idx] * vec4(world_pos, 1.0);
    let shadow_coords = shadow_pos.xyz / shadow_pos.w;
    let uv = vec2(shadow_coords.x * 0.5 + 0.5, 1.0 - (shadow_coords.y * 0.5 + 0.5));

    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return 1.0;
    }

    let edge_factor = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)) / SHADOW_EDGE_FADE;
    let edge_shadow_blend = clamp(edge_factor, 0.0, 1.0);

    let receiver_depth = shadow_coords.z;
    let cos_theta = max(dot(normal, sun_dir), 0.0);
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    let bias = SHADOW_BASE_BIAS + SHADOW_SLOPE_BIAS * sin_theta / max(cos_theta, 0.1);

    let noise = interleaved_gradient_noise(frag_coord);
    let rotation_angle = noise * TWO_PI;
    let texel_size = 1.0 / SHADOW_MAP_SIZE;
    let filter_radius = 4.0 * texel_size;

    var shadow: f32 = 0.0;
    for (var i: i32 = 0; i < PCF_SAMPLES; i++) {
        let offset = get_poisson_sample(i, rotation_angle) * filter_radius;
        shadow += textureSampleCompare(
            shadow_map,
            shadow_sampler,
            uv + offset,
            cascade_idx,
            receiver_depth - bias
        );
    }

    shadow /= f32(PCF_SAMPLES);
    shadow = smoothstep(0.05, 0.95, shadow);

    return mix(1.0, shadow, edge_shadow_blend);
}

// ============================================================================
// FRAGMENT SHADER - OPTIMIZED
// ============================================================================
@fragment
fn fs_water(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(texture_atlas, texture_sampler, in.uv, i32(in.tex_index + 0.5)).rgb;
    var base_water = tex_color;
    
    // Single unified Gerstner calculation for both normals
    let waves = calculate_gerstner_dual(in.world_pos, uniforms.time, uniforms.camera_pos);
    
    // Blend with vertex normal based on distance
    let dist_to_camera = length(in.world_pos - uniforms.camera_pos);
    let normal_blend = clamp(1.0 - dist_to_camera / NORMAL_BLEND_DISTANCE, NORMAL_BLEND_MIN, 1.0);
    let water_normal = normalize(mix(in.normal, waves.normal, normal_blend));
    
    // View direction and Fresnel
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let cos_theta = max(dot(view_dir, water_normal), 0.0);
    let fresnel = schlick_fresnel(cos_theta, FRESNEL_R0);
    
    // Time of day
    let sun_dir = normalize(uniforms.sun_position);
    let day_factor = clamp(sun_dir.y, 0.0, 1.0);
    let night_factor = clamp(-sun_dir.y, 0.0, 1.0);
    
    // Reflection direction
    let fragment_view_dir = normalize(in.world_pos - uniforms.camera_pos);
    var reflect_dir_ssr = reflect(fragment_view_dir, water_normal);
    reflect_dir_ssr.y = max(reflect_dir_ssr.y, 0.001);
    reflect_dir_ssr = normalize(reflect_dir_ssr);

    let sky_color = calculate_sky_color(reflect_dir_ssr, sun_dir);
    
    // Screen UV for refraction
    let clip_ndc = in.clip_position.xyz / in.clip_position.w;
    let screen_uv = vec2(clip_ndc.x * 0.5 + 0.5, 1.0 - (clip_ndc.y * 0.5 + 0.5));
    
    // Refraction
    let refract_offset = water_normal.xz * REFRACTION_STRENGTH + waves.normal.xz * 0.01;
    let refract_uv = clamp(screen_uv + refract_offset * (1.0 - fresnel), vec2(0.0), vec2(1.0));
    let refract_color = textureSample(ssr_color, ssr_sampler, refract_uv).rgb;
    base_water = mix(base_water, refract_color * 0.6, REFRACTION_MIX * (1.0 - fresnel));
    
    // Reflection (SSR or sky)
    var reflection_color = sky_color;
    let reflection_mode = i32(uniforms.reflection_mode);
    let ssr_distance_fade = clamp(1.0 - dist_to_camera / SSR_FADE_DISTANCE, 0.0, 1.0);

    if reflection_mode != 0 {
        let ssr_result = ssr_trace(in.world_pos, reflect_dir_ssr, in.clip_position);
        let smoothed_confidence = smoothstep(0.1, 0.95, ssr_result.w);

        if smoothed_confidence > 0.05 {
            let ssr_blend = smoothed_confidence * 0.8 * ssr_distance_fade;
            reflection_color = mix(sky_color, ssr_result.rgb, ssr_blend);
        }
    }
    
    // Shadow
    var shadow = 1.0;
    if sun_dir.y > 0.0 {
        shadow = calculate_shadow(in.world_pos, in.normal, sun_dir, in.view_depth, in.clip_position.xy);
    }

    let ambient = mix(AMBIENT_NIGHT, AMBIENT_DAY, day_factor);
    
    // Mix base with reflection
    var water_color = mix(base_water, reflection_color, fresnel);
    
    // Glitter (using pre-computed spark normal)
    let glitter = pow(max(dot(water_normal, waves.spark_normal), 0.0), 32.0) * 0.2;
    water_color += vec3(glitter * shadow * day_factor);
    
    // Solar specular
    if sun_dir.y > 0.0 {
        let spec_normal = normalize(mix(in.normal, water_normal, 0.5));
        let sun_reflect = reflect(-sun_dir, spec_normal);
        let spec_main = pow(max(dot(view_dir, sun_reflect), 0.0), 128.0);

        let spark_reflect = reflect(-sun_dir, waves.spark_normal);
        let spec_spark = pow(max(dot(view_dir, spark_reflect), 0.0), 256.0);

        water_color += vec3(1.0, 0.98, 0.9) * (spec_main + spec_spark * 0.8) * shadow * day_factor;
    }
    
    // Lunar specular
    if night_factor > 0.2 {
        let moon_dir = normalize(vec3(0.3, 0.5, -0.8));
        let spec_normal = normalize(mix(in.normal, water_normal, 0.4));
        let moon_reflect = reflect(-moon_dir, spec_normal);
        let moon_spec = pow(max(dot(view_dir, moon_reflect), 0.0), 64.0);
        water_color += vec3(0.7, 0.8, 1.0) * moon_spec * 0.3 * night_factor;
    }

    water_color *= (ambient + shadow * SHADOW_CONTRIBUTION * day_factor);
    
    // Fog
    let dist = length(in.world_pos.xz - uniforms.camera_pos.xz);
    let is_underwater = uniforms.is_underwater > 0.5;

    var visibility_range: f32;
    var fog_color_final: vec3<f32>;

    if is_underwater {
        visibility_range = UNDERWATER_VISIBILITY;
        fog_color_final = vec3(0.05, 0.15, 0.3);
    } else {
        visibility_range = mix(FOG_VISIBILITY_NIGHT, FOG_VISIBILITY_DAY, day_factor);
        let night_fog_color = vec3(0.001, 0.001, 0.008);
        fog_color_final = mix(night_fog_color, sky_color, day_factor);
    }

    let fog_start = visibility_range * FOG_START_RATIO;
    let fog_end = visibility_range;
    let visibility = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
    var final_color = mix(fog_color_final, water_color, visibility);
    
    // Underwater effects
    if is_underwater {
        let water_tint = vec3(0.5, 0.8, 1.0);
        final_color *= water_tint;

        let caustic = sin(in.world_pos.x * 0.5 + uniforms.time * 2.0) * sin(in.world_pos.z * 0.5 + uniforms.time * 1.5) * 0.15 + 0.85;
        final_color *= caustic;
    }
    
    // Alpha
    let alpha = select(0.75 + fresnel * 0.2, 0.9, is_underwater);

    return vec4(final_color, alpha);
}