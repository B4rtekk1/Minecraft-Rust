//! Application module containing the game state and main loop
//!
//! This module re-exports the main game components from a single large file.
//! Future refactoring can split game.rs into smaller focused modules.

mod game;
mod cloud;

pub use game::run_game;
