/// Water Rendering Shader
///
/// This shader handles the rendering of transparent water surfaces.
/// It includes support for:
/// - Procedural vertex wave displacement (sinusoidal)
/// - Fresnel-based sky reflections
/// - Improved Screen Space Reflections (SSR) with accelerating steps and best-hit search
/// - Specular highlights for sun and moon
/// - Procedural shimmer/glitter effects
/// - Depth-based fog and alpha blending
/// - Subtle screen-space refraction for added realism
/// - Dynamic SSR thickness

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
    reflection_mode: f32, // 0=off, 1=SSR
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var texture_atlas: texture_2d_array<f32>;
@group(0) @binding(2)
var texture_sampler: sampler;
@group(0) @binding(3)
var shadow_map: texture_depth_2d_array;
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
    @location(5) roughness: f32,
    @location(6) metallic: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) tex_index: f32,
    @location(5) roughness: f32,
    @location(6) metallic: f32,
    @location(7) view_depth: f32,
};

// ============================================================================
// GERSTNER WAVES - Realistic ocean wave simulation
// ============================================================================

/// Single Gerstner wave calculation
/// Returns: vec3(horizontal_displacement_x, vertical_displacement, horizontal_displacement_z)
/// 
/// Gerstner waves create realistic circular orbital motion of water particles,
/// producing sharper peaks and flatter troughs than simple sine waves.
fn gerstner_wave(
    pos: vec2<f32>,        // XZ world position
    time: f32,             // Animation time
    wavelength: f32,       // Distance between wave crests
    amplitude: f32,        // Wave height
    steepness: f32,        // 0-1, controls sharpness (Q factor)
    direction: vec2<f32>   // Normalized wave direction
) -> vec3<f32> {
    let k = 2.0 * 3.14159265359 / wavelength;  // Wave number
    let c = sqrt(9.8 / k);                      // Phase speed (gravity-based)
    let d = normalize(direction);
    let f = k * (dot(d, pos) - c * time);       // Phase

    let a = steepness / k;  // Amplitude adjusted by steepness

    return vec3<f32>(
        d.x * a * cos(f),   // X displacement (horizontal)
        amplitude * sin(f), // Y displacement (vertical height)
        d.y * a * cos(f)    // Z displacement (horizontal)
    );
}

