/// Terrain Rendering Shader
///
/// This shader handles the rendering of solid terrain blocks (grass, dirt, stone, etc.).
/// It includes support for:
/// - Texture Array based atlas sampling
/// - High-quality PCF shadows using Vogel Disk sampling with world-space noise for temporal stability
/// - Time-of-day based lighting (ambient, solar diffuse, secondary fill light)
/// - Biome-aware fog and atmospheric scattering

struct Uniforms {
    /// Projection * View matrix for the camera
    view_proj: mat4x4<f32>,
    /// Inverse of Project * View matrix for unprojecting
    inv_view_proj: mat4x4<f32>,
    /// Projection * View matrix from the sun's perspective (for shadow mapping)
    sun_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_position: vec3<f32>,
    /// 1.0 if camera is underwater, 0.0 otherwise
    is_underwater: f32,
};


@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

/// Array of 2D textures containing block faces
@group(0) @binding(1)
var texture_atlas: texture_2d_array<f32>;
@group(0) @binding(2)
var texture_sampler: sampler;
/// Depth map generated during the shadow pass
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
const PCF_SAMPLES: i32 = 12;

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

/// Generates a sample offset on a Vogel Disk.
/// This provides a very uniform distribution of samples for soft shadows.
fn vogel_disk_sample(sample_index: i32, sample_count: i32, phi: f32) -> vec2<f32> {
    let r = sqrt(f32(sample_index) + 0.5) / sqrt(f32(sample_count));
    let theta = f32(sample_index) * GOLDEN_ANGLE + phi;
    return vec2<f32>(r * cos(theta), r * sin(theta));
}

