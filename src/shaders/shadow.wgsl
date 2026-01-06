/// Shadow Mapping Shader
/// 
/// This shader is used during the shadow pass to generate a depth map 
/// from the light's (sun's) perspective.
/// 
/// Improvements:
/// - Wave displacement for top-facing faces (normal.y > 0.5), synced exactly
///   with water vertex shader. Enables accurate self-shadowing on animated
///   water surfaces without artifacts.
/// - Consistent with water rendering: same wave parameters, time-driven,
///   and y-offset to prevent z-fighting.
/// - No fragment shader needed (depth-only pass).

struct Uniforms {
    /// Standard view-projection matrix (not used in shadow pass)
    view_proj: mat4x4<f32>,
    /// Inverse view-projection matrix
    inv_view_proj: mat4x4<f32>,
    /// CSM cascade view-projection matrices (use cascade 0 for shadow pass)
    csm_view_proj: array<mat4x4<f32>, 4>,
    /// View-space split distances for cascade selection
    csm_split_distances: vec4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_position: vec3<f32>,
    is_underwater: f32,
    /// Unused in shadow pass but kept for uniform buffer compatibility
    screen_size: vec2<f32>,
    water_level: f32,
    reflection_mode: f32,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) tex_index: f32,
    @location(5) roughness: f32,
    @location(6) metallic: f32,
};

/// Shadow Vertex Shader
///
/// Transforms the vertex position into the sun's coordinate space.
/// Applies identical wave displacement as water VS for top-facing water surfaces.
@vertex
fn vs_shadow(model: VertexInput) -> @builtin(position) vec4<f32> {
    var pos = model.position;
    
    // Wave displacement for top-facing faces (water surfaces) - EXACTLY matches water VS
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

    return uniforms.csm_view_proj[0] * vec4<f32>(pos, 1.0);
}