use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct PlayerSettings {
    pub fov: u32,
    pub render_distance: u32,
    pub simulation_distance: u32,
    pub brightness: u32,
}
