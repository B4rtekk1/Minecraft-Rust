struct CloudUniforms {
    time: f32,
    wind_direction: vec2<f32>,
    sun_direction: vec2<f32>,
    coverage: f32,
    density: f32,
    steps: u32,
    _padding: u32,  // Align to 16 bytes (40 total)
}

@group(0) @binding(0) var<uniform> uniforms: CloudUniforms;
@group(0) @binding(1) var input_texture: texture_2d<f32>;
@group(0) @binding(2) var input_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let uv = vec2<f32>(f32(vertex_index & 1u), f32((vertex_index >> 1u) & 1u)) * 2.0;
    let pos = uv * 2.0 - 1.0;
    return VertexOutput(vec4<f32>(pos, 0.0, 1.0), uv);
}

fn hash3(p: vec3<f32>) -> f32 {
    let p3 = fract(p * 0.1031);
    return fract((p3.x + p3.y) * p3.z + dot(p3, vec3<f32>(19.19, 19.19, 19.19)));
}

fn fbm3d(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 1.0;
    var frequency = 1.0;
    var max_value = 0.0;

    for (var i = 0; i < octaves; i += 1) {
        let sample_point = p * frequency;
        let grid = floor(sample_point);
        let frac = fract(sample_point);
        let u = frac * frac * (3.0 - 2.0 * frac);

        let h000 = hash3(grid);
        let h100 = hash3(grid + vec3<f32>(1.0, 0.0, 0.0));
        let h010 = hash3(grid + vec3<f32>(0.0, 1.0, 0.0));
        let h110 = hash3(grid + vec3<f32>(1.0, 1.0, 0.0));
        let h001 = hash3(grid + vec3<f32>(0.0, 0.0, 1.0));
        let h101 = hash3(grid + vec3<f32>(1.0, 0.0, 1.0));
        let h011 = hash3(grid + vec3<f32>(0.0, 1.0, 1.0));
        let h111 = hash3(grid + vec3<f32>(1.0, 1.0, 1.0));

        let x00 = mix(h000, h100, u.x);
        let x10 = mix(h010, h110, u.x);
        let x01 = mix(h001, h101, u.x);
        let x11 = mix(h011, h111, u.x);

        let y0 = mix(x00, x10, u.y);
        let y1 = mix(x01, x11, u.y);

        let noise = mix(y0, y1, u.z) * 2.0 - 1.0;

        value += noise * amplitude;
        max_value += amplitude;

        amplitude *= 0.5;
        frequency *= 2.0;
    }

    return value / max_value;
}

// USUNIĘTO compute_shadow - nie jest potrzebne bez volumetric lighting

fn cloud_density(p: vec3<f32>) -> f32 {
    let wind = uniforms.wind_direction * uniforms.time * 0.1;
    let wind_pos = p + vec3<f32>(wind.x, 0.0, wind.y);

    let base_noise = fbm3d(wind_pos * 0.8, 3);
    let detail = fbm3d(wind_pos * 3.5, 1);

    let combined = base_noise * 0.7 + detail * 0.3;
    let density = smoothstep(uniforms.coverage - 0.2, uniforms.coverage + 0.1, combined);

    return density * uniforms.density;
}

// Prostszy raymarch bez self-shadowing
fn raymarch_clouds(ray_origin: vec3<f32>, ray_dir: vec3<f32>, max_dist: f32, num_steps: i32) -> vec4<f32> {
    var accumulated_color = vec3<f32>(0.0);
    var accumulated_alpha = 0.0;

    let step_size = max_dist / f32(num_steps);
    var ray_pos = ray_origin;
    let sun_dir = normalize(vec3<f32>(uniforms.sun_direction.x, 1.0, uniforms.sun_direction.y));
    
    // Prosty lighting bez raymarching
    let light_dot = dot(ray_dir, sun_dir);
    let phase = 0.25 * (1.0 + light_dot * light_dot);

    for (var i = 0; i < num_steps; i += 1) {
        if accumulated_alpha > 0.95 {
            break;
        }

        let current_density = cloud_density(ray_pos);

        if current_density > 0.001 {
            // Prosty lighting bazowany tylko na kierunku słońca
            let light_factor = max(0.3, phase);

            let cloud_color = mix(
                vec3<f32>(0.7, 0.7, 0.75),  // Ciemniejsza podstawa
                vec3<f32>(1.0, 0.98, 0.95), // Jasne podświetlenie
                light_factor
            );

            let alpha = current_density * step_size * 1.2;
            let blend = 1.0 - accumulated_alpha;
            accumulated_color += blend * cloud_color * alpha;
            accumulated_alpha += blend * alpha;
        }

        ray_pos += ray_dir * step_size;
    }
    
    // Podstawowe ambient light
    accumulated_color += vec3<f32>(0.3, 0.35, 0.4) * accumulated_alpha * 0.2;

    return vec4<f32>(accumulated_color, accumulated_alpha);
}

fn sample_bloom(uv: vec2<f32>) -> vec3<f32> {
    var bloom = vec3<f32>(0.0);
    let radius = 0.008;

    for (var i = -2; i <= 2; i += 1) {
        for (var j = -2; j <= 2; j += 1) {
            let offset = vec2<f32>(f32(i), f32(j)) * radius;
            let sample = textureSample(input_texture, input_sampler, uv + offset);
            let brightness = dot(sample.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
            bloom += sample.rgb * max(0.0, brightness - 0.7);
        }
    }

    return bloom * 0.04;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;

    let ray_origin = vec3<f32>(uv * 2.0 - 1.0, 2.0);
    let ray_dir = normalize(vec3<f32>(uv - 0.5, -1.5));

    let cloud_result = raymarch_clouds(ray_origin, ray_dir, 8.0, i32(uniforms.steps));

    let temporal_offset = sin(uniforms.time) * 0.001;
    let prev_sample = textureSample(input_texture, input_sampler,
        clamp(uv + vec2<f32>(temporal_offset), vec2<f32>(0.0), vec2<f32>(1.0)));
    let blended = mix(cloud_result, prev_sample, 0.2);

    let bloom = sample_bloom(uv);
    let with_bloom = blended.rgb + bloom * 0.3;

    let tone_mapped = with_bloom / (with_bloom + vec3<f32>(1.0));
    let final_color = pow(tone_mapped, vec3<f32>(0.4545));

    return vec4<f32>(final_color, blended.a);
}