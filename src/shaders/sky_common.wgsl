/// Shared Sky Color Calculation
///
/// This file contains the shared sky color calculation function used by
/// terrain.wgsl and water.wgsl shaders.
///
/// INCLUDE INSTRUCTIONS:
/// Copy this function into shaders that need sky color calculations.
/// Future: Use WGSL include preprocessing or shader composition.

/// Calculate sky color with localized sunrise/sunset gradient
/// 
/// Parameters:
/// - view_dir: Normalized direction from camera to fragment (or reflection direction)
/// - sun_dir: Normalized sun direction
/// 
/// Returns: RGB sky color clamped to [0, 1]
fn calculate_sky_color(view_dir: vec3<f32>, sun_dir: vec3<f32>) -> vec3<f32> {
    let sun_height = sun_dir.y;
    
    // Time-of-day factors
    let day_factor = clamp(sun_height, 0.0, 1.0);
    let night_factor = clamp(-sun_height, 0.0, 1.0);
    let sunset_factor = 1.0 - abs(sun_height);
    
    // Vertical gradient
    let view_height = view_dir.y;
    
    // Angle between view direction and sun direction (horizontal plane)
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

    // Sun glow during day
    if day_factor > 0.1 {
        let sun_glow = pow(max(0.0, cos_angle_3d), 128.0) * day_factor;
        sky_color += vec3<f32>(1.0, 0.95, 0.9) * sun_glow;
    }

    return clamp(sky_color, vec3<f32>(0.0), vec3<f32>(1.0));
}
