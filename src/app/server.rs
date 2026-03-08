use std::sync::Arc;

use crate::multiplayer::protocol::Packet;
use crate::multiplayer::tcp::TcpServer;

pub async fn run_dedicated_server(addr: &str) {
    match TcpServer::bind(addr).await {
        Ok(server_inst) => {
            let server = Arc::new(server_inst);
            println!("Server is now listening on {}", addr);
            println!("Waiting for connections...");
            std::io::Write::flush(&mut std::io::stdout()).unwrap();

            loop {
                match server.accept().await {
                    Ok((id, conn)) => {
                        println!("Client {} connected from {}", id, conn.addr());
                        let server_clone = server.clone();

                        tokio::spawn(async move {
                            loop {
                                match conn.recv().await {
                                    Ok(mut packet) => {
                                        match packet {
                                            Packet::Connect {
                                                ref mut player_id, ..
                                            } => {
                                                *player_id = id;
                                                let ack = Packet::ConnectAck {
                                                    success: true,
                                                    player_id: id,
                                                };
                                                let _ = conn.send(&ack).await;
                                            }
                                            Packet::Position {
                                                ref mut player_id, ..
                                            } => {
                                                *player_id = id;
                                            }
                                            Packet::Rotation {
                                                ref mut player_id, ..
                                            } => {
                                                *player_id = id;
                                            }
                                            Packet::Chat {
                                                ref mut player_id, ..
                                            } => {
                                                *player_id = id;
                                            }
                                            Packet::Disconnect {
                                                ref mut player_id, ..
                                            } => {
                                                *player_id = id;
                                            }
                                            _ => {}
                                        }

                                        let _ = server_clone.broadcast_except(&packet, id).await;
                                    }
                                    Err(_) => {
                                        println!("Client {} disconnected", id);
                                        let disconnect_packet =
                                            Packet::Disconnect { player_id: id };
                                        let _ = server_clone
                                            .broadcast_except(&disconnect_packet, id)
                                            .await;
                                        server_clone.remove_client(id).await;
                                        break;
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("Accept error: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to start server: {}", e);
        }
    }
}
