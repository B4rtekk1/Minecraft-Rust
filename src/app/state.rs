use std::sync::Arc;
use std::time::Instant;
use std::collections::HashMap;

use glyphon::{FontSystem, SwashCache, TextAtlas, TextRenderer, Viewport};
use wgpu;
use winit::window::Window;

use crate::multiplayer::player::RemotePlayer;
use crate::multiplayer::protocol::Packet;
use crate::ui::menu::{GameState, MenuState};
use render3d::{
    Camera, DiggingState, IndirectManager, InputState, World,
};
use render3d::chunk_loader::ChunkLoader;

pub struct State {
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub render_pipeline: wgpu::RenderPipeline,
    pub water_pipeline: wgpu::RenderPipeline,
    pub sun_pipeline: wgpu::RenderPipeline,
    pub sky_pipeline: wgpu::RenderPipeline,
    pub shadow_pipeline: wgpu::RenderPipeline,
    pub crosshair_pipeline: wgpu::RenderPipeline,
    pub sun_vertex_buffer: wgpu::Buffer,
    pub sun_index_buffer: wgpu::Buffer,
    pub crosshair_vertex_buffer: wgpu::Buffer,
    pub crosshair_index_buffer: wgpu::Buffer,
    pub num_crosshair_indices: u32,
    pub uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,
    pub shadow_bind_group: wgpu::BindGroup,
    pub depth_texture: wgpu::TextureView,
    pub msaa_texture_view: wgpu::TextureView,
    pub shadow_texture_view: wgpu::TextureView,
    pub shadow_cascade_views: Vec<wgpu::TextureView>,
    pub shadow_cascade_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    pub shadow_sampler: wgpu::Sampler,
    pub world: Arc<parking_lot::RwLock<World>>,
    pub camera: Camera,
    pub input: InputState,
    pub digging: DiggingState,
    pub window: Arc<Window>,
    pub frame_count: u32,
    pub last_fps_update: Instant,
    pub current_fps: f32,
    pub frame_time_ms: f32,
    pub cpu_update_ms: f32,
    pub last_redraw: Instant,
    pub last_frame: Instant,
    pub mouse_captured: bool,
    pub chunks_rendered: u32,
    pub subchunks_rendered: u32,
    pub game_start_time: Instant,
    pub coords_vertex_buffer: Option<wgpu::Buffer>,
    pub coords_index_buffer: Option<wgpu::Buffer>,
    pub coords_num_indices: u32,
    pub last_coords_position: (i32, i32, i32),
    pub progress_bar_vertex_buffer: Option<wgpu::Buffer>,
    pub progress_bar_index_buffer: Option<wgpu::Buffer>,
    #[allow(dead_code)]
    pub texture_atlas: wgpu::Texture,
    #[allow(dead_code)]
    pub texture_view: wgpu::TextureView,
    #[allow(dead_code)]
    pub texture_sampler: wgpu::Sampler,
    pub game_state: GameState,
    pub menu_state: MenuState,
    /// Reflection mode: 0=off, 1=SSR (default)
    pub reflection_mode: u32,
    /// Cached underwater state (1.0 = underwater, 0.0 = above water), updated each tick
    pub is_underwater: f32,
    // Multiplayer
    pub remote_players: HashMap<u32, RemotePlayer>,
    pub my_player_id: u32,
    pub last_position_send: Instant,
    pub network_runtime: Option<tokio::runtime::Runtime>,
    pub network_rx: Option<tokio::sync::mpsc::UnboundedReceiver<Packet>>,
    pub network_tx: Option<tokio::sync::mpsc::UnboundedSender<Packet>>,
    pub last_input_time: Instant,
    // Player model rendering
    pub player_model_vertex_buffer: Option<wgpu::Buffer>,
    pub player_model_index_buffer: Option<wgpu::Buffer>,
    pub player_model_num_indices: u32,
    // Async chunk loading
    pub chunk_loader: ChunkLoader,
    /// Cached player chunk coords — missing-chunk scan is skipped when unchanged
    pub last_gen_player_cx: i32,
    pub last_gen_player_cz: i32,
    // SSR (Screen Space Reflections) for water
    pub ssr_color_texture: wgpu::Texture,
    pub ssr_color_view: wgpu::TextureView,
    pub ssr_depth_texture: wgpu::Texture,
    pub ssr_depth_view: wgpu::TextureView,
    pub ssr_sampler: wgpu::Sampler,
    pub water_bind_group: wgpu::BindGroup,
    pub water_bind_group_layout: wgpu::BindGroupLayout,
    pub surface_format: wgpu::TextureFormat,
    // Glyphon text rendering
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub text_atlas: TextAtlas,
    pub text_renderer: TextRenderer,
    pub viewport: Viewport,
    pub fps_buffer: glyphon::Buffer,
    pub menu_buffer: glyphon::Buffer,
    pub player_label_buffers: Vec<glyphon::Buffer>,
    pub mesh_loader: render3d::MeshLoader,
    pub composite_pipeline: wgpu::RenderPipeline,
    pub composite_bind_group: wgpu::BindGroup,
    pub scene_color_texture: wgpu::Texture,
    pub scene_color_view: wgpu::TextureView,
    // GPU Indirect Drawing
    pub indirect_manager: IndirectManager,
    pub water_indirect_manager: IndirectManager,
    // Hi-Z Occlusion Culling
    pub hiz_texture: wgpu::Texture,
    pub hiz_view: wgpu::TextureView,
    pub hiz_mips: Vec<wgpu::TextureView>,
    pub hiz_pipeline: wgpu::ComputePipeline,
    pub hiz_bind_groups: Vec<wgpu::BindGroup>,
    pub hiz_bind_group_layout: wgpu::BindGroupLayout,
    /// Dimensions of the Hi-Z texture (matches screen size).
    pub hiz_size: [u32; 2],
    // Depth resolve for SSR
    pub depth_resolve_pipeline: wgpu::RenderPipeline,
    pub depth_resolve_bind_group: wgpu::BindGroup,
    /// Whether the device supports MULTI_DRAW_INDIRECT_COUNT feature
    pub supports_indirect_count: bool,
}

pub struct WorldSnapshot {
    pub missing_chunks: Vec<(i32, i32, i32)>,
    pub raycast_result: Option<(i32, i32, i32, i32, i32, i32)>,
    pub target_block: Option<render3d::BlockType>,
    pub eye_block: render3d::BlockType,
}

pub struct WorldWriteOps {
    pub completed_chunks: Vec<(i32, i32, render3d::Chunk)>,
    pub block_break: Option<(i32, i32, i32)>,
    pub mark_dirty: Vec<(i32, i32, i32)>,
}

