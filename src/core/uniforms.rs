use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub csm_view_proj: [[[[f32; 4]; 4]; 1]; 4],
    pub csm_split_distances: [f32; 4],
    pub camera_pos: [f32; 3],
    pub time: f32,
    pub sun_position: [f32; 3],
    pub is_underwater: f32,
    pub screen_size: [f32; 2],
    pub water_level: f32,
    pub reflection_mode: f32,
    pub moon_position: [f32; 3],
    pub _pad1_moon: f32,
}
