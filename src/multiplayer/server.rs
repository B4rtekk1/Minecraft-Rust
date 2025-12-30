use crate::multiplayer::protocol::{Packet, PlayerId};
use crate::multiplayer::quic::QuicServer;
use crate::multiplayer::tcp::TcpServer;
use crate::multiplayer::transport::TransportType;
use std::collections::HashMap;
use std::io::Result;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

/// Player information stored on the server
#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub id: PlayerId,
    pub username: String,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: u8,
    pub pitch: u8,
}

/// Events emitted by the server
#[derive(Debug, Clone)]
pub enum ServerEvent {
    PlayerConnected(PlayerId, String),
    PlayerDisconnected(PlayerId),
    PlayerMoved(PlayerId, f32, f32, f32),
    PlayerRotated(PlayerId, u8, u8),
    BlockChanged(i32, i32, i32, u8),
    ChatMessage(PlayerId, String),
}

/// Game server that manages player connections
pub struct GameServer {
    transport_type: TransportType,
    players: Arc<RwLock<HashMap<PlayerId, PlayerInfo>>>,
    event_tx: mpsc::UnboundedSender<ServerEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<ServerEvent>>,
}

impl GameServer {
    pub fn new(transport_type: TransportType) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            transport_type,
            players: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    /// Take the event receiver (can only be called once)
    pub fn take_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<ServerEvent>> {
        self.event_rx.take()
    }

    /// Get transport type
    pub fn transport_type(&self) -> TransportType {
        self.transport_type
    }

    /// Handle an incoming packet from a player
    pub async fn handle_packet(&self, player_id: PlayerId, packet: Packet) -> Result<()> {
        match packet {
            Packet::Connect { username, .. } => {
                let player = PlayerInfo {
                    id: player_id,
                    username: username.clone(),
                    x: 0.0,
                    y: 64.0,
                    z: 0.0,
                    yaw: 0,
                    pitch: 128,
                };

                {
                    let mut players = self.players.write().await;
                    players.insert(player_id, player);
                }

                let _ = self
                    .event_tx
                    .send(ServerEvent::PlayerConnected(player_id, username));
            }

            Packet::Position { x, y, z, .. } => {
                {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(&player_id) {
                        player.x = x;
                        player.y = y;
                        player.z = z;
                    }
                }

                let _ = self
                    .event_tx
                    .send(ServerEvent::PlayerMoved(player_id, x, y, z));
            }

            Packet::Rotation { yaw, pitch, .. } => {
                {
                    let mut players = self.players.write().await;
                    if let Some(player) = players.get_mut(&player_id) {
                        player.yaw = yaw;
                        player.pitch = pitch;
                    }
                }

                let _ = self
                    .event_tx
                    .send(ServerEvent::PlayerRotated(player_id, yaw, pitch));
            }

            Packet::BlockChange {
                x,
                y,
                z,
                block_type,
            } => {
                let _ = self
                    .event_tx
                    .send(ServerEvent::BlockChanged(x, y, z, block_type));
            }

            Packet::Chat { message, .. } => {
                let _ = self
                    .event_tx
                    .send(ServerEvent::ChatMessage(player_id, message));
            }

            Packet::Disconnect { .. } => {
                {
                    let mut players = self.players.write().await;
                    players.remove(&player_id);
                }

                let _ = self
                    .event_tx
                    .send(ServerEvent::PlayerDisconnected(player_id));
            }

            _ => {}
        }

        Ok(())
    }

    /// Remove a player from the server
    pub async fn remove_player(&self, player_id: PlayerId) {
        let mut players = self.players.write().await;
        players.remove(&player_id);
        let _ = self
            .event_tx
            .send(ServerEvent::PlayerDisconnected(player_id));
    }

    /// Get all connected players
    pub async fn get_players(&self) -> Vec<PlayerInfo> {
        let players = self.players.read().await;
        players.values().cloned().collect()
    }

    /// Get player count
    pub async fn player_count(&self) -> usize {
        self.players.read().await.len()
    }
}

/// Configuration for starting a server
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub address: String,
    pub port: u16,
    pub transport: TransportType,
    pub max_players: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            address: "0.0.0.0".to_string(),
            port: 25565,
            transport: TransportType::Tcp,
            max_players: 100,
        }
    }
}

impl ServerConfig {
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}
