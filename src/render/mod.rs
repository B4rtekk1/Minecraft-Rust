//! Rendering-related modules
//! Contains mesh building, frustum culling, texture generation, mesh loading, and indirect drawing.

pub mod frustum;
pub mod indirect;
pub mod mesh;
pub mod mesh_loader;
pub mod texture;

// Re-export commonly used types
pub use frustum::{AABB, extract_frustum_planes};
pub use indirect::{DrawIndexedIndirect, IndirectManager, SubchunkKey};
pub use mesh::{add_greedy_quad, add_quad, build_crosshair, build_player_model};
pub use mesh_loader::MeshLoader;
pub use texture::{generate_texture_atlas, load_texture_atlas_from_file};
