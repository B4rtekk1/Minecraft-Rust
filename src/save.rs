use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use crate::block::BlockType;
use crate::constants::*;

const MAGIC_HEADER: &[u8; 4] = b"R3DW";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct SavedBlock {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_type: BlockType,
}

#[derive(Serialize, Deserialize)]
pub struct SavedChunk {
    pub cx: i32,
    pub cz: i32,
    pub blocks: Vec<SavedBlock>,
}

#[derive(Serialize, Deserialize)]
pub struct SavedWorld {
    pub seed: u32,
    pub player_x: f32,
    pub player_y: f32,
    pub player_z: f32,
    pub player_yaw: f32,
    pub player_pitch: f32,
    pub chunks: Vec<SavedChunk>,
}

impl SavedWorld {
    pub fn from_world(
        chunks: &HashMap<(i32, i32), crate::chunk::Chunk>,
        seed: u32,
        player_pos: (f32, f32, f32),
        player_rot: (f32, f32),
    ) -> Self {
        let mut saved_chunks = Vec::new();

        for (&(cx, cz), chunk) in chunks.iter() {
            if !chunk.player_modified {
                continue;
            }

            let mut blocks = Vec::new();
            let base_x = cx * CHUNK_SIZE;
            let base_z = cz * CHUNK_SIZE;

            for (sy, subchunk) in chunk.subchunks.iter().enumerate() {
                let base_y = sy as i32 * SUBCHUNK_HEIGHT;

                for lx in 0..CHUNK_SIZE as usize {
                    for ly in 0..SUBCHUNK_HEIGHT as usize {
                        for lz in 0..CHUNK_SIZE as usize {
                            let block = subchunk.blocks[lx][ly][lz];
                            blocks.push(SavedBlock {
                                x: base_x + lx as i32,
                                y: base_y + ly as i32,
                                z: base_z + lz as i32,
                                block_type: block,
                            });
                        }
                    }
                }
            }

            saved_chunks.push(SavedChunk { cx, cz, blocks });
        }

        SavedWorld {
            seed,
            player_x: player_pos.0,
            player_y: player_pos.1,
            player_z: player_pos.2,
            player_yaw: player_rot.0,
            player_pitch: player_rot.1,
            chunks: saved_chunks,
        }
    }
}

pub fn save_world<P: AsRef<Path>>(path: P, world: &SavedWorld) -> Result<(), String> {
    let file = File::create(path).map_err(|e| format!("Nie można utworzyć pliku: {}", e))?;
    let mut writer = BufWriter::new(file);
    writer.write_all(MAGIC_HEADER).map_err(|e| e.to_string())?;
    writer
        .write_all(&VERSION.to_le_bytes())
        .map_err(|e| e.to_string())?;

    let data = bincode::serialize(world).map_err(|e| format!("Błąd serializacji: {}", e))?;

    let size = data.len() as u64;
    writer
        .write_all(&size.to_le_bytes())
        .map_err(|e| e.to_string())?;

    writer.write_all(&data).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;

    Ok(())
}

pub fn load_world<P: AsRef<Path>>(path: P) -> Result<SavedWorld, String> {
    let file = File::open(path).map_err(|e| format!("Nie można otworzyć pliku: {}", e))?;
    let mut reader = BufReader::new(file);
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != MAGIC_HEADER {
        return Err("Nieprawidłowy format pliku".to_string());
    }

    let mut version_bytes = [0u8; 4];
    reader
        .read_exact(&mut version_bytes)
        .map_err(|e| e.to_string())?;
    let version = u32::from_le_bytes(version_bytes);
    if version != VERSION {
        return Err(format!("Nieobsługiwana wersja pliku: {}", version));
    }

    let mut size_bytes = [0u8; 8];
    reader
        .read_exact(&mut size_bytes)
        .map_err(|e| e.to_string())?;
    let size = u64::from_le_bytes(size_bytes) as usize;

    let mut data = vec![0u8; size];
    reader.read_exact(&mut data).map_err(|e| e.to_string())?;

    bincode::deserialize(&data).map_err(|e| format!("Błąd deserializacji: {}", e))
}

pub const WORLD_FILE_EXTENSION: &str = "r3d";
pub const DEFAULT_WORLD_FILE: &str = "world.r3d";
