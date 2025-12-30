use crate::multiplayer::player::RemotePlayer;
use crate::multiplayer::protocol::{Packet, decode_pitch, decode_yaw};
use crate::multiplayer::tcp::TcpClient;
use crate::ui::menu::{GameState, MenuState};
use std::time::Instant;
use winit::window::Window;

pub fn connect_to_server(
    menu_state: &mut MenuState,
    game_state: &mut GameState,
    network_runtime: &Option<tokio::runtime::Runtime>,
    network_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<Packet>>,
    network_tx: &mut Option<tokio::sync::mpsc::UnboundedSender<Packet>>,
) {
    let addr = menu_state.server_address.clone();
    let username = menu_state.username.clone();

    menu_state.set_status(&format!("Connecting to {}...", addr));
    *game_state = GameState::Connecting;

    if let Some(rt) = network_runtime {
        let result = rt.block_on(async {
            let mut client = TcpClient::new();
            client.connect(&addr).await?;
            Ok::<TcpClient, std::io::Error>(client)
        });

        match result {
            Ok(client) => {
                println!("Connected to server: {}", addr);
                let (rx_tx, rx_rx) = tokio::sync::mpsc::unbounded_channel();
                let (tx_tx, mut tx_rx) = tokio::sync::mpsc::unbounded_channel();
                *network_rx = Some(rx_rx);
                *network_tx = Some(tx_tx);

                let client_rx = client.clone();
                rt.spawn(async move {
                    while let Ok(packet) = client_rx.recv().await {
                        if rx_tx.send(packet).is_err() {
                            break;
                        }
                    }
                });

                let client_tx = client.clone();
                rt.spawn(async move {
                    while let Some(packet) = tx_rx.recv().await {
                        if client_tx.send(&packet).await.is_err() {
                            break;
                        }
                    }
                });

                let connect_packet = Packet::Connect {
                    player_id: 0,
                    username,
                };
                if let Some(tx) = network_tx {
                    let _ = tx.send(connect_packet);
                }
            }
            Err(e) => {
                eprintln!("Failed to connect: {}", e);
                menu_state.set_error(&format!("Connection failed: {}", e));
            }
        }
    }
}

pub fn update_network(
    my_player_id: &mut u32,
    camera_pos: &cgmath::Point3<f32>,
    camera_yaw: f32,
    camera_pitch: f32,
    last_position_send: &mut Instant,
    network_tx: &Option<tokio::sync::mpsc::UnboundedSender<Packet>>,
    network_rx: &mut Option<tokio::sync::mpsc::UnboundedReceiver<Packet>>,
    remote_players: &mut std::collections::HashMap<u32, RemotePlayer>,
    game_state: &mut GameState,
    mouse_captured: &mut bool,
    window: &Window,
) {
    // Send position every 50ms
    if last_position_send.elapsed().as_millis() > 50 {
        *last_position_send = Instant::now();

        let pos_packet = Packet::Position {
            player_id: *my_player_id,
            x: camera_pos.x,
            y: camera_pos.y,
            z: camera_pos.z,
        };

        let rot_packet = Packet::Rotation {
            player_id: *my_player_id,
            yaw: crate::multiplayer::protocol::encode_yaw(camera_yaw),
            pitch: crate::multiplayer::protocol::encode_pitch(camera_pitch),
        };

        if let Some(tx) = network_tx {
            let _ = tx.send(pos_packet);
            let _ = tx.send(rot_packet);
        }
    }

    // Receive packets from channel (non-blocking)
    if let Some(rx) = network_rx.as_mut() {
        while let Ok(packet) = rx.try_recv() {
            match packet {
                Packet::ConnectAck { success, player_id } => {
                    if success {
                        *my_player_id = player_id;
                        println!("Joined as Player ID: {}", player_id);
                        *game_state = GameState::Playing;

                        *mouse_captured = true;
                        let _ = window
                            .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                            .or_else(|_| {
                                window.set_cursor_grab(winit::window::CursorGrabMode::Locked)
                            });
                        window.set_cursor_visible(false);
                    } else {
                        *game_state = GameState::Menu;
                    }
                }
                Packet::Position { player_id, x, y, z } => {
                    if player_id != *my_player_id {
                        if let Some(player) = remote_players.get_mut(&player_id) {
                            player.x = x;
                            player.y = y;
                            player.z = z;
                        } else {
                            remote_players.insert(
                                player_id,
                                RemotePlayer {
                                    x,
                                    y,
                                    z,
                                    yaw: 0.0,
                                    pitch: 0.0,
                                    username: format!("Player{}", player_id),
                                },
                            );
                        }
                    }
                }
                Packet::Rotation {
                    player_id,
                    yaw,
                    pitch,
                } => {
                    if player_id != *my_player_id {
                        if let Some(player) = remote_players.get_mut(&player_id) {
                            player.yaw = decode_yaw(yaw);
                            player.pitch = decode_pitch(pitch);
                        }
                    }
                }
                Packet::Connect {
                    player_id,
                    username,
                } => {
                    println!("Player joined: {} (ID: {})", username, player_id);
                    if let Some(player) = remote_players.get_mut(&player_id) {
                        player.username = username;
                    } else {
                        remote_players.insert(
                            player_id,
                            RemotePlayer {
                                x: 0.0,
                                y: 70.0,
                                z: 0.0,
                                yaw: 0.0,
                                pitch: 0.0,
                                username,
                            },
                        );
                    }
                }
                Packet::Disconnect { player_id } => {
                    remote_players.remove(&player_id);
                    println!("Player left: ID {}", player_id);
                }
                _ => {}
            }
        }
    }
}