/// Calculate combined Gerstner waves with LOD (Level of Detail)
/// Waves fade out based on distance to camera for performance
fn calculate_gerstner_displacement(pos: vec3<f32>, time: f32, camera_pos: vec3<f32>) -> vec3<f32> {
    // Distance-based LOD factor
    let dist = length(pos.xz - camera_pos.xz);
    
    // Fade waves from 0-80 blocks (full detail to no detail)
    let lod_near = 0.0;
    let lod_far = 80.0;
    let lod_factor = 1.0 - clamp((dist - lod_near) / (lod_far - lod_near), 0.0, 1.0);
    
    // Early exit for distant water (save GPU cycles)
    if lod_factor < 0.01 {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    
    // Smooth LOD transition (ease out)
    let smooth_lod = lod_factor * lod_factor;

    var displacement = vec3<f32>(0.0, 0.0, 0.0);
    let p = pos.xz;
    
    // Wave 1: Primary swell (slow, realistic)
    displacement += gerstner_wave(
        p, time * 0.5,             // slow ocean swell
        10.0,                      // wavelength
        0.07 * smooth_lod,         // amplitude
        0.15,                      // low steepness = stable at shores
        vec2<f32>(1.0, 0.3)        // direction
    );
    
    // Wave 2: Secondary swell (crossing angle)
    displacement += gerstner_wave(
        p, time * 0.4,
        6.0,
        0.032 * smooth_lod,
        0.12,
        vec2<f32>(0.7, 0.7)
    );
    
    // Wave 3: Gentle chop
    displacement += gerstner_wave(
        p, time * 0.7,
        4.0,
        0.023 * smooth_lod,
        0.1,
        vec2<f32>(-0.5, 0.8)
    );
    
    // Wave 4: Fine ripples - only close up
    let detail_lod = smooth_lod * smooth_lod;
    displacement += gerstner_wave(
        p, time * 1.0,
        2.0,
        0.015 * detail_lod,
        0.08,
        vec2<f32>(0.2, -0.9)
    );

    return displacement;
}

/// Water Vertex Shader
///
/// Displaces water vertices using Gerstner waves for realistic ocean-like motion.
/// Uses only vertical (Y) displacement to prevent water from separating at shores.
/// LOD reduces wave detail at distance for better performance.
@vertex
fn vs_water(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    var pos = model.position;
    if model.normal.y > 0.5 {
        // Apply Gerstner wave displacement - Y only (no horizontal to prevent shore gaps)
        let wave_offset = calculate_gerstner_displacement(pos, uniforms.time, uniforms.camera_pos);
        pos.y += wave_offset.y;
        
        // Slightly lower water level to prevent z-fighting with adjacent solid blocks
        pos.y -= 0.15;
    }

    out.clip_position = uniforms.view_proj * vec4<f32>(pos, 1.0);
    out.world_pos = pos;
    out.normal = model.normal;
    out.color = model.color;
    out.uv = model.uv;
    out.tex_index = model.tex_index;
    out.view_depth = out.clip_position.w;
    return out;
}

const PI: f32 = 3.14159265359;
const SHADOW_MAP_SIZE: f32 = 4096.0;
const PCF_SAMPLES: i32 = 16;
const SSR_MAX_STEPS: i32 = 64;
const SSR_MAX_DISTANCE: f32 = 60.0;
const SSR_THICKNESS_BASE: f32 = 0.05;
const SSR_THICKNESS_SCALE: f32 = 0.001;

/// Improved Screen Space Reflections ray marching
/// Uses accelerating steps, best-hit search, and dynamic thickness for better accuracy and fewer artifacts.
/// Returns vec4 where xyz = reflected color, w = confidence (0 = no hit, 1 = solid hit)
fn ssr_trace(
    world_pos: vec3<f32>,
    reflect_dir: vec3<f32>,
    clip_pos: vec4<f32>
) -> vec4<f32> {
    var ray_pos = world_pos;
    let dir = normalize(reflect_dir);

    var best_uv = vec2<f32>(0.0);
    var best_depth_diff = 1e10;
    var best_confidence = 0.0;

    for (var i: i32 = 0; i < SSR_MAX_STEPS; i++) {
        // Accelerating step size: start small, grow larger for efficiency
        let step_progress = f32(i) / f32(SSR_MAX_STEPS);
        let step_dist = mix(0.2, SSR_MAX_DISTANCE / f32(SSR_MAX_STEPS) * 4.0, step_progress);
        ray_pos += dir * step_dist;
        
        // Project to clip/NDC
        let ray_clip = uniforms.view_proj * vec4<f32>(ray_pos, 1.0);
        if ray_clip.w <= 0.0 { break; }
        let ray_ndc = ray_clip.xyz / ray_clip.w;
        
        // UV (flip Y for WebGPU)
        let ray_uv = vec2<f32>(ray_ndc.x * 0.5 + 0.5, 1.0 - (ray_ndc.y * 0.5 + 0.5));
        
        // Bounds check
        if ray_uv.x < 0.0 || ray_uv.x > 1.0 || ray_uv.y < 0.0 || ray_uv.y > 1.0 {
            break;
        }
        
        // Sample depth
        let scene_depth = textureSample(ssr_depth, ssr_sampler, ray_uv);
        let ray_depth = ray_ndc.z;
        
        // Dynamic thickness: tighter near camera, looser far
        let thickness = SSR_THICKNESS_BASE + abs(ray_depth) * SSR_THICKNESS_SCALE;

        let depth_diff = ray_depth - scene_depth;
        if depth_diff > 0.0 && depth_diff < thickness {
            // Better hit?
            if depth_diff < best_depth_diff {
                best_depth_diff = depth_diff;
                best_uv = ray_uv;
                best_confidence = 1.0 - (depth_diff / thickness);
            }
        }
    }

    if best_confidence > 0.01 {
        let scene_color = textureSample(ssr_color, ssr_sampler, best_uv).rgb;
        
        // Edge fade vignette
        let edge_x = min(best_uv.x, 1.0 - best_uv.x);
        let edge_y = min(best_uv.y, 1.0 - best_uv.y);
        let edge_fade = min(edge_x, edge_y) * 20.0;
        let confidence = best_confidence * clamp(edge_fade, 0.0, 1.0);

        return vec4<f32>(scene_color, confidence);
    }

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
    if v_len > 0.0001 && s_len > 0.0001 {
        cos_angle_horizontal = dot(view_horizontal_vec / v_len, sun_horizontal_vec / s_len);
    }
    
    // 3D angle to sun
    let cos_angle_3d = dot(normalize(view_dir), normalize(sun_dir));
    
    // --- BASE SKY COLORS (Unified) ---
    let zenith_day = vec3<f32>(0.25, 0.45, 0.85);
    let horizon_day = vec3<f32>(0.65, 0.82, 0.98);
    let zenith_night = vec3<f32>(0.001, 0.001, 0.008);
    let horizon_night = vec3<f32>(0.015, 0.015, 0.03);

    // Use a smooth mapping that keeps the horizon color more prominent but avoids hard cuts.
    let height_factor = clamp(view_height * 0.5 + 0.5, 0.0, 1.0);
    let curved_height = pow(height_factor, 0.8); // Slightly bias towards zenith for clearer sky
    var sky_color = mix(horizon_day, zenith_day, curved_height) * day_factor;
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

    return clamp(sky_color, vec3<f32>(0.0), vec3<f32>(1.5));
}

fn get_poisson_sample(idx: i32, rotation: f32) -> vec2<f32> {
    var p: vec2<f32>;
    switch (idx) {
        case 0: { p = vec2<f32>(-0.94201624, -0.39906216); }
        case 1: { p = vec2<f32>(0.94558609, -0.76890725); }
        case 2: { p = vec2<f32>(-0.094184101, -0.92938870); }
        case 3: { p = vec2<f32>(0.34495938, 0.29387760); }
        case 4: { p = vec2<f32>(-0.91588581, 0.45771432); }
        case 5: { p = vec2<f32>(-0.81544232, -0.87912464); }
        case 6: { p = vec2<f32>(-0.38277543, 0.27676845); }
        case 7: { p = vec2<f32>(0.97484398, 0.75648379); }
        case 8: { p = vec2<f32>(0.44323325, -0.97511554); }
        case 9: { p = vec2<f32>(0.53742981, -0.47373420); }
        case 10: { p = vec2<f32>(-0.65476012, -0.051473853); }
        case 11: { p = vec2<f32>(0.18395645, 0.89721549); }
        case 12: { p = vec2<f32>(-0.097153940, -0.006734560); }
        case 13: { p = vec2<f32>(0.53472400, 0.73356543); }
        case 14: { p = vec2<f32>(-0.45611231, -0.40212851); }
        case 15: { p = vec2<f32>(-0.57321081, 0.65476012); }
        default: { p = vec2<f32>(0.0, 0.0); }
    }
    let s = sin(rotation);
    let c = cos(rotation);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
}

fn interleaved_gradient_noise(frag_coord: vec2<f32>) -> f32 {
    let magic = vec3<f32>(0.06711056, 0.00583715, 52.9829189);
    return fract(magic.z * fract(dot(frag_coord, magic.xy)));
}

/// Select cascade based on view-space depth
fn select_cascade(view_depth: f32) -> i32 {
    if view_depth < uniforms.csm_split_distances.x {
        return 0;
    } else if view_depth < uniforms.csm_split_distances.y {
        return 1;
    } else if view_depth < uniforms.csm_split_distances.z {
        return 2;
    }
    return 3;
}

/// Calculate perturbed water normal for realistic ripple reflections
/// Uses derivative of wave functions with noise and crossing wave patterns
fn calculate_water_normal(world_pos: vec3<f32>, time: f32) -> vec3<f32> {
    // Add noise offset to break up regularity
    let noise1 = fract(sin(dot(world_pos.xz * 0.1, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    let noise2 = fract(sin(dot(world_pos.xz * 0.15, vec2<f32>(39.346, 11.135))) * 43758.5453);
    let noise_offset = (noise1 - 0.5) * 0.3;
    
    // Primary waves with noise-varied phases and non-aligned frequencies
    let wave1_dx = cos(world_pos.x * 0.37 + time * 1.8 + noise_offset) * 0.03;
    let wave2_dz = cos(world_pos.z * 0.43 + time * 1.4 + noise_offset) * 0.025;
    
    // Diagonal crossing waves (different angles) - breaks up parallel lines
    let diag1 = cos((world_pos.x * 0.7 + world_pos.z * 0.7) * 0.45 + time * 2.2) * 0.02;
    let diag2 = cos((world_pos.x * 0.7 - world_pos.z * 0.7) * 0.51 + time * 1.9) * 0.018;
    
    // Different angle waves (30, 60 degrees) for more organic look
    let angle1 = cos((world_pos.x * 0.866 + world_pos.z * 0.5) * 0.62 + time * 2.1) * 0.015;
    let angle2 = cos((world_pos.x * 0.5 + world_pos.z * 0.866) * 0.55 + time * 1.6) * 0.015;
    let angle3 = cos((world_pos.x * 0.866 - world_pos.z * 0.5) * 0.71 + time * 2.4) * 0.012;
    
    // Fine detail ripples with noise variation
    let ripple_scale = 1.0 + noise2 * 0.3;
    let ripple1 = cos(world_pos.x * 2.8 * ripple_scale + time * 4.2) * 0.01;
    let ripple2 = cos(world_pos.z * 3.1 * ripple_scale + time * 3.8) * 0.009;
    let ripple3 = cos((world_pos.x + world_pos.z) * 1.9 + time * 5.2) * 0.007;
    let ripple4 = cos((world_pos.x - world_pos.z) * 2.3 + time * 4.7) * 0.007;
    
    // Sum all derivatives for X and Z
    // Diagonal waves contribute to both X and Z based on their direction
    let dx = wave1_dx + diag1 * 0.7 + diag2 * 0.7 + angle1 * 0.866 + angle2 * 0.5 + angle3 * 0.866 + ripple1 + ripple3 + ripple4;
    let dz = wave2_dz + diag1 * 0.7 - diag2 * 0.7 + angle1 * 0.5 + angle2 * 0.866 - angle3 * 0.5 + ripple2 + ripple3 - ripple4;
    
    // Normal from tangent plane: n = normalize((-dx, 1, -dz))
    return normalize(vec3<f32>(-dx, 1.0, -dz));
}

fn calculate_shadow(world_pos: vec3<f32>, normal: vec3<f32>, sun_dir: vec3<f32>, view_depth: f32, frag_coord: vec2<f32>) -> f32 {
    if sun_dir.y < 0.05 {
        return 0.0;
    }

    let cascade_idx = select_cascade(view_depth);
    
    // No normal-based offset - rely on pipeline depth bias to prevent shadow acne
    let offset_world_pos = world_pos;

    let shadow_pos = uniforms.csm_view_proj[cascade_idx] * vec4<f32>(offset_world_pos, 1.0);
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

    let noise = interleaved_gradient_noise(frag_coord);
    let rotation_angle = noise * 2.0 * PI;

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

/// Water Fragment Shader
///
/// Implements a water lighting model with Fresnel reflections, specular highlights,
/// procedural shimmer, SSR, and subtle refraction.
@fragment
fn fs_water(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(texture_atlas, texture_sampler, in.uv, i32(in.tex_index + 0.5)).rgb;
    var base_water = tex_color;
    
    // Higher-frequency ripple normal for specular/sparkle
    let spark_normal = calculate_water_normal(in.world_pos * 2.0, uniforms.time * 1.5);
    
    // Calculate perturbed normal for realistic ripple reflections
    let perturbed_normal = calculate_water_normal(in.world_pos, uniforms.time);
    
    // Blend between vertex normal and perturbed normal based on distance
    // (less perturbation at distance to avoid noise)
    let dist_to_camera = length(in.world_pos - uniforms.camera_pos);
    let normal_blend = clamp(1.0 - dist_to_camera / 100.0, 0.3, 1.0);
    let water_normal = normalize(mix(in.normal, perturbed_normal, normal_blend));
    
    // Fresnel effect: water is more reflective when viewed at a grazing angle
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);  // surface to camera
    // Fresnel effect (Schlick's approximation): 
    // Water has a base reflectance of ~0.02 when viewed top-down.
    let cos_theta = max(dot(view_dir, water_normal), 0.0);
    let r0 = 0.02;
    let fresnel = r0 + (1.0 - r0) * pow(1.0 - cos_theta, 5.0);

    let sun_dir = normalize(uniforms.sun_position);
    
    // Time of day factors
    let day_factor = clamp(sun_dir.y, 0.0, 1.0);
    let night_factor = clamp(-sun_dir.y, 0.0, 1.0);
    let sunset_factor = 1.0 - abs(sun_dir.y);
    
    // Reflection direction for SSR and sky (reflect(-view_dir, N))
    let fragment_view_dir = normalize(in.world_pos - uniforms.camera_pos);  // camera to surface
    // Stabilize reflection vector: ensure it points upwards to avoid horizon flickering
    var reflect_dir_ssr = reflect(fragment_view_dir, water_normal);
    reflect_dir_ssr.y = max(reflect_dir_ssr.y, 0.001);
    reflect_dir_ssr = normalize(reflect_dir_ssr);
    
    // Calculate sky color based on reflection direction
    let sky_color = calculate_sky_color(reflect_dir_ssr, sun_dir);
    
    // Screen UV for refraction
    let clip_ndc = in.clip_position.xyz / in.clip_position.w;
    let screen_uv = vec2<f32>(clip_ndc.x * 0.5 + 0.5, 1.0 - (clip_ndc.y * 0.5 + 0.5));
    
    // --- REFRACTION (subtle distortion through water) ---
    let refract_offset = water_normal.xz * 0.02 + perturbed_normal.xz * 0.01;
    let refract_uv = clamp(screen_uv + refract_offset * (1.0 - fresnel), vec2(0.0), vec2(1.0));
    let refract_color = textureSample(ssr_color, ssr_sampler, refract_uv).rgb;
    base_water = mix(base_water, refract_color * 0.6, 0.4 * (1.0 - fresnel));

    let reflection_mode = i32(uniforms.reflection_mode);
    var reflection_color = sky_color;

    let ssr_distance_fade = clamp(1.0 - dist_to_camera / 150.0, 0.0, 1.0);
    
    // Reflection modes
    if reflection_mode == 0 {
        // Sky only
        reflection_color = sky_color;
    } else {
        // SSR + sky fallback
        let ssr_result = ssr_trace(in.world_pos, reflect_dir_ssr, in.clip_position);
        // Heavily smooth the confidence to reduce flickering during camera movement
        // Use higher threshold (0.25) and tighter smoothstep for more stable transitions
        let smoothed_confidence = smoothstep(0.25, 0.85, ssr_result.w);
        if smoothed_confidence > 0.05 {
            // Reduce max blend to 0.5 to always keep significant sky influence for stability
            // This reduces SSR "popping" when reflections come and go
            let ssr_blend = smoothed_confidence * 0.5 * ssr_distance_fade;
            reflection_color = mix(sky_color, ssr_result.rgb, ssr_blend);
        }
    }

    var shadow = 1.0;
    if sun_dir.y > 0.0 {
        shadow = calculate_shadow(in.world_pos, in.normal, sun_dir, in.view_depth, in.clip_position.xy);
    }

    let ambient_day = 0.4;
    let ambient_night = 0.008;
    let ambient = mix(ambient_night, ambient_day, day_factor);
    
    // Mix base (refracted) color with reflected color based on Fresnel
    // Reduce fresnel multiplier for more stable appearance during camera movement
    // Mix base (refracted) color with reflected color based on Fresnel
    // Using full Fresnel for physically accurate reflection transition
    var water_color = mix(base_water, reflection_color, fresnel);
    
    // Natural glitter/sparkle based on high-frequency normal variation
    let glitter = pow(max(dot(water_normal, spark_normal), 0.0), 32.0) * 0.2;
    water_color += vec3<f32>(glitter * shadow * day_factor);
    
    // Solar specular highlights (sun glisten)
    if sun_dir.y > 0.0 {
        // Use a mix of normals for more interesting highlights:
        // - base normal for stable reflection
        // - perturbed normal for wave-shaped highlights
        // - spark normal for tiny sparkles
        let spec_normal = normalize(mix(in.normal, water_normal, 0.5));
        let sun_reflect = reflect(-sun_dir, spec_normal);
        let spec_main = pow(max(dot(view_dir, sun_reflect), 0.0), 128.0);
        
        // Sharp sparkles
        let spark_reflect = reflect(-sun_dir, spark_normal);
        let spec_spark = pow(max(dot(view_dir, spark_reflect), 0.0), 256.0);

        water_color += vec3<f32>(1.0, 0.98, 0.9) * (spec_main + spec_spark * 0.8) * shadow * day_factor;
    }
    
    // Lunar specular highlights
    if night_factor > 0.2 {
        let moon_dir = normalize(vec3<f32>(0.3, 0.5, -0.8));
        let spec_normal = normalize(mix(in.normal, water_normal, 0.4));
        let moon_reflect = reflect(-moon_dir, spec_normal);
        let moon_spec = pow(max(dot(view_dir, moon_reflect), 0.0), 64.0);
        water_color += vec3<f32>(0.7, 0.8, 1.0) * moon_spec * 0.3 * night_factor;
    }

    water_color *= (ambient + shadow * 0.6 * day_factor);
    
    // --- FOG ---
    let dist = length(in.world_pos.xz - uniforms.camera_pos.xz);
    
    // Check if underwater
    let is_underwater = uniforms.is_underwater > 0.5;

    var visibility_range: f32;
    var fog_color_final: vec3<f32>;

    if is_underwater {
        visibility_range = 20.0;
        fog_color_final = vec3<f32>(0.05, 0.15, 0.3);
    } else {
        let visibility_night = 20.0;
        let visibility_day = 250.0;
        visibility_range = mix(visibility_night, visibility_day, day_factor);
        let night_fog_color = vec3<f32>(0.001, 0.001, 0.008);
        fog_color_final = mix(night_fog_color, sky_color, day_factor);
    }

    let fog_start = visibility_range * 0.2;
    let fog_end = visibility_range;
    let visibility = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);

    var final_color = mix(fog_color_final, water_color, visibility);
    
    // Underwater tint and caustics
    if is_underwater {
        let water_tint = vec3<f32>(0.5, 0.8, 1.0);
        final_color *= water_tint;

        let caustic = sin(in.world_pos.x * 0.5 + uniforms.time * 2.0) * sin(in.world_pos.z * 0.5 + uniforms.time * 1.5) * 0.15 + 0.85;
        final_color *= caustic;
    }
    
    // Fresnel opacity + underwater opaque
    var alpha: f32;
    if is_underwater {
        alpha = 0.9;
    } else {
        alpha = 0.75 + fresnel * 0.2;
    }

    return vec4<f32>(final_color, alpha);
}