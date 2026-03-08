pub mod generator;
pub mod loader;
mod spline;
pub mod structures;
pub mod terrain;

// Re-export commonly used types
pub use generator::ChunkGenerator;
pub use loader::{ChunkGenResult, ChunkLoader};
pub use terrain::World;
