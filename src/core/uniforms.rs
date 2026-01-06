use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    /// CSM cascade view-projection matrices (4 cascades)
    pub csm_view_proj: [[[[f32; 4]; 4]; 1]; 4],
    /// View-space split distances for each cascade
    pub csm_split_distances: [f32; 4],
    pub camera_pos: [f32; 3],
    pub time: f32,
    pub sun_position: [f32; 3],
    /// 1.0 if camera is underwater, 0.0 otherwise
    pub is_underwater: f32,
    /// Screen dimensions for SSR calculations
    pub screen_size: [f32; 2],
    /// Water level for reflections
    pub water_level: f32,
    /// Reflection mode: 0=off, 1=SSR only
    pub reflection_mode: f32,
}
