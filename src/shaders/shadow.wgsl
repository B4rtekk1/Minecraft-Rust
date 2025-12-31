/// Shadow Mapping Shader
/// 
/// This shader is used during the shadow pass to generate a depth map 
/// from the light's (sun's) perspective.

struct Uniforms {
    /// Standard view-projection matrix (not used in shadow pass)
    view_proj: mat4x4<f32>,
    /// Inverse view-projection matrix
    inv_view_proj: mat4x4<f32>,
    /// View-projection matrix from the sun's perspective
    sun_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_position: vec3<f32>,
    is_underwater: f32,
};


@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) tex_index: f32,
};

/// Shadow Vertex Shader
///
/// Transforms the vertex position into the sun's coordinate space.
/// No fragment shader is required as we only care about the depth buffer.
@vertex
fn vs_shadow(model: VertexInput) -> @builtin(position) vec4<f32> {
    return uniforms.sun_view_proj * vec4<f32>(model.position, 1.0);
}
