struct ShadowUniforms {
    cascade_view_proj: mat4x4<f32>,
    time: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: ShadowUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) color: vec4<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) tex_index: f32,
};

struct WaveParams {
    wavelength: f32,
    amplitude: f32,
    steepness: f32,
    direction: vec2<f32>,
    speed: f32,
};

fn get_wave(idx: i32) -> WaveParams {
    switch (idx) {
        case 0: { return WaveParams(12.0, 0.08, 0.15, vec2<f32>(1.0, 0.3), 0.5); }
        case 1: { return WaveParams(8.0, 0.04, 0.12, vec2<f32>(0.7, 0.7), 0.4); }
        case 2: { return WaveParams(5.0, 0.03, 0.1, vec2<f32>(-0.5, 0.8), 0.7); }
        case 3: { return WaveParams(2.5, 0.015, 0.05, vec2<f32>(0.2, -0.9), 1.2); }
        default: { return WaveParams(1.0, 0.0, 0.0, vec2<f32>(1.0, 0.0), 1.0); }
    }
}

fn calculate_wave_y(pos: vec3<f32>, time: f32) -> f32 {
    var y_offset: f32 = 0.0;
    let p = pos.xz;

    for (var i: i32 = 0; i < 4; i++) {
        let w = get_wave(i);
        let k = 2.0 * 3.14159265359 / w.wavelength;
        let c = sqrt(9.8 / k) * w.speed;
        let d = normalize(w.direction);
        let f = k * (dot(d, p) - c * time);
        y_offset += w.amplitude * sin(f);
    }
    return y_offset;
}

@vertex
fn vs_shadow(model: VertexInput) -> @builtin(position) vec4<f32> {
    var pos = model.position;

    if model.normal.y > 0.5 {
        pos.y += calculate_wave_y(pos, uniforms.time);
        pos.y -= 0.15;
    }

    return uniforms.cascade_view_proj * vec4<f32>(pos, 1.0);
}