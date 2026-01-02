//! Core data structures for the game
//! Contains fundamental types like blocks, biomes, chunks, and vertices.

pub mod biome;
pub mod block;
pub mod chunk;
pub mod uniforms;
pub mod vertex;

// Re-export commonly used types
pub use biome::Biome;
pub use block::BlockType;
pub use chunk::{Chunk, SubChunk};
pub use uniforms::Uniforms;
pub use vertex::Vertex;
