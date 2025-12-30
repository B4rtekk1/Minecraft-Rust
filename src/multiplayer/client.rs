use crate::multiplayer::protocol::{Packet, PlayerId};
use crate::multiplayer::quic::QuicClient;
use crate::multiplayer::tcp::TcpClient;
use crate::multiplayer::transport::TransportType;
use std::io::{Error, ErrorKind, Result};
use tokio::sync::mpsc;

/// Events received by the client from the server
#[derive(Debug, Clone)]
pub enum ClientEvent {
    Connected(PlayerId),
    Disconnected,
    PlayerJoined(PlayerId, String),
    PlayerLeft(PlayerId),
    PlayerMoved(PlayerId, f32, f32, f32),
    PlayerRotated(PlayerId, u8, u8),
    BlockChanged(i32, i32, i32, u8),
    ChatMessage(PlayerId, String),
    Pong(u64),
}

/// State of the client connection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

/// Game client for connecting to a server
pub struct GameClient {
    transport_type: TransportType,
    state: ConnectionState,
    player_id: Option<PlayerId>,
    tcp_client: Option<TcpClient>,
    quic_client: Option<QuicClient>,
    event_tx: mpsc::UnboundedSender<ClientEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<ClientEvent>>,
}

impl GameClient {
    pub fn new(transport_type: TransportType) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        Self {
            transport_type,
            state: ConnectionState::Disconnected,
            player_id: None,
            tcp_client: None,
            quic_client: None,
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    /// Take the event receiver (can only be called once)
    pub fn take_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<ClientEvent>> {
        self.event_rx.take()
    }

    /// Connect to a server
    pub async fn connect(&mut self, address: &str, username: &str) -> Result<()> {
        self.state = ConnectionState::Connecting;

        match self.transport_type {
            TransportType::Tcp => {
                let mut client = TcpClient::new();
                client.connect(address).await?;

                // Send connect packet
                let connect_packet = Packet::Connect {
                    player_id: 0, // Server will assign
                    username: username.to_string(),
                };
                client.send(&connect_packet).await?;

                // Wait for ConnectAck
                let response = client.recv().await?;
                match response {
                    Packet::ConnectAck { success, player_id } => {
                        if success {
                            self.tcp_client = Some(client);
                            self.state = ConnectionState::Connected;
                            self.player_id = Some(player_id);
                            let _ = self.event_tx.send(ClientEvent::Connected(player_id));
                            Ok(())
                        } else {
                            Err(Error::new(
                                ErrorKind::PermissionDenied,
                                "Connection rejected",
                            ))
                        }
                    }
                    _ => Err(Error::new(ErrorKind::InvalidData, "Unexpected response")),
                }
            }

            TransportType::Quic => {
                let mut client = QuicClient::new();
                client.connect(address).await?;

                // Send connect packet
                let connect_packet = Packet::Connect {
                    player_id: 0,
                    username: username.to_string(),
                };
                client.send(&connect_packet).await?;

                // Wait for ConnectAck
                let response = client.recv().await?;
                match response {
                    Packet::ConnectAck { success, player_id } => {
                        if success {
                            self.quic_client = Some(client);
                            self.state = ConnectionState::Connected;
                            self.player_id = Some(player_id);
                            let _ = self.event_tx.send(ClientEvent::Connected(player_id));
                            Ok(())
                        } else {
                            Err(Error::new(
                                ErrorKind::PermissionDenied,
                                "Connection rejected",
                            ))
                        }
                    }
                    _ => Err(Error::new(ErrorKind::InvalidData, "Unexpected response")),
                }
            }
        }
    }

    /// Send a packet to the server
    pub async fn send(&self, packet: &Packet) -> Result<()> {
        match self.transport_type {
            TransportType::Tcp => {
                if let Some(client) = &self.tcp_client {
                    client.send(packet).await
                } else {
                    Err(Error::new(ErrorKind::NotConnected, "Not connected"))
                }
            }

            TransportType::Quic => {
                if let Some(client) = &self.quic_client {
                    client.send(packet).await
                } else {
                    Err(Error::new(ErrorKind::NotConnected, "Not connected"))
                }
            }
        }
    }

    /// Receive a packet from the server
    pub async fn recv(&self) -> Result<Packet> {
        match self.transport_type {
            TransportType::Tcp => {
                if let Some(client) = &self.tcp_client {
                    client.recv().await
                } else {
                    Err(Error::new(ErrorKind::NotConnected, "Not connected"))
                }
            }

            TransportType::Quic => {
                if let Some(client) = &self.quic_client {
                    client.recv().await
                } else {
                    Err(Error::new(ErrorKind::NotConnected, "Not connected"))
                }
            }
        }
    }

    /// Handle an incoming packet and emit events
    pub fn handle_packet(&self, packet: Packet) {
        match packet {
            Packet::Position { player_id, x, y, z } => {
                let _ = self
                    .event_tx
                    .send(ClientEvent::PlayerMoved(player_id, x, y, z));
            }
            Packet::Rotation {
                player_id,
                yaw,
                pitch,
            } => {
                let _ = self
                    .event_tx
                    .send(ClientEvent::PlayerRotated(player_id, yaw, pitch));
            }
            Packet::BlockChange {
                x,
                y,
                z,
                block_type,
            } => {
                let _ = self
                    .event_tx
                    .send(ClientEvent::BlockChanged(x, y, z, block_type));
            }
            Packet::Chat { player_id, message } => {
                let _ = self
                    .event_tx
                    .send(ClientEvent::ChatMessage(player_id, message));
            }
            Packet::Connect {
                player_id,
                username,
            } => {
                let _ = self
                    .event_tx
                    .send(ClientEvent::PlayerJoined(player_id, username));
            }
            Packet::Disconnect { player_id } => {
                let _ = self.event_tx.send(ClientEvent::PlayerLeft(player_id));
            }
            Packet::Pong { timestamp } => {
                let _ = self.event_tx.send(ClientEvent::Pong(timestamp));
            }
            _ => {}
        }
    }

    /// Disconnect from the server
    pub async fn disconnect(&mut self) -> Result<()> {
        match self.transport_type {
            TransportType::Tcp => {
                if let Some(mut client) = self.tcp_client.take() {
                    // Send disconnect packet
                    if let Some(id) = self.player_id {
                        let _ = client.send(&Packet::Disconnect { player_id: id }).await;
                    }
                    client.disconnect().await?;
                }
            }

            TransportType::Quic => {
                if let Some(mut client) = self.quic_client.take() {
                    if let Some(id) = self.player_id {
                        let _ = client.send(&Packet::Disconnect { player_id: id }).await;
                    }
                    client.disconnect().await?;
                }
            }
        }

        self.state = ConnectionState::Disconnected;
        self.player_id = None;
        let _ = self.event_tx.send(ClientEvent::Disconnected);
        Ok(())
    }

    /// Get connection state
    pub fn state(&self) -> ConnectionState {
        self.state
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.state == ConnectionState::Connected
    }

    /// Get player ID
    pub fn player_id(&self) -> Option<PlayerId> {
        self.player_id
    }

    /// Get transport type
    pub fn transport_type(&self) -> TransportType {
        self.transport_type
    }

    /// Send position update
    pub async fn send_position(&self, x: f32, y: f32, z: f32) -> Result<()> {
        if let Some(id) = self.player_id {
            self.send(&Packet::Position {
                player_id: id,
                x,
                y,
                z,
            })
            .await
        } else {
            Err(Error::new(ErrorKind::NotConnected, "Not connected"))
        }
    }

    /// Send rotation update
    pub async fn send_rotation(&self, yaw: u8, pitch: u8) -> Result<()> {
        if let Some(id) = self.player_id {
            self.send(&Packet::Rotation {
                player_id: id,
                yaw,
                pitch,
            })
            .await
        } else {
            Err(Error::new(ErrorKind::NotConnected, "Not connected"))
        }
    }

    /// Send block change
    pub async fn send_block_change(&self, x: i32, y: i32, z: i32, block_type: u8) -> Result<()> {
        self.send(&Packet::BlockChange {
            x,
            y,
            z,
            block_type,
        })
        .await
    }

    /// Send chat message
    pub async fn send_chat(&self, message: &str) -> Result<()> {
        if let Some(id) = self.player_id {
            self.send(&Packet::Chat {
                player_id: id,
                message: message.to_string(),
            })
            .await
        } else {
            Err(Error::new(ErrorKind::NotConnected, "Not connected"))
        }
    }

    /// Send ping
    pub async fn send_ping(&self) -> Result<()> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.send(&Packet::Ping { timestamp }).await
    }
}

/// Configuration for client connection
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub server_address: String,
    pub server_port: u16,
    pub username: String,
    pub transport: TransportType,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_address: "127.0.0.1".to_string(),
            server_port: 25565,
            username: "Player".to_string(),
            transport: TransportType::Tcp,
        }
    }
}

impl ClientConfig {
    pub fn full_address(&self) -> String {
        format!("{}:{}", self.server_address, self.server_port)
    }
}
