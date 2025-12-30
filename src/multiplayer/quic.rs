//! Experimental QUIC transport implementation
//! This module is only available when the `quic-beta` feature is enabled.
//!
//! WARNING: This is experimental and may have breaking changes.

use crate::multiplayer::protocol::Packet;
use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::collections::HashMap;
use std::io::{Error, ErrorKind, Result};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio::sync::RwLock;

const READ_BUFFER_SIZE: usize = 4096;

/// Generate self-signed certificate for development/testing
fn generate_self_signed_cert() -> Result<(Vec<CertificateDer<'static>>, PrivatePkcs8KeyDer<'static>)>
{
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])
        .map_err(|e| Error::new(ErrorKind::Other, format!("Failed to generate cert: {}", e)))?;

    let key = PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der());
    let cert_der = CertificateDer::from(cert.cert);

    Ok((vec![cert_der], key))
}

/// QUIC connection wrapper
pub struct QuicConnection {
    connection: Connection,
    send_stream: Arc<RwLock<Option<SendStream>>>,
    recv_stream: Arc<RwLock<Option<RecvStream>>>,
    connected: AtomicBool,
}

impl QuicConnection {
    pub async fn new(connection: Connection) -> Result<Self> {
        // Open a bidirectional stream
        let (send, recv) = connection.open_bi().await.map_err(|e| {
            Error::new(
                ErrorKind::ConnectionRefused,
                format!("Failed to open stream: {}", e),
            )
        })?;

        Ok(Self {
            connection,
            send_stream: Arc::new(RwLock::new(Some(send))),
            recv_stream: Arc::new(RwLock::new(Some(recv))),
            connected: AtomicBool::new(true),
        })
    }

    pub async fn from_incoming(connection: Connection) -> Result<Self> {
        // Accept an incoming bidirectional stream
        let (send, recv) = connection.accept_bi().await.map_err(|e| {
            Error::new(
                ErrorKind::ConnectionRefused,
                format!("Failed to accept stream: {}", e),
            )
        })?;

        Ok(Self {
            connection,
            send_stream: Arc::new(RwLock::new(Some(send))),
            recv_stream: Arc::new(RwLock::new(Some(recv))),
            connected: AtomicBool::new(true),
        })
    }

    pub fn remote_address(&self) -> SocketAddr {
        self.connection.remote_address()
    }

