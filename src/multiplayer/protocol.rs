use std::io::{Cursor, Error, ErrorKind, Read, Result, Write};

pub type PlayerId = u32;

#[derive(Debug, Clone)]
pub enum Packet {
    Connect {
        player_id: PlayerId,
        username: String,
    },
    ConnectAck {
        success: bool,
        player_id: PlayerId,
    },
    Position {
        player_id: PlayerId,
        x: f32,
        y: f32,
        z: f32,
    },
    Rotation {
        player_id: PlayerId,
        yaw: u8,   // 0-255 = 0-360 degrees
        pitch: u8, // 0-255 = -90 to +90 degrees
    },
    BlockChange {
        x: i32,
        y: i32,
        z: i32,
        block_type: u8,
    },
    Chat {
        player_id: PlayerId,
        message: String,
    },
    Disconnect {
        player_id: PlayerId,
    },
    Ping {
        timestamp: u64,
    },
    Pong {
        timestamp: u64,
    },
}

impl Packet {
    fn packet_id(&self) -> u8 {
        match self {
            Packet::Connect { .. } => 0x01,
            Packet::ConnectAck { .. } => 0x02,
            Packet::Position { .. } => 0x10,
            Packet::Rotation { .. } => 0x11,
            Packet::BlockChange { .. } => 0x20,
            Packet::Chat { .. } => 0x30,
            Packet::Disconnect { .. } => 0x40,
            Packet::Ping { .. } => 0xFE,
            Packet::Pong { .. } => 0xFF,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        buf.push(self.packet_id());

        match self {
            Packet::Connect {
                player_id,
                username,
            } => {
                buf.extend_from_slice(&player_id.to_le_bytes());
                write_string(&mut buf, username);
            }
            Packet::ConnectAck { success, player_id } => {
                buf.push(if *success { 1 } else { 0 });
                buf.extend_from_slice(&player_id.to_le_bytes());
            }
            Packet::Position { player_id, x, y, z } => {
                buf.extend_from_slice(&player_id.to_le_bytes());
                buf.extend_from_slice(&x.to_le_bytes());
                buf.extend_from_slice(&y.to_le_bytes());
                buf.extend_from_slice(&z.to_le_bytes());
            }
            Packet::Rotation {
                player_id,
                yaw,
                pitch,
            } => {
                buf.extend_from_slice(&player_id.to_le_bytes());
                buf.push(*yaw);
                buf.push(*pitch);
            }
            Packet::BlockChange {
                x,
                y,
                z,
                block_type,
            } => {
                buf.extend_from_slice(&x.to_le_bytes());
                buf.extend_from_slice(&y.to_le_bytes());
                buf.extend_from_slice(&z.to_le_bytes());
                buf.push(*block_type);
            }
            Packet::Chat { player_id, message } => {
                buf.extend_from_slice(&player_id.to_le_bytes());
                write_string(&mut buf, message);
            }
            Packet::Disconnect { player_id } => {
                buf.extend_from_slice(&player_id.to_le_bytes());
            }
            Packet::Ping { timestamp } | Packet::Pong { timestamp } => {
                buf.extend_from_slice(&timestamp.to_le_bytes());
            }
        }

        let len = buf.len() as u16;
        let mut result = Vec::with_capacity(2 + buf.len());
        result.extend_from_slice(&len.to_le_bytes());
        result.extend(buf);
        result
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 3 {
            return Err(Error::new(ErrorKind::InvalidData, "Packet too short"));
        }

        let mut cursor = Cursor::new(data);

        let mut len_bytes = [0u8; 2];
        cursor.read_exact(&mut len_bytes)?;
        let _len = u16::from_le_bytes(len_bytes);
        let mut id = [0u8; 1];
        cursor.read_exact(&mut id)?;

        match id[0] {
            0x01 => {
                let player_id = read_u32(&mut cursor)?;
                let username = read_string(&mut cursor)?;
                Ok(Packet::Connect {
                    player_id,
                    username,
                })
            }
            0x02 => {
                let mut b = [0u8; 1];
                cursor.read_exact(&mut b)?;
                let player_id = read_u32(&mut cursor)?;
                Ok(Packet::ConnectAck {
                    success: b[0] != 0,
                    player_id,
                })
            }
            0x10 => {
                let player_id = read_u32(&mut cursor)?;
                let x = read_f32(&mut cursor)?;
                let y = read_f32(&mut cursor)?;
                let z = read_f32(&mut cursor)?;
                Ok(Packet::Position { player_id, x, y, z })
            }
            0x11 => {
                let player_id = read_u32(&mut cursor)?;
                let mut angles = [0u8; 2];
                cursor.read_exact(&mut angles)?;
                Ok(Packet::Rotation {
                    player_id,
                    yaw: angles[0],
                    pitch: angles[1],
                })
            }
            0x20 => {
                let x = read_i32(&mut cursor)?;
                let y = read_i32(&mut cursor)?;
                let z = read_i32(&mut cursor)?;
                let mut bt = [0u8; 1];
                cursor.read_exact(&mut bt)?;
                Ok(Packet::BlockChange {
                    x,
                    y,
                    z,
                    block_type: bt[0],
                })
            }
            0x30 => {
                let player_id = read_u32(&mut cursor)?;
                let message = read_string(&mut cursor)?;
                Ok(Packet::Chat { player_id, message })
            }
            0x40 => {
                let player_id = read_u32(&mut cursor)?;
                Ok(Packet::Disconnect { player_id })
            }
            0xFE => {
                let timestamp = read_u64(&mut cursor)?;
                Ok(Packet::Ping { timestamp })
            }
            0xFF => {
                let timestamp = read_u64(&mut cursor)?;
                Ok(Packet::Pong { timestamp })
            }
            _ => Err(Error::new(ErrorKind::InvalidData, "Unknown packet ID")),
        }
    }
}

fn write_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    buf.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
    buf.extend_from_slice(bytes);
}