/// Simple hashing function for stable world-space noise.
/// Used to rotate PCF samples differently for each pixel to hide banding artifacts.
fn world_space_noise(world_pos: vec3<f32>) -> f32 {
    let p = world_pos * 0.5; // Scale down for smoother distribution
    return fract(sin(dot(floor(p.xz), vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

/// Percentage Closer Filtering (PCF) shadow calculation.
/// Uses multiple samples from the shadow map to calculate a fractional shadow value [0.0 - 1.0].
fn calculate_shadow(world_pos: vec3<f32>, normal: vec3<f32>, sun_dir: vec3<f32>) -> f32 {
    // Disable shadows if sun is below horizon
    if sun_dir.y < 0.05 {
        return 0.0;
    }

    // No normal-based offset - rely on pipeline depth bias to prevent shadow acne
    // This ensures shadows start exactly at object bases
    let offset_world_pos = world_pos;
    
    // Project world coordinates to shadow map UVs
    let shadow_pos = uniforms.sun_view_proj * vec4<f32>(offset_world_pos, 1.0);
    let shadow_coords = shadow_pos.xyz / shadow_pos.w;

    let uv = vec2<f32>(
        shadow_coords.x * 0.5 + 0.5,
        1.0 - (shadow_coords.y * 0.5 + 0.5)
    );
    
    // Smoothly fade shadows at the edges of the shadow frustum
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
    
    // Randomize sample rotation per pixel using world-space noise
    let noise = world_space_noise(world_pos);
    let rotation_angle = noise * 2.0 * PI;

    let texel_size = 1.0 / SHADOW_MAP_SIZE;
    let filter_radius = 3.5 * texel_size;

    var shadow: f32 = 0.0;

    // Accumulate shadow samples
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
    
    // Re-map shadow intensity for better contrast
    shadow = smoothstep(0.05, 0.95, shadow);

    return mix(1.0, shadow, edge_shadow_blend);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample block texture from array
    let tex_sample = textureSample(texture_atlas, texture_sampler, in.uv, i32(in.tex_index + 0.5));
    
    // Alpha test (for leaves, etc.)
    if tex_sample.a < 0.5 {
        discard;
    }

    let tex_color = tex_sample.rgb;
    let sun_dir = normalize(uniforms.sun_position);
    
    // Calculate view direction from camera to this fragment (for localized sky gradient)
    let view_dir = normalize(in.world_pos - uniforms.camera_pos);
    
    // --- LIGHTING MODEL ---
    
    // Time-of-day factors
    let day_factor = clamp(sun_dir.y, 0.0, 1.0);
    let night_factor = clamp(-sun_dir.y, 0.0, 1.0);
    let sunset_factor = 1.0 - abs(sun_dir.y); 
    // Twilight factor - active during sunrise/sunset transition
    let twilight_factor = smoothstep(-0.1, 0.15, sun_dir.y) * smoothstep(0.4, 0.0, sun_dir.y);
    
    // Calculate sky color with localized sunset effect based on view direction
    let sky_color = calculate_sky_color(view_dir, sun_dir);
    
    // Primary solar shadow
    var shadow = 1.0;
    if sun_dir.y > 0.0 {
        shadow = calculate_shadow(in.world_pos, in.normal, sun_dir);
    }
    
    // Ambient light - add twilight boost during sunrise/sunset
    let ambient_day = 0.4;
    let ambient_night = 0.005;
    let ambient_twilight = 0.25; // Extra ambient during sunrise/sunset
    var ambient = mix(ambient_night, ambient_day, day_factor);
    ambient = max(ambient, ambient_twilight * twilight_factor);
    
    // Main sun diffuse component
    let sun_diffuse = max(dot(in.normal, sun_dir), 0.0) * 0.5 * shadow * day_factor;
    
    // Secondary "fill" light (from opposite side) to ground objects
    let fill_dir = normalize(vec3<f32>(-sun_dir.x, 0.5, -sun_dir.z));
    let fill_diffuse = max(dot(in.normal, fill_dir), 0.0) * 0.1 * day_factor;
    
    // Directional shading for block faces (mimic Minecraft look)
    var face_shade = 1.0;
    if abs(in.normal.y) > 0.5 {
        if in.normal.y > 0.0 {
            face_shade = 1.0; // Top
        } else {
            face_shade = 0.5; // Bottom
        }
    } else if abs(in.normal.x) > 0.5 {
        face_shade = 0.7; // X-sides
    } else {
        face_shade = 0.8; // Z-sides
    }

    let effective_face_shade = mix(1.0, face_shade, day_factor + 0.3);

    let lighting = (ambient + sun_diffuse + fill_diffuse) * effective_face_shade;

    var lit_color = tex_color * lighting;
    
    // Apply sunset tint to lit surfaces
    if sunset_factor > 0.3 && sun_dir.y > -0.2 {
        let sunset_tint = vec3<f32>(1.0, 0.85, 0.7);
        lit_color = lit_color * mix(vec3<f32>(1.0), sunset_tint, sunset_factor * 0.5);
    }
    
    // --- FOG CALCULATION ---

    let dist = length(in.world_pos.xz - uniforms.camera_pos.xz);
    
    // Check if underwater
    let is_underwater = uniforms.is_underwater > 0.5;
    
    // Visibility range depends on time of day and underwater state
    var visibility_range: f32;
    var fog_color: vec3<f32>;

    if is_underwater {
        // Underwater: very short visibility, blue-green tint
        visibility_range = 24.0;
        fog_color = vec3<f32>(0.05, 0.15, 0.3);
    } else {
        let visibility_night = 20.0;  // Much shorter at night - objects should disappear in darkness
        let visibility_day = 250.0;
        let visibility_twilight = 100.0;
        // Better visibility during twilight than pure night
        visibility_range = mix(visibility_night, visibility_day, day_factor);
        visibility_range = max(visibility_range, visibility_twilight * twilight_factor);
        
        // Fog color: at night, fog must match the night sky exactly to hide silhouettes
        let night_fog_color = vec3<f32>(0.001, 0.001, 0.008);  // Must match zenith_night color
        let twilight_blend = max(day_factor, twilight_factor * 0.7);
        fog_color = mix(night_fog_color, sky_color, twilight_blend);
    }

    let fog_start = visibility_range * 0.2;
    let fog_end = visibility_range;

    let visibility = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);

    var final_color = mix(fog_color, lit_color, visibility);
    
    // Apply underwater color filter
    if is_underwater {
        // Blue-green color shift
        let water_tint = vec3<f32>(0.4, 0.7, 1.0);
        final_color = final_color * water_tint;
        
        // Add subtle caustic-like brightness variations
        let caustic = sin(in.world_pos.x * 0.5 + uniforms.time * 2.0) * sin(in.world_pos.z * 0.5 + uniforms.time * 1.5) * 0.1 + 0.9;
        final_color = final_color * caustic;
        
        // Darken with depth (simulate light absorption)
        let depth_factor = clamp(dist / visibility_range, 0.0, 1.0);
        final_color = mix(final_color, fog_color, depth_factor * 0.5);
    }

    return vec4<f32>(final_color, 1.0);
}