    pub async fn send(&self, packet: &Packet) -> Result<()> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(Error::new(ErrorKind::NotConnected, "Connection closed"));
        }

        let bytes = packet.to_bytes();
        let mut stream_guard = self.send_stream.write().await;

        if let Some(ref mut stream) = *stream_guard {
            use tokio::io::AsyncWriteExt;
            stream.write_all(&bytes).await?;
            Ok(())
        } else {
            Err(Error::new(ErrorKind::NotConnected, "Send stream closed"))
        }
    }

    pub async fn recv(&self) -> Result<Packet> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(Error::new(ErrorKind::NotConnected, "Connection closed"));
        }

        let mut stream_guard = self.recv_stream.write().await;

        if let Some(ref mut stream) = *stream_guard {
            // Read packet length (2 bytes)
            let mut len_buf = [0u8; 2];
            stream
                .read_exact(&mut len_buf)
                .await
                .map_err(|e| Error::new(ErrorKind::UnexpectedEof, format!("Read error: {}", e)))?;
            let len = u16::from_le_bytes(len_buf) as usize;

            if len > READ_BUFFER_SIZE {
                return Err(Error::new(ErrorKind::InvalidData, "Packet too large"));
            }

            // Read packet data
            let mut data = vec![0u8; len + 2];
            data[0..2].copy_from_slice(&len_buf);
            stream
                .read_exact(&mut data[2..])
                .await
                .map_err(|e| Error::new(ErrorKind::UnexpectedEof, format!("Read error: {}", e)))?;

            Packet::from_bytes(&data)
        } else {
            Err(Error::new(ErrorKind::NotConnected, "Recv stream closed"))
        }
    }

    pub async fn close(&self) -> Result<()> {
        self.connected.store(false, Ordering::Relaxed);
        self.connection.close(0u32.into(), b"connection closed");
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

/// QUIC server using quinn
pub struct QuicServer {
    endpoint: Endpoint,
    connections: Arc<RwLock<HashMap<u32, Arc<QuicConnection>>>>,
    next_id: AtomicU32,
    running: AtomicBool,
}

impl QuicServer {
    pub async fn bind(addr: &str) -> Result<Self> {
        let socket_addr: SocketAddr = addr
            .parse()
            .map_err(|e| Error::new(ErrorKind::InvalidInput, format!("Invalid address: {}", e)))?;

        // Generate self-signed certificate
        let (certs, key) = generate_self_signed_cert()?;

        let server_config = ServerConfig::with_single_cert(certs, key.into())
            .map_err(|e| Error::new(ErrorKind::Other, format!("TLS config error: {}", e)))?;

        let endpoint = Endpoint::server(server_config, socket_addr)?;

        println!("[QUIC Server] Listening on {} (EXPERIMENTAL)", addr);

        Ok(Self {
            endpoint,
            connections: Arc::new(RwLock::new(HashMap::new())),
            next_id: AtomicU32::new(1),
            running: AtomicBool::new(true),
        })
    }

    /// Accept a new connection and return its ID
    pub async fn accept(&self) -> Result<(u32, Arc<QuicConnection>)> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| Error::new(ErrorKind::NotConnected, "Server endpoint closed"))?;

        let connection = incoming.await.map_err(|e| {
            Error::new(
                ErrorKind::ConnectionRefused,
                format!("Connection failed: {}", e),
            )
        })?;

        let addr = connection.remote_address();
        let quic_conn = Arc::new(QuicConnection::from_incoming(connection).await?);

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        {
            let mut conns = self.connections.write().await;
            conns.insert(id, quic_conn.clone());
        }

        println!("[QUIC Server] Client {} connected from {}", id, addr);
        Ok((id, quic_conn))
    }

    /// Broadcast a packet to all connected clients
    pub async fn broadcast(&self, packet: &Packet) -> Result<()> {
        let conns = self.connections.read().await;
        for (id, conn) in conns.iter() {
            if let Err(e) = conn.send(packet).await {
                eprintln!("[QUIC Server] Failed to send to client {}: {}", id, e);
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
                    eprintln!("[QUIC Server] Failed to send to client {}: {}", id, e);
                }
            }
        }
        Ok(())
    }

    /// Remove a disconnected client
    pub async fn remove_client(&self, id: u32) {
        let mut conns = self.connections.write().await;
        if conns.remove(&id).is_some() {
            println!("[QUIC Server] Client {} disconnected", id);
        }
    }

    /// Get the number of connected clients
    pub async fn client_count(&self) -> usize {
        self.connections.read().await.len()
    }

    /// Stop the server
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        self.endpoint.close(0u32.into(), b"server shutdown");
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }
}

/// QUIC client using quinn
pub struct QuicClient {
    endpoint: Option<Endpoint>,
    connection: Option<Arc<QuicConnection>>,
}

impl QuicClient {
    pub fn new() -> Self {
        Self {
            endpoint: None,
            connection: None,
        }
    }

    pub async fn connect(&mut self, addr: &str) -> Result<()> {
        println!("[QUIC Client] Connecting to {} (EXPERIMENTAL)...", addr);

        let socket_addr: SocketAddr = addr
            .parse()
            .map_err(|e| Error::new(ErrorKind::InvalidInput, format!("Invalid address: {}", e)))?;

        // Create client config that accepts self-signed certificates (for development)
        let crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        let client_config = ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(crypto)
                .map_err(|e| Error::new(ErrorKind::Other, format!("QUIC config error: {}", e)))?,
        ));

        let mut endpoint = Endpoint::client("0.0.0.0:0".parse().unwrap())?;
        endpoint.set_default_client_config(client_config);

        let connection = endpoint
            .connect(socket_addr, "localhost")
            .map_err(|e| {
                Error::new(
                    ErrorKind::ConnectionRefused,
                    format!("Connect error: {}", e),
                )
            })?
            .await
            .map_err(|e| {
                Error::new(
                    ErrorKind::ConnectionRefused,
                    format!("Connection failed: {}", e),
                )
            })?;

        let quic_conn = Arc::new(QuicConnection::new(connection).await?);

        self.endpoint = Some(endpoint);
        self.connection = Some(quic_conn);

        println!("[QUIC Client] Connected to {}", addr);
        Ok(())
    }

    pub fn connection(&self) -> Option<&Arc<QuicConnection>> {
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
        if let Some(endpoint) = self.endpoint.take() {
            endpoint.close(0u32.into(), b"client disconnect");
        }
        println!("[QUIC Client] Disconnected");
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connection
            .as_ref()
            .map(|c| c.is_connected())
            .unwrap_or(false)
    }
}

impl Default for QuicClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Skip server certificate verification (DEVELOPMENT ONLY!)
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> std::result::Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // WARNING: This skips certificate verification!
        // Only use this for development/testing with self-signed certificates.
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
