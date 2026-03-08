#![allow(dead_code)]

use crate::multiplayer::protocol::Packet;
use std::io::Result;

pub trait Transport: Send + Sync {
    fn send(&self, packet: &Packet) -> impl std::future::Future<Output = Result<()>> + Send;

    fn recv(&self) -> impl std::future::Future<Output = Result<Packet>> + Send;

    fn close(&self) -> impl std::future::Future<Output = Result<()>> + Send;

    fn is_connected(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportType {
    Tcp,
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
        }
    }
}