fn read_string(cursor: &mut Cursor<&[u8]>) -> Result<String> {
    let mut len_bytes = [0u8; 2];
    cursor.read_exact(&mut len_bytes)?;
    let len = u16::from_le_bytes(len_bytes) as usize;

    let mut buf = vec![0u8; len];
    cursor.read_exact(&mut buf)?;

    String::from_utf8(buf).map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid UTF-8"))
}

fn read_u32(cursor: &mut Cursor<&[u8]>) -> Result<u32> {
    let mut bytes = [0u8; 4];
    cursor.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(cursor: &mut Cursor<&[u8]>) -> Result<u64> {
    let mut bytes = [0u8; 8];
    cursor.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

// Angle encoding helpers
pub fn encode_yaw(degrees: f32) -> u8 {
    (((degrees % 360.0) + 360.0) % 360.0 / 360.0 * 256.0) as u8
}

pub fn decode_yaw(val: u8) -> f32 {
    val as f32 / 256.0 * 360.0
}

pub fn encode_pitch(degrees: f32) -> u8 {
    ((degrees.clamp(-90.0, 90.0) + 90.0) / 180.0 * 255.0) as u8
}

pub fn decode_pitch(val: u8) -> f32 {
    val as f32 / 255.0 * 180.0 - 90.0
}

fn read_f32(cursor: &mut Cursor<&[u8]>) -> Result<f32> {
    let mut bytes = [0u8; 4];
    cursor.read_exact(&mut bytes)?;
    Ok(f32::from_le_bytes(bytes))
}

fn read_i32(cursor: &mut Cursor<&[u8]>) -> Result<i32> {
    let mut bytes = [0u8; 4];
    cursor.read_exact(&mut bytes)?;
    Ok(i32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_roundtrip() {
        let packet = Packet::Position {
            player_id: 12345,
            x: 1.5,
            y: 64.0,
            z: -3.25,
        };

        let bytes = packet.to_bytes();
        let decoded = Packet::from_bytes(&bytes).unwrap();

        if let Packet::Position { player_id, x, y, z } = decoded {
            assert_eq!(player_id, 12345);
            assert_eq!(x, 1.5);
            assert_eq!(y, 64.0);
            assert_eq!(z, -3.25);
        } else {
            panic!("Wrong packet type");
        }
    }
}
