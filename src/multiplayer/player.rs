/// Remote player data
#[derive(Debug, Clone)]
pub struct RemotePlayer {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub username: String,
}

pub struct PlayerLabel {
    pub username: String,
    pub screen_x: f32,
    pub screen_y: f32,
}

/// Calculate screen positions for remote player usernames
pub fn queue_remote_players_labels(
    remote_players: &std::collections::HashMap<u32, RemotePlayer>,
    view_proj: &cgmath::Matrix4<f32>,
    width: f32,
    height: f32,
) -> Vec<PlayerLabel> {
    let mut labels = Vec::new();

    // Render each remote player's username above their head
    for (_id, player) in remote_players {
        let pos = cgmath::Vector4::new(player.x, player.y + 2.2, player.z, 1.0);
        let clip_pos = view_proj * pos;

        if clip_pos.w > 0.0 {
            let screen_x = (clip_pos.x / clip_pos.w + 1.0) / 2.0 * width;
            let screen_y = (1.0 - clip_pos.y / clip_pos.w) / 2.0 * height;

            labels.push(PlayerLabel {
                username: player.username.clone(),
                screen_x,
                screen_y,
            });
        }
    }
    labels
}
