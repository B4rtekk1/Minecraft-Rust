//! Application module containing the game state and main loop
//!
//! This module re-exports the main game components from a single large file.
//! Future refactoring can split game.rs into smaller focused modules.

mod game;
mod texture_cache;
mod state;
mod init;
mod resize;
mod update;
mod render;
mod input;
mod server;

pub use texture_cache::{
    create_texture_atlas_optimized, generate_texture_atlas_with_mipmaps, load_or_generate_atlas,
};

pub use game::run_game;
