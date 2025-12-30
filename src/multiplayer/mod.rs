pub mod client;
pub mod network;
pub mod player;
pub mod protocol;
pub mod quic;
pub mod server;
pub mod tcp;
pub mod transport;

// Re-exports for convenience
pub use client::{ClientConfig, ClientEvent, ConnectionState, GameClient};
pub use protocol::{Packet, PlayerId};
pub use quic::{QuicClient, QuicConnection, QuicServer};
pub use server::{GameServer, PlayerInfo, ServerConfig, ServerEvent};
pub use tcp::{TcpClient, TcpConnection, TcpServer};
pub use transport::TransportType;
