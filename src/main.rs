//! Mini Minecraft 3D Game
//!
//! Main entry point that delegates to the app module.

mod app;
mod multiplayer;
mod ui;

fn main() {
    app::run_game();
}
