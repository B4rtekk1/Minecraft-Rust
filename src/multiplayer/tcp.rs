use crate::multiplayer::protocol::Packet;
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc};

const READ_BUFFER_SIZE: usize = 4096;

/// TCP connection wrapper implementing the transport layer
pub struct TcpConnection {
    writer: Arc<RwLock<tokio::net::tcp::OwnedWriteHalf>>,
    reader: Arc<RwLock<tokio::net::tcp::OwnedReadHalf>>,
    connected: AtomicBool,
    addr: SocketAddr,
}

impl TcpConnection {
    pub fn new(stream: TcpStream, addr: SocketAddr) -> Self {
        let (reader, writer) = stream.into_split();
        Self {
            writer: Arc::new(RwLock::new(writer)),
            reader: Arc::new(RwLock::new(reader)),
            connected: AtomicBool::new(true),
            addr,
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub async fn send(&self, packet: &Packet) -> Result<()> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(Error::new(ErrorKind::NotConnected, "Connection closed"));
        }

        let bytes = packet.to_bytes();
        let mut writer = self.writer.write().await;
        writer.write_all(&bytes).await?;
        writer.flush().await?;
        Ok(())
    }

    pub async fn recv(&self) -> Result<Packet> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(Error::new(ErrorKind::NotConnected, "Connection closed"));
        }

        let mut reader = self.reader.write().await;

        // Read packet length (2 bytes)
        let mut len_buf = [0u8; 2];
        reader.read_exact(&mut len_buf).await?;
        let len = u16::from_le_bytes(len_buf) as usize;

        if len > READ_BUFFER_SIZE {
            return Err(Error::new(ErrorKind::InvalidData, "Packet too large"));
        }

        // Read packet data
        let mut data = vec![0u8; len + 2];
        data[0..2].copy_from_slice(&len_buf);
        reader.read_exact(&mut data[2..]).await?;

        Packet::from_bytes(&data)
    }

    pub async fn close(&self) -> Result<()> {
        self.connected.store(false, Ordering::Relaxed);
        let mut writer = self.writer.write().await;
        writer.shutdown().await?;
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

/// TCP server that listens for incoming connections
pub struct TcpServer {
    listener: Option<TcpListener>,
    connections: Arc<RwLock<HashMap<u32, Arc<TcpConnection>>>>,
    next_id: AtomicU32,
    running: AtomicBool,
}

impl TcpServer {
    pub async fn bind(addr: &str) -> Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        println!("[TCP Server] Listening on {}", addr);

        Ok(Self {
            listener: Some(listener),
            connections: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU32::new(1),
            running: AtomicBool::new(true),
        })
    }

    /// Accept a new connection and return its ID
    pub async fn accept(&self) -> Result<(u32, Arc<TcpConnection>)> {
        let listener = self
            .listener
            .as_ref()
            .ok_or_else(|| Error::new(ErrorKind::NotConnected, "Server not running"))?;

        let (stream, addr) = listener.accept().await?;
        stream.set_nodelay(true)?;

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let connection = Arc::new(TcpConnection::new(stream, addr));

        {
            let mut conns = self.connections.write().await;
            conns.insert(id, connection.clone());
        }

        println!("[TCP Server] Client {} connected from {}", id, addr);
        Ok((id, connection))
    }

    /// Broadcast a packet to all connected clients
    pub async fn broadcast(&self, packet: &Packet) -> Result<()> {
        let conns = self.connections.read().await;
        for (id, conn) in conns.iter() {
            if let Err(e) = conn.send(packet).await {
                eprintln!("[TCP Server] Failed to send to client {}: {}", id, e);
            }
        }
        Ok(())
    }

    /// Broadcast a packet to all clients except one
    pub async fn broadcast_except(&self, packet: &Packet, except_id: u32) -> Result<()> {
        let conns = self.connections.read().await;
        for (id, conn) in conns.iter() {
            if *id != except_id {
                if let Err(e) = conn.send(packet).await {
                    eprintln!("[TCP Server] Failed to send to client {}: {}", id, e);
                }
            }
        }
        Ok(())
    }

    /// Remove a disconnected client
    pub async fn remove_client(&self, id: u32) {
        let mut conns = self.connections.write().await;
        if conns.remove(&id).is_some() {
            println!("[TCP Server] Client {} disconnected", id);
        }
    }

    /// Get the number of connected clients
    pub async fn client_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Stop the server
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

/// TCP client for connecting to a server
#[derive(Clone)]
pub struct TcpClient {
    connection: Option<Arc<TcpConnection>>,
}

impl TcpClient {
    pub fn new() -> Self {
        Self { connection: None }
    }

    pub async fn connect(&mut self, addr: &str) -> Result<()> {
        println!("[TCP Client] Connecting to {}...", addr);
        let stream = TcpStream::connect(addr).await?;
        stream.set_nodelay(true)?;

        let socket_addr = stream.peer_addr()?;
        self.connection = Some(Arc::new(TcpConnection::new(stream, socket_addr)));

        println!("[TCP Client] Connected to {}", addr);
        Ok(())
    }

    pub fn connection(&self) -> Option<&Arc<TcpConnection>> {
        self.connection.as_ref()
    }

    pub async fn send(&self, packet: &Packet) -> Result<()> {
        match &self.connection {
            Some(conn) => conn.send(packet).await,
            None => Err(Error::new(ErrorKind::NotConnected, "Not connected")),
        }
    }

    pub async fn recv(&self) -> Result<Packet> {
        match &self.connection {
            Some(conn) => conn.recv().await,
            None => Err(Error::new(ErrorKind::NotConnected, "Not connected")),
        }
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(conn) = self.connection.take() {
            conn.close().await?;
        }
        println!("[TCP Client] Disconnected");
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connection
            .as_ref()
            .map(|c| c.is_connected())
            .unwrap_or(false)
    }
}

impl Default for TcpClient {
    fn default() -> Self {
        Self::new()
    }
}
