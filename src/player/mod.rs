//! Player-related modules
//! Contains camera, input handling, and physics.

pub mod camera;
pub mod input;

// Re-export commonly used types
pub use camera::Camera;
pub use input::{DiggingState, InputState};
