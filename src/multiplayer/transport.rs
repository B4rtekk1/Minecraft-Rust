use crate::multiplayer::protocol::Packet;
use std::io::Result;

/// Common transport trait for TCP and QUIC backends
pub trait Transport: Send + Sync {
    /// Send a packet through the transport
    fn send(&self, packet: &Packet) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Receive a packet from the transport
    fn recv(&self) -> impl std::future::Future<Output = Result<Packet>> + Send;

    /// Close the transport connection
    fn close(&self) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Check if the connection is still alive
    fn is_connected(&self) -> bool;
}

/// Transport type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    /// TCP - stable, reliable, use for built-in server
    Tcp,
    /// QUIC - modern protocol, good for external large-scale servers
    Quic,
}

impl Default for TransportType {
    fn default() -> Self {
        TransportType::Tcp
    }
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportType::Tcp => write!(f, "TCP"),
            TransportType::Quic => write!(f, "QUIC"),
        }
    }
}
