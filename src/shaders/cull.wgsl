/// GPU Frustum Culling Compute Shader
///
/// Performs frustum culling on the GPU for all subchunks in parallel.
/// Visible subchunks are appended to the draw commands buffer.

struct SubchunkMeta {
    /// AABB min (xyz), padding in w
    aabb_min: vec4<f32>,
    /// AABB max (xyz), slot_index in w
    aabb_max: vec4<f32>,
    /// draw_data: index_count, first_index, base_vertex, enabled
    draw_data: vec4<u32>,
}

struct DrawIndexedIndirect {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

struct CullUniforms {
    /// 6 frustum planes (each is vec4: xyz=normal, w=distance)
    frustum_planes: array<vec4<f32>, 6>,
    /// Number of active subchunks
    subchunk_count: u32,
    /// Padding
    _padding: vec3<u32>,
}

/// Culling uniforms
@group(0) @binding(0)
var<uniform> cull_uniforms: CullUniforms;

/// All subchunk metadata (read-only)
@group(0) @binding(1)
var<storage, read> subchunks: array<SubchunkMeta>;

/// Output: visible draw commands
@group(0) @binding(2)
var<storage, read_write> draw_commands: array<DrawIndexedIndirect>;

/// Atomic counter for visible subchunks
@group(0) @binding(3)
var<storage, read_write> visible_count: atomic<u32>;

/// Test if an AABB is visible against a frustum plane
fn aabb_vs_plane(aabb_min: vec3<f32>, aabb_max: vec3<f32>, plane: vec4<f32>) -> bool {
    // Get the positive vertex (furthest along the plane normal)
    let p = vec3<f32>(
        select(aabb_min.x, aabb_max.x, plane.x > 0.0),
        select(aabb_min.y, aabb_max.y, plane.y > 0.0),
        select(aabb_min.z, aabb_max.z, plane.z > 0.0),
    );
    
    // If the positive vertex is behind the plane, AABB is fully outside
    return dot(plane.xyz, p) + plane.w >= 0.0;
}

/// Test if an AABB is inside the frustum
fn is_visible(aabb_min: vec3<f32>, aabb_max: vec3<f32>) -> bool {
    // Add margin for conservative culling
    let margin = vec3<f32>(2.0);
    let expanded_min = aabb_min - margin;
    let expanded_max = aabb_max + margin;
    
    // Test against all 6 frustum planes
    for (var i = 0u; i < 6u; i++) {
        if !aabb_vs_plane(expanded_min, expanded_max, cull_uniforms.frustum_planes[i]) {
            return false;
        }
    }
    return true;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    
    // Bounds check
    if idx >= cull_uniforms.subchunk_count {
        return;
    }

    let subchunk = subchunks[idx];
    
    // Check if this slot is enabled
    if subchunk.draw_data.w == 0u {
        return;
    }

    let aabb_min = subchunk.aabb_min.xyz;
    let aabb_max = subchunk.aabb_max.xyz;
    
    // Frustum test
    if is_visible(aabb_min, aabb_max) {
        // Atomically get slot in output array
        let slot = atomicAdd(&visible_count, 1u);
        
        // Write draw command
        draw_commands[slot].index_count = subchunk.draw_data.x;
        draw_commands[slot].instance_count = 1u;
        draw_commands[slot].first_index = subchunk.draw_data.y;
        draw_commands[slot].base_vertex = i32(subchunk.draw_data.z);
        draw_commands[slot].first_instance = 0u;
    }
}
