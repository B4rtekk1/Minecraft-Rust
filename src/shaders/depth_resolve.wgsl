@group(0) @binding(0) var msaa_depth: texture_depth_multisampled_2d;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Fullscreen triangle trick (covers screen with 3 vertices)
    let x = f32((vertex_index << 1u) & 2u);
    let y = f32(vertex_index & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) pos: vec4<f32>) -> @builtin(frag_depth) f32 {
    let coords = vec2<i32>(pos.xy);
    
    // Resolve MSAA depth by averaging all 4 samples
    var depth_sum = 0.0;
    for (var i = 0; i < 4; i++) {
        depth_sum += textureLoad(msaa_depth, coords, i);
    }
    let resolved_depth = depth_sum / 4.0;
    // return vec4<f32>(resolved_depth, 0.0, 0.0, 1.0);
    return resolved_depth;
}
