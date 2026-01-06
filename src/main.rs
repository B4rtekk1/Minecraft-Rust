//! Mini Minecraft 3D Game
//!
//! Main entry point that delegates to the app module.

mod app;
mod multiplayer;
mod ui;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting Render3D application...");
    app::run_game();
}
