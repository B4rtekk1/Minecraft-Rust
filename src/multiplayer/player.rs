use wgpu_glyph::{Section, Text};

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

/// Queue remote player usernames as text labels (rendered above their heads)
pub fn queue_remote_players_labels(
    glyph_brush: &mut wgpu_glyph::GlyphBrush<()>,
    remote_players: &std::collections::HashMap<u32, RemotePlayer>,
    view_proj: &cgmath::Matrix4<f32>,
    width: f32,
    height: f32,
) {
    // Render each remote player's username above their head
    for (_id, player) in remote_players {
        let pos = cgmath::Vector4::new(player.x, player.y + 2.2, player.z, 1.0);
        let clip_pos = view_proj * pos;

        if clip_pos.w > 0.0 {
            let screen_x = (clip_pos.x / clip_pos.w + 1.0) / 2.0 * width;
            let screen_y = (1.0 - clip_pos.y / clip_pos.w) / 2.0 * height;

            // Draw nickname
            glyph_brush.queue(Section {
                screen_position: (screen_x, screen_y),
                text: vec![
                    Text::new(&player.username)
                        .with_color([0.3, 1.0, 0.3, 1.0])
                        .with_scale(24.0),
                ],
                layout: wgpu_glyph::Layout::default()
                    .h_align(wgpu_glyph::HorizontalAlign::Center)
                    .v_align(wgpu_glyph::VerticalAlign::Center),
                ..Section::default()
            });
        }
    }
}
