use bytemuck::{Pod, Zeroable};
use cgmath::{Matrix4, Point3, Rad, Vector3, Vector4, prelude::*};
use image::GenericImageView;
use noise::{NoiseFn, Perlin};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use wgpu_glyph::{GlyphBrush, GlyphBrushBuilder, Section, Text, ab_glyph};
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceEvent, ElementState, Event, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowBuilder},
};

const WORLD_HEIGHT: i32 = 256;
const CHUNK_SIZE: i32 = 16;
const SUBCHUNK_HEIGHT: i32 = 16;
const NUM_SUBCHUNKS: i32 = WORLD_HEIGHT / SUBCHUNK_HEIGHT;
const RENDER_DISTANCE: i32 = 10;
const GENERATION_DISTANCE: i32 = 12;
const SEA_LEVEL: i32 = 64;
const CHUNK_UNLOAD_DISTANCE: i32 = 16;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    uv: [f32; 2],
    tex_index: f32,
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 9]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

// Row 0: grass_top, dirt, grass_side, stone
// Row 1: sand, water, wood_side, wood_top
// Row 2: leaves, bedrock, snow, gravel
// Row 3: clay, ice, cactus, dead_bush
const TEX_GRASS_TOP: f32 = 0.0;
const TEX_GRASS_SIDE: f32 = 1.0;
const TEX_DIRT: f32 = 2.0;
const TEX_STONE: f32 = 3.0;
const TEX_SAND: f32 = 4.0;
const TEX_WATER: f32 = 5.0;
const TEX_WOOD_SIDE: f32 = 6.0;
const TEX_WOOD_TOP: f32 = 7.0;
const TEX_LEAVES: f32 = 8.0;
const TEX_BEDROCK: f32 = 9.0;
const TEX_SNOW: f32 = 10.0;
const TEX_GRAVEL: f32 = 11.0;
const TEX_CLAY: f32 = 12.0;
const TEX_ICE: f32 = 13.0;
const TEX_CACTUS: f32 = 14.0;
const TEX_DEAD_BUSH: f32 = 15.0;

const TEXTURE_SIZE: u32 = 256;
const ATLAS_SIZE: u32 = 4;

fn load_texture_atlas_from_file<P: AsRef<Path>>(path: P) -> Result<(Vec<u8>, u32, u32), String> {
    let img = image::open(path).map_err(|e| format!("Failed to load texture: {}", e))?;
    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    if width % 4 != 0 || height % 4 != 0 {
        return Err(format!(
            "Texture atlas dimensions {}x{} not divisible by 4",
            width, height
        ));
    }

    let tile_w = width / 4;
    let tile_h = height / 4;

    if tile_w != tile_h {
        return Err(format!(
            "Texture atlas tiles are not square: {}x{}",
            tile_w, tile_h
        ));
    }

    let mut layers = Vec::with_capacity((width * height * 4) as usize);

    for i in 0..16 {
        let col = i % 4;
        let row = i / 4;
        let start_x = col * tile_w;
        let start_y = row * tile_h;

        for y in 0..tile_h {
            for x in 0..tile_w {
                let pixel = rgba.get_pixel(start_x + x, start_y + y);
                layers.extend_from_slice(&pixel.0);
            }
        }
    }

    Ok((layers, tile_w, tile_h))
}

fn generate_texture_atlas() -> Vec<u8> {
    let total_pixels = (TEXTURE_SIZE * TEXTURE_SIZE * ATLAS_SIZE * ATLAS_SIZE) as usize;
    let mut data = vec![0u8; total_pixels * 4];

    let set_pixel = |data: &mut [u8], tex_idx: u32, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8| {
        let layer_size = (TEXTURE_SIZE * TEXTURE_SIZE * 4) as usize;
        let layer_offset = (tex_idx as usize) * layer_size;
        let pixel_offset = ((y * TEXTURE_SIZE + x) * 4) as usize;
        let idx = layer_offset + pixel_offset;

        if idx + 3 < data.len() {
            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = a;
        }
    };

    let hash = |x: u32, y: u32, seed: u32| -> u8 {
        let n = x
            .wrapping_mul(374761393)
            .wrapping_add(y.wrapping_mul(668265263))
            .wrapping_add(seed);
        let n = (n ^ (n >> 13)).wrapping_mul(1274126177);
        ((n ^ (n >> 16)) & 0xFF) as u8
    };

    for tex_idx in 0..16u32 {
        for y in 0..TEXTURE_SIZE {
            for x in 0..TEXTURE_SIZE {
                let (r, g, b, a) = match tex_idx {
                    0 => {
                        // Grass top - green with variation
                        let noise = hash(x, y, 0) as i32 - 128;
                        let g_val = (100 + noise / 8).clamp(60, 140) as u8;
                        (50, g_val, 30, 255)
                    }
                    1 => {
                        // Grass side - dirt with green strip
                        if y < 3 {
                            let noise = hash(x, y, 1) as i32 - 128;
                            let g_val = (100 + noise / 8).clamp(60, 140) as u8;
                            (50, g_val, 30, 255)
                        } else {
                            let noise = hash(x, y, 2) as i32 - 128;
                            let base = 139 + noise / 10;
                            (
                                base.clamp(100, 160) as u8,
                                (base - 40).clamp(60, 120) as u8,
                                (base - 80).clamp(20, 60) as u8,
                                255,
                            )
                        }
                    }
                    2 => {
                        // Dirt
                        let noise = hash(x, y, 3) as i32 - 128;
                        let base = 139 + noise / 8;
                        (
                            base.clamp(100, 170) as u8,
                            (base - 40).clamp(60, 130) as u8,
                            (base - 80).clamp(20, 70) as u8,
                            255,
                        )
                    }
                    3 => {
                        // Stone
                        let noise = hash(x, y, 4) as i32 - 128;
                        let base = 128 + noise / 6;
                        let v = base.clamp(90, 160) as u8;
                        (v, v, v, 255)
                    }
                    4 => {
                        // Sand
                        let noise = hash(x, y, 5) as i32 - 128;
                        let base = 220 + noise / 12;
                        (
                            base.clamp(180, 240) as u8,
                            (base - 20).clamp(160, 220) as u8,
                            (base - 80).clamp(100, 160) as u8,
                            255,
                        )
                    }
                    5 => {
                        // Water
                        let noise = hash(x, y, 6) as i32 - 128;
                        let b_val = 180 + noise / 10;
                        (
                            30,
                            100 + (noise / 15) as u8,
                            b_val.clamp(150, 220) as u8,
                            200,
                        )
                    }
                    6 => {
                        // Wood side (bark)
                        let stripe = if x % 4 == 0 || x % 4 == 3 { 10i32 } else { 0 };
                        let noise = hash(x, y, 7) as i32 - 128;
                        let base = 100 + noise / 12 + stripe;
                        (
                            (base + 30).clamp(80, 150) as u8,
                            base.clamp(50, 120) as u8,
                            (base - 30).clamp(20, 70) as u8,
                            255,
                        )
                    }
                    7 => {
                        // Wood top (rings)
                        let cx = x as i32 - 8;
                        let cy = y as i32 - 8;
                        let dist = ((cx * cx + cy * cy) as f32).sqrt() as i32;
                        let ring = if dist % 3 == 0 { 20i32 } else { 0 };
                        let noise = hash(x, y, 8) as i32 - 128;
                        let base = 150 + noise / 15 - ring;
                        (
                            (base).clamp(100, 180) as u8,
                            (base - 40).clamp(60, 140) as u8,
                            (base - 80).clamp(20, 80) as u8,
                            255,
                        )
                    }
                    8 => {
                        // Leaves
                        let noise = hash(x, y, 9);
                        if noise > 180 {
                            (0, 0, 0, 0) // Transparent holes
                        } else {
                            let g_val = 80 + (noise / 4);
                            (30, g_val, 20, 240)
                        }
                    }
                    9 => {
                        // Bedrock
                        let noise = hash(x, y, 10) as i32 - 128;
                        let base = 50 + noise / 8;
                        let v = base.clamp(30, 80) as u8;
                        (v, v, v, 255)
                    }
                    10 => {
                        // Snow
                        let noise = hash(x, y, 11) as i32 - 128;
                        let base = 245 + noise / 20;
                        let v = base.clamp(230, 255) as u8;
                        (v, v, (v as i32 + 5).min(255) as u8, 255)
                    }
                    11 => {
                        // Gravel
                        let noise = hash(x, y, 12);
                        let pebble = if (noise / 40) % 3 == 0 { 30i32 } else { 0 };
                        let base = 120 + (noise as i32 / 10) - pebble;
                        let v = base.clamp(80, 150) as u8;
                        (v, v, v, 255)
                    }
                    12 => {
                        // Clay
                        let noise = hash(x, y, 13) as i32 - 128;
                        let base = 150 + noise / 12;
                        (
                            base.clamp(120, 170) as u8,
                            (base - 20).clamp(100, 150) as u8,
                            (base - 10).clamp(110, 160) as u8,
                            255,
                        )
                    }
                    13 => {
                        // Ice
                        let noise = hash(x, y, 14) as i32 - 128;
                        let base = 200 + noise / 15;
                        (
                            base.clamp(170, 230) as u8,
                            (base + 20).clamp(190, 250) as u8,
                            255,
                            220,
                        )
                    }
                    14 => {
                        // Cactus
                        let edge = x == 0 || x == 15 || y == 0 || y == 15;
                        let noise = hash(x, y, 15) as i32 - 128;
                        if edge {
                            (30, 80, 20, 255) // Darker edge
                        } else {
                            let g_val = 120 + noise / 10;
                            (40, g_val.clamp(100, 150) as u8, 30, 255)
                        }
                    }
                    15 => {
                        // Dead bush (mostly transparent)
                        let noise = hash(x, y, 16);
                        let is_branch = (x + y) % 5 == 0 && noise > 100;
                        if is_branch {
                            (100, 70, 40, 255)
                        } else {
                            (0, 0, 0, 0)
                        }
                    }
                    _ => (255, 0, 255, 255),
                };
                set_pixel(&mut data, tex_idx, x, y, r, g, b, a);
            }
        }
    }

    data
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
enum Biome {
    #[default]
    Plains,
    Forest,
    Desert,
    Tundra,
    Mountains,
    Swamp,
    Ocean,
    Beach,
    River,
    Lake,
    Island,
}

impl Biome {
    fn grass_color(&self) -> [f32; 3] {
        match self {
            Biome::Plains => [0.45, 0.75, 0.30],
            Biome::Forest => [0.25, 0.55, 0.20],
            Biome::Desert => [0.89, 0.83, 0.61],
            Biome::Tundra => [0.65, 0.75, 0.70],
            Biome::Mountains => [0.50, 0.60, 0.45],
            Biome::Swamp => [0.35, 0.50, 0.25],
            Biome::Ocean => [0.25, 0.46, 0.82],
            Biome::Beach => [0.89, 0.83, 0.61],
            Biome::River => [0.25, 0.46, 0.82],
            Biome::Lake => [0.25, 0.46, 0.82],
            Biome::Island => [0.40, 0.70, 0.30],
        }
    }

    fn leaves_color(&self) -> [f32; 3] {
        match self {
            Biome::Plains => [0.35, 0.65, 0.25],
            Biome::Forest => [0.20, 0.50, 0.15],
            Biome::Tundra => [0.30, 0.45, 0.35],
            Biome::Swamp => [0.30, 0.45, 0.20],
            Biome::Island => [0.35, 0.60, 0.25],
            _ => [0.30, 0.60, 0.20],
        }
    }

    fn tree_density(&self) -> f64 {
        match self {
            Biome::Plains => 0.75,
            Biome::Forest => 0.45,
            Biome::Desert => 1.0,
            Biome::Tundra => 0.85,
            Biome::Mountains => 0.80,
            Biome::Swamp => 0.60,
            Biome::Ocean => 1.0,
            Biome::Beach => 1.0,
            Biome::River => 1.0,
            Biome::Lake => 1.0,
            Biome::Island => 0.65,
        }
    }

    fn has_trees(&self) -> bool {
        matches!(
            self,
            Biome::Plains
                | Biome::Forest
                | Biome::Tundra
                | Biome::Mountains
                | Biome::Swamp
                | Biome::Island
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
enum BlockType {
    #[default]
    Air,
    Grass,
    Dirt,
    Stone,
    Sand,
    Water,
    Wood,
    Leaves,
    Bedrock,
    Snow,
    Gravel,
    Clay,
    Ice,
    Cactus,
    DeadBush,
}

impl BlockType {
    fn color(&self) -> [f32; 3] {
        match self {
            BlockType::Air => [0.0, 0.0, 0.0],
            BlockType::Grass => [0.45, 0.32, 0.22],
            BlockType::Dirt => [0.52, 0.37, 0.26],
            BlockType::Stone => [0.55, 0.55, 0.55],
            BlockType::Sand => [0.89, 0.83, 0.61],
            BlockType::Water => [0.25, 0.46, 0.82],
            BlockType::Wood => [0.6, 0.4, 0.2],
            BlockType::Leaves => [0.3, 0.6, 0.2],
            BlockType::Bedrock => [0.2, 0.2, 0.2],
            BlockType::Snow => [0.95, 0.95, 0.98],
            BlockType::Gravel => [0.5, 0.5, 0.52],
            BlockType::Clay => [0.65, 0.65, 0.72],
            BlockType::Ice => [0.7, 0.85, 0.95],
            BlockType::Cactus => [0.2, 0.55, 0.2],
            BlockType::DeadBush => [0.55, 0.4, 0.25],
        }
    }

    fn top_color(&self) -> [f32; 3] {
        match self {
            BlockType::Grass => [0.36, 0.7, 0.28],
            _ => self.color(),
        }
    }

    fn bottom_color(&self) -> [f32; 3] {
        match self {
            BlockType::Grass => [0.52, 0.37, 0.26],
            _ => self.color(),
        }
    }

    fn is_solid(&self) -> bool {
        !matches!(
            self,
            BlockType::Air | BlockType::Water | BlockType::DeadBush
        )
    }

    fn is_transparent(&self) -> bool {
        matches!(
            self,
            BlockType::Air
                | BlockType::Water
                | BlockType::Leaves
                | BlockType::Ice
                | BlockType::DeadBush
        )
    }

    fn should_render_face_against(&self, neighbor: BlockType) -> bool {
        if neighbor == BlockType::Air || neighbor == BlockType::Water {
            return true;
        }
        if *self == BlockType::Leaves && neighbor == BlockType::Leaves {
            return true;
        }
        neighbor.is_transparent()
    }

    fn break_time(&self) -> f32 {
        match self {
            BlockType::Air => 0.0,
            BlockType::Grass => 0.6,
            BlockType::Dirt => 0.5,
            BlockType::Stone => 1.5,
            BlockType::Sand => 0.5,
            BlockType::Water => 0.0,
            BlockType::Wood => 2.0,
            BlockType::Leaves => 0.2,
            BlockType::Bedrock => f32::INFINITY,
            BlockType::Snow => 0.2,
            BlockType::Gravel => 0.6,
            BlockType::Clay => 0.6,
            BlockType::Ice => 0.5,
            BlockType::Cactus => 0.4,
            BlockType::DeadBush => 0.0,
        }
    }

    fn tex_top(&self) -> f32 {
        match self {
            BlockType::Air => 0.0,
            BlockType::Grass => TEX_GRASS_TOP,
            BlockType::Dirt => TEX_DIRT,
            BlockType::Stone => TEX_STONE,
            BlockType::Sand => TEX_SAND,
            BlockType::Water => TEX_WATER,
            BlockType::Wood => TEX_WOOD_TOP,
            BlockType::Leaves => TEX_LEAVES,
            BlockType::Bedrock => TEX_BEDROCK,
            BlockType::Snow => TEX_SNOW,
            BlockType::Gravel => TEX_GRAVEL,
            BlockType::Clay => TEX_CLAY,
            BlockType::Ice => TEX_ICE,
            BlockType::Cactus => TEX_CACTUS,
            BlockType::DeadBush => TEX_DEAD_BUSH,
        }
    }

    fn tex_side(&self) -> f32 {
        match self {
            BlockType::Grass => TEX_GRASS_SIDE,
            BlockType::Wood => TEX_WOOD_SIDE,
            _ => self.tex_top(),
        }
    }

    fn tex_bottom(&self) -> f32 {
        match self {
            BlockType::Grass => TEX_DIRT,
            BlockType::Wood => TEX_WOOD_TOP,
            _ => self.tex_top(),
        }
    }
}

#[derive(Clone, Copy)]
struct AABB {
    min: Vector3<f32>,
    max: Vector3<f32>,
}

impl AABB {
    fn new(min: Vector3<f32>, max: Vector3<f32>) -> Self {
        AABB { min, max }
    }

    fn is_visible(&self, frustum_planes: &[Vector4<f32>; 6]) -> bool {
        let margin = 2.0;
        let expanded_min = Vector3::new(
            self.min.x - margin,
            self.min.y - margin,
            self.min.z - margin,
        );
        let expanded_max = Vector3::new(
            self.max.x + margin,
            self.max.y + margin,
            self.max.z + margin,
        );

        for plane in frustum_planes {
            let p = Vector3::new(
                if plane.x > 0.0 {
                    expanded_max.x
                } else {
                    expanded_min.x
                },
                if plane.y > 0.0 {
                    expanded_max.y
                } else {
                    expanded_min.y
                },
                if plane.z > 0.0 {
                    expanded_max.z
                } else {
                    expanded_min.z
                },
            );
            if plane.x * p.x + plane.y * p.y + plane.z * p.z + plane.w < 0.0 {
                return false;
            }
        }
        true
    }
}

struct SubChunk {
    blocks: [[[BlockType; CHUNK_SIZE as usize]; SUBCHUNK_HEIGHT as usize]; CHUNK_SIZE as usize],
    is_empty: bool,
    mesh_dirty: bool,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    num_indices: u32,
    water_vertex_buffer: Option<wgpu::Buffer>,
    water_index_buffer: Option<wgpu::Buffer>,
    num_water_indices: u32,
    aabb: AABB,
}

impl SubChunk {
    fn new(chunk_x: i32, subchunk_y: i32, chunk_z: i32) -> Self {
        let world_x = chunk_x * CHUNK_SIZE;
        let world_y = subchunk_y * SUBCHUNK_HEIGHT;
        let world_z = chunk_z * CHUNK_SIZE;

        SubChunk {
            blocks: [[[BlockType::Air; CHUNK_SIZE as usize]; SUBCHUNK_HEIGHT as usize];
                CHUNK_SIZE as usize],
            is_empty: true,
            mesh_dirty: true,
            vertex_buffer: None,
            index_buffer: None,
            num_indices: 0,
            water_vertex_buffer: None,
            water_index_buffer: None,
            num_water_indices: 0,
            aabb: AABB::new(
                Vector3::new(world_x as f32, world_y as f32, world_z as f32),
                Vector3::new(
                    (world_x + CHUNK_SIZE) as f32,
                    (world_y + SUBCHUNK_HEIGHT) as f32,
                    (world_z + CHUNK_SIZE) as f32,
                ),
            ),
        }
    }

    fn get_block(&self, x: i32, y: i32, z: i32) -> BlockType {
        if x >= 0 && x < CHUNK_SIZE && y >= 0 && y < SUBCHUNK_HEIGHT && z >= 0 && z < CHUNK_SIZE {
            self.blocks[x as usize][y as usize][z as usize]
        } else {
            BlockType::Air
        }
    }

    fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockType) {
        if x >= 0 && x < CHUNK_SIZE && y >= 0 && y < SUBCHUNK_HEIGHT && z >= 0 && z < CHUNK_SIZE {
            self.blocks[x as usize][y as usize][z as usize] = block;
            self.mesh_dirty = true;
            self.is_empty = block == BlockType::Air && self.is_empty;
        }
    }

    fn check_empty(&mut self) {
        self.is_empty = true;
        for x in 0..CHUNK_SIZE as usize {
            for y in 0..SUBCHUNK_HEIGHT as usize {
                for z in 0..CHUNK_SIZE as usize {
                    if self.blocks[x][y][z] != BlockType::Air {
                        self.is_empty = false;
                        return;
                    }
                }
            }
        }
    }
}

struct Chunk {
    subchunks: Vec<SubChunk>,
}

impl Chunk {
    fn new(x: i32, z: i32) -> Self {
        let mut subchunks = Vec::with_capacity(NUM_SUBCHUNKS as usize);
        for sy in 0..NUM_SUBCHUNKS {
            subchunks.push(SubChunk::new(x, sy, z));
        }
        Chunk { subchunks }
    }

    fn get_block(&self, x: i32, y: i32, z: i32) -> BlockType {
        if y < 0 || y >= WORLD_HEIGHT {
            return BlockType::Air;
        }
        let subchunk_idx = (y / SUBCHUNK_HEIGHT) as usize;
        let local_y = y % SUBCHUNK_HEIGHT;
        self.subchunks[subchunk_idx].get_block(x, local_y, z)
    }

    fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockType) {
        if y < 0 || y >= WORLD_HEIGHT {
            return;
        }
        let subchunk_idx = (y / SUBCHUNK_HEIGHT) as usize;
        let local_y = y % SUBCHUNK_HEIGHT;
        self.subchunks[subchunk_idx].set_block(x, local_y, z, block);
    }
}

struct World {
    chunks: HashMap<(i32, i32), Chunk>,
    perlin_continents: Perlin,
    perlin_terrain: Perlin,
    perlin_detail: Perlin,
    perlin_temperature: Perlin,
    perlin_moisture: Perlin,
    perlin_river: Perlin,
    perlin_lake: Perlin,
    perlin_trees: Perlin,
    perlin_island: Perlin,
    seed: u32,
}

impl World {
    fn new() -> Self {
        let seed = 42u32;
        let mut world = World {
            chunks: HashMap::new(),
            perlin_continents: Perlin::new(seed),
            perlin_terrain: Perlin::new(seed.wrapping_add(1)),
            perlin_detail: Perlin::new(seed.wrapping_add(2)),
            perlin_temperature: Perlin::new(seed.wrapping_add(3)),
            perlin_moisture: Perlin::new(seed.wrapping_add(4)),
            perlin_river: Perlin::new(seed.wrapping_add(5)),
            perlin_lake: Perlin::new(seed.wrapping_add(6)),
            perlin_trees: Perlin::new(seed.wrapping_add(7)),
            perlin_island: Perlin::new(seed.wrapping_add(8)),
            seed,
        };

        let spawn_cx = 0;
        let spawn_cz = 0;
        for cx in (spawn_cx - GENERATION_DISTANCE)..=(spawn_cx + GENERATION_DISTANCE) {
            for cz in (spawn_cz - GENERATION_DISTANCE)..=(spawn_cz + GENERATION_DISTANCE) {
                world.ensure_chunk_generated(cx, cz);
            }
        }

        world
    }

    fn ensure_chunk_generated(&mut self, cx: i32, cz: i32) {
        if self.chunks.contains_key(&(cx, cz)) {
            return;
        }
        self.generate_chunk(cx, cz);
    }

    fn update_chunks_around_player(&mut self, player_x: f32, player_z: f32) {
        let player_cx = (player_x / CHUNK_SIZE as f32).floor() as i32;
        let player_cz = (player_z / CHUNK_SIZE as f32).floor() as i32;

        for cx in (player_cx - GENERATION_DISTANCE)..=(player_cx + GENERATION_DISTANCE) {
            for cz in (player_cz - GENERATION_DISTANCE)..=(player_cz + GENERATION_DISTANCE) {
                self.ensure_chunk_generated(cx, cz);
            }
        }

        let chunks_to_remove: Vec<(i32, i32)> = self
            .chunks
            .keys()
            .filter(|(cx, cz)| {
                let dx = (*cx - player_cx).abs();
                let dz = (*cz - player_cz).abs();
                dx > CHUNK_UNLOAD_DISTANCE || dz > CHUNK_UNLOAD_DISTANCE
            })
            .cloned()
            .collect();

        for key in chunks_to_remove {
            self.chunks.remove(&key);
        }
    }

    fn get_biome(&self, x: i32, z: i32) -> Biome {
        let scale_continent = 0.002;
        let scale_temp = 0.008;
        let scale_moist = 0.01;
        let scale_river = 0.015;
        let scale_lake = 0.025;

        let continent = self
            .perlin_continents
            .get([x as f64 * scale_continent, z as f64 * scale_continent]);
        let river_noise = self
            .perlin_river
            .get([x as f64 * scale_river, z as f64 * scale_river]);
        let river_value = 1.0 - river_noise.abs() * 3.0;

        let lake_noise = self
            .perlin_lake
            .get([x as f64 * scale_lake, z as f64 * scale_lake]);

        if river_value > 0.85 && continent > -0.3 {
            return Biome::River;
        }

        if lake_noise < -0.6 && continent > -0.2 {
            return Biome::Lake;
        }

        if continent < -0.35 {
            let island_scale = 0.05;
            let island_noise = self
                .perlin_island
                .get([x as f64 * island_scale, z as f64 * island_scale]);
            if island_noise > 0.65 {
                return Biome::Island;
            }
            return Biome::Ocean;
        }

        if continent < -0.2 {
            return Biome::Beach;
        }

        let temp = self
            .perlin_temperature
            .get([x as f64 * scale_temp, z as f64 * scale_temp]);
        let moist = self
            .perlin_moisture
            .get([x as f64 * scale_moist, z as f64 * scale_moist]);

        if temp < -0.3 {
            Biome::Tundra
        } else if temp > 0.5 {
            if moist < -0.2 {
                Biome::Desert
            } else {
                Biome::Plains
            }
        } else {
            if moist > 0.3 {
                Biome::Swamp
            } else if moist > -0.2 {
                Biome::Forest
            } else {
                let mountain_noise = self
                    .perlin_terrain
                    .get([x as f64 * 0.005, z as f64 * 0.005]);
                if mountain_noise > 0.4 {
                    Biome::Mountains
                } else {
                    Biome::Plains
                }
            }
        }
    }

    fn get_terrain_height(&self, x: i32, z: i32) -> i32 {
        let biome = self.get_biome(x, z);

        let scale1 = 0.01;
        let scale2 = 0.04;
        let scale3 = 0.08;

        let noise1 = self
            .perlin_terrain
            .get([x as f64 * scale1, z as f64 * scale1]);
        let noise2 = self
            .perlin_detail
            .get([x as f64 * scale2, z as f64 * scale2]);
        let noise3 = self
            .perlin_detail
            .get([x as f64 * scale3, z as f64 * scale3]);

        let base_height = match biome {
            Biome::Ocean => {
                let depth = (noise1 + 1.0) * 0.5 * 15.0 + 35.0;
                depth as i32
            }
            Biome::River => SEA_LEVEL - 3,
            Biome::Lake => SEA_LEVEL - 5 + (noise2 * 3.0) as i32,
            Biome::Beach => SEA_LEVEL + 1 + (noise2 * 2.0) as i32,
            Biome::Island => {
                let island_height = self.perlin_island.get([x as f64 * 0.05, z as f64 * 0.05]);
                let height = SEA_LEVEL + ((island_height + 1.0) * 0.5 * 20.0) as i32;
                height.max(SEA_LEVEL - 5)
            }
            Biome::Plains => {
                let height = (noise1 + 1.0) * 0.5 * 15.0 + (noise2 + 1.0) * 0.5 * 5.0 + 65.0;
                height as i32
            }
            Biome::Forest => {
                let height = (noise1 + 1.0) * 0.5 * 20.0 + (noise2 + 1.0) * 0.5 * 8.0 + 65.0;
                height as i32
            }
            Biome::Desert => {
                let dune = (noise2 + 1.0) * 0.5 * 10.0;
                let height = (noise1 + 1.0) * 0.5 * 12.0 + dune + 64.0;
                height as i32
            }
            Biome::Tundra => {
                let height = (noise1 + 1.0) * 0.5 * 10.0 + (noise3 + 1.0) * 0.5 * 5.0 + 68.0;
                height as i32
            }
            Biome::Mountains => {
                let base = (noise1 + 1.0) * 0.5 * 80.0;
                let detail = (noise2 + 1.0) * 0.5 * 20.0;
                let height = base + detail + 70.0;
                height as i32
            }
            Biome::Swamp => {
                let height = (noise1 + 1.0) * 0.5 * 5.0 + (noise3 + 1.0) * 0.5 * 2.0 + 62.0;
                height as i32
            }
        };

        base_height.clamp(1, WORLD_HEIGHT - 20)
    }

    fn generate_chunk(&mut self, cx: i32, cz: i32) {
        let mut chunk = Chunk::new(cx, cz);
        let base_x = cx * CHUNK_SIZE;
        let base_z = cz * CHUNK_SIZE;

        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let world_x = base_x + lx;
                let world_z = base_z + lz;
                let biome = self.get_biome(world_x, world_z);
                let height = self.get_terrain_height(world_x, world_z);

                for y in 0..WORLD_HEIGHT.min(height + 10) {
                    let block = self.get_block_for_biome(biome, y, height);
                    if block != BlockType::Air {
                        chunk.set_block(lx, y, lz, block);
                    }
                }

                if height < SEA_LEVEL {
                    for y in height..SEA_LEVEL {
                        if biome == Biome::Tundra && y == SEA_LEVEL - 1 {
                            chunk.set_block(lx, y, lz, BlockType::Ice);
                        } else {
                            chunk.set_block(lx, y, lz, BlockType::Water);
                        }
                    }
                }
            }
        }

        self.generate_chunk_decorations(&mut chunk, cx, cz);

        for subchunk in &mut chunk.subchunks {
            subchunk.check_empty();
        }

        self.chunks.insert((cx, cz), chunk);
    }

    fn get_block_for_biome(&self, biome: Biome, y: i32, surface_height: i32) -> BlockType {
        if y == 0 {
            return BlockType::Bedrock;
        }

        let depth_from_surface = surface_height - y;

        match biome {
            Biome::Ocean | Biome::River | Biome::Lake => {
                if depth_from_surface > 3 {
                    BlockType::Stone
                } else if depth_from_surface > 0 {
                    BlockType::Gravel
                } else if y < surface_height {
                    BlockType::Sand
                } else {
                    BlockType::Air
                }
            }
            Biome::Beach | Biome::Island => {
                if depth_from_surface > 5 {
                    BlockType::Stone
                } else if depth_from_surface > 0 {
                    BlockType::Sand
                } else if y == surface_height - 1 {
                    if biome == Biome::Island {
                        BlockType::Grass
                    } else {
                        BlockType::Sand
                    }
                } else {
                    BlockType::Air
                }
            }
            Biome::Desert => {
                if depth_from_surface > 8 {
                    BlockType::Stone
                } else if depth_from_surface > 0 {
                    BlockType::Sand
                } else if y == surface_height - 1 {
                    BlockType::Sand
                } else {
                    BlockType::Air
                }
            }
            Biome::Tundra => {
                if depth_from_surface > 5 {
                    BlockType::Stone
                } else if depth_from_surface > 1 {
                    BlockType::Dirt
                } else if y == surface_height - 1 {
                    BlockType::Snow
                } else {
                    BlockType::Air
                }
            }
            Biome::Mountains => {
                if y > 120 {
                    if y == surface_height - 1 {
                        BlockType::Snow
                    } else if depth_from_surface > 0 {
                        BlockType::Stone
                    } else {
                        BlockType::Air
                    }
                } else {
                    if depth_from_surface > 3 {
                        BlockType::Stone
                    } else if depth_from_surface > 0 {
                        BlockType::Stone
                    } else if y == surface_height - 1 {
                        BlockType::Grass
                    } else {
                        BlockType::Air
                    }
                }
            }
            Biome::Swamp => {
                if depth_from_surface > 5 {
                    BlockType::Stone
                } else if depth_from_surface > 1 {
                    BlockType::Dirt
                } else if y == surface_height - 1 {
                    if y <= SEA_LEVEL {
                        BlockType::Clay
                    } else {
                        BlockType::Grass
                    }
                } else {
                    BlockType::Air
                }
            }
            Biome::Plains | Biome::Forest => {
                if depth_from_surface > 5 {
                    BlockType::Stone
                } else if depth_from_surface > 1 {
                    BlockType::Dirt
                } else if y == surface_height - 1 {
                    BlockType::Grass
                } else {
                    BlockType::Air
                }
            }
        }
    }

    fn generate_chunk_decorations(&self, chunk: &mut Chunk, cx: i32, cz: i32) {
        let base_x = cx * CHUNK_SIZE;
        let base_z = cz * CHUNK_SIZE;

        for lx in 2..(CHUNK_SIZE - 2) {
            for lz in 2..(CHUNK_SIZE - 2) {
                let world_x = base_x + lx;
                let world_z = base_z + lz;
                let biome = self.get_biome(world_x, world_z);
                let height = self.get_terrain_height(world_x, world_z);

                if height < SEA_LEVEL {
                    continue;
                }

                let tree_noise = self
                    .perlin_trees
                    .get([world_x as f64 * 0.3, world_z as f64 * 0.3]);

                if biome.has_trees() && tree_noise > biome.tree_density() {
                    self.place_tree_in_chunk(chunk, lx, height, lz, biome);
                }

                if biome == Biome::Desert {
                    let cactus_noise = self
                        .perlin_trees
                        .get([world_x as f64 * 0.5 + 100.0, world_z as f64 * 0.5]);
                    if cactus_noise > 0.8 {
                        self.place_cactus_in_chunk(chunk, lx, height, lz);
                    } else if cactus_noise > 0.7 {
                        chunk.set_block(lx, height, lz, BlockType::DeadBush);
                    }
                }
            }
        }
    }

    fn place_tree_in_chunk(&self, chunk: &mut Chunk, lx: i32, y: i32, lz: i32, biome: Biome) {
        let trunk_height = match biome {
            Biome::Forest => 6,
            Biome::Swamp => 7,
            Biome::Tundra => 4,
            _ => 5,
        };

        for ty in 0..trunk_height {
            chunk.set_block(lx, y + ty, lz, BlockType::Wood);
        }

        let crown_center_y = y + trunk_height;
        let crown_radius = if biome == Biome::Tundra {
            2.0_f32
        } else {
            2.5_f32
        };

        for dx in -3..=3 {
            for dy in -1..=3 {
                for dz in -3..=3 {
                    if dx == 0 && dz == 0 && dy < 0 {
                        continue;
                    }
                    let nlx = lx + dx;
                    let nly = crown_center_y + dy;
                    let nlz = lz + dz;

                    if nlx < 0 || nlx >= CHUNK_SIZE || nlz < 0 || nlz >= CHUNK_SIZE {
                        continue;
                    }

                    let dist = ((dx * dx + (dy - 1) * (dy - 1) + dz * dz) as f32).sqrt();
                    if dist <= crown_radius {
                        if chunk.get_block(nlx, nly, nlz) == BlockType::Air {
                            chunk.set_block(nlx, nly, nlz, BlockType::Leaves);
                        }
                    }
                }
            }
        }
    }

    fn place_cactus_in_chunk(&self, chunk: &mut Chunk, lx: i32, y: i32, lz: i32) {
        let height = 2 + ((self.seed as i32 + lx * 17 + lz * 31) % 2);
        for ty in 0..height {
            chunk.set_block(lx, y + ty, lz, BlockType::Cactus);
        }
    }

    fn get_block(&self, x: i32, y: i32, z: i32) -> BlockType {
        if y < 0 || y >= WORLD_HEIGHT {
            return BlockType::Air;
        }
        let cx = if x >= 0 {
            x / CHUNK_SIZE
        } else {
            (x - CHUNK_SIZE + 1) / CHUNK_SIZE
        };
        let cz = if z >= 0 {
            z / CHUNK_SIZE
        } else {
            (z - CHUNK_SIZE + 1) / CHUNK_SIZE
        };
        let lx = x.rem_euclid(CHUNK_SIZE);
        let lz = z.rem_euclid(CHUNK_SIZE);

        if let Some(chunk) = self.chunks.get(&(cx, cz)) {
            chunk.get_block(lx, y, lz)
        } else {
            BlockType::Air
        }
    }

    fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockType) {
        if y < 0 || y >= WORLD_HEIGHT {
            return;
        }
        let cx = if x >= 0 {
            x / CHUNK_SIZE
        } else {
            (x - CHUNK_SIZE + 1) / CHUNK_SIZE
        };
        let cz = if z >= 0 {
            z / CHUNK_SIZE
        } else {
            (z - CHUNK_SIZE + 1) / CHUNK_SIZE
        };
        let lx = x.rem_euclid(CHUNK_SIZE);
        let lz = z.rem_euclid(CHUNK_SIZE);

        if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
            chunk.set_block(lx, y, lz, block);
        }
    }

    fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.get_block(x, y, z).is_solid()
    }

    fn find_spawn_point(&self) -> (f32, f32, f32) {
        for radius in 0..50 {
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let x = dx;
                    let z = dz;
                    let height = self.get_terrain_height(x, z);
                    let biome = self.get_biome(x, z);

                    if height >= SEA_LEVEL
                        && !matches!(biome, Biome::Ocean | Biome::River | Biome::Lake)
                    {
                        return (x as f32 + 0.5, (height + 2) as f32, z as f32 + 0.5);
                    }
                }
            }
        }
        (0.5, 80.0, 0.5)
    }

    fn build_subchunk_mesh(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        subchunk_y: i32,
    ) -> ((Vec<Vertex>, Vec<u32>), (Vec<Vertex>, Vec<u32>)) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut water_vertices = Vec::new();
        let mut water_indices = Vec::new();

        let base_x = chunk_x * CHUNK_SIZE;
        let base_y = subchunk_y * SUBCHUNK_HEIGHT;
        let base_z = chunk_z * CHUNK_SIZE;

        for lx in 0..CHUNK_SIZE {
            for ly in 0..SUBCHUNK_HEIGHT {
                for lz in 0..CHUNK_SIZE {
                    let world_x = base_x + lx;
                    let world_y = base_y + ly;
                    let world_z = base_z + lz;

                    let block = self.get_block(world_x, world_y, world_z);
                    if block == BlockType::Air {
                        continue;
                    }

                    let is_water = block == BlockType::Water;
                    let (target_verts, target_inds) = if is_water {
                        (&mut water_vertices, &mut water_indices)
                    } else {
                        (&mut vertices, &mut indices)
                    };

                    let fx = world_x as f32;
                    let fy = world_y as f32;
                    let fz = world_z as f32;

                    let biome = self.get_biome(world_x, world_z);
                    let side_color = if block == BlockType::Grass {
                        block.color()
                    } else if block == BlockType::Leaves {
                        biome.leaves_color()
                    } else {
                        block.color()
                    };
                    let top_color = if block == BlockType::Grass {
                        biome.grass_color()
                    } else {
                        block.top_color()
                    };
                    let bottom_color = block.bottom_color();

                    let neighbor_top = self.get_block(world_x, world_y + 1, world_z);
                    if block.should_render_face_against(neighbor_top) {
                        add_quad(
                            target_verts,
                            target_inds,
                            [fx, fy + 1.0, fz],
                            [fx, fy + 1.0, fz + 1.0],
                            [fx + 1.0, fy + 1.0, fz + 1.0],
                            [fx + 1.0, fy + 1.0, fz],
                            [0.0, 1.0, 0.0],
                            top_color,
                            block.tex_top(),
                        );
                    }
                    let neighbor_bottom = self.get_block(world_x, world_y - 1, world_z);
                    if block.should_render_face_against(neighbor_bottom) {
                        add_quad(
                            target_verts,
                            target_inds,
                            [fx, fy, fz + 1.0],
                            [fx, fy, fz],
                            [fx + 1.0, fy, fz],
                            [fx + 1.0, fy, fz + 1.0],
                            [0.0, -1.0, 0.0],
                            bottom_color,
                            block.tex_bottom(),
                        );
                    }
                    let neighbor_front = self.get_block(world_x, world_y, world_z + 1);
                    if block.should_render_face_against(neighbor_front) {
                        add_quad(
                            target_verts,
                            target_inds,
                            [fx, fy, fz + 1.0],
                            [fx + 1.0, fy, fz + 1.0],
                            [fx + 1.0, fy + 1.0, fz + 1.0],
                            [fx, fy + 1.0, fz + 1.0],
                            [0.0, 0.0, 1.0],
                            side_color,
                            block.tex_side(),
                        );
                    }
                    let neighbor_back = self.get_block(world_x, world_y, world_z - 1);
                    if block.should_render_face_against(neighbor_back) {
                        add_quad(
                            target_verts,
                            target_inds,
                            [fx + 1.0, fy, fz],
                            [fx, fy, fz],
                            [fx, fy + 1.0, fz],
                            [fx + 1.0, fy + 1.0, fz],
                            [0.0, 0.0, -1.0],
                            side_color,
                            block.tex_side(),
                        );
                    }
                    let neighbor_right = self.get_block(world_x + 1, world_y, world_z);
                    if block.should_render_face_against(neighbor_right) {
                        add_quad(
                            target_verts,
                            target_inds,
                            [fx + 1.0, fy, fz + 1.0],
                            [fx + 1.0, fy, fz],
                            [fx + 1.0, fy + 1.0, fz],
                            [fx + 1.0, fy + 1.0, fz + 1.0],
                            [1.0, 0.0, 0.0],
                            side_color,
                            block.tex_side(),
                        );
                    }
                    let neighbor_left = self.get_block(world_x - 1, world_y, world_z);
                    if block.should_render_face_against(neighbor_left) {
                        add_quad(
                            target_verts,
                            target_inds,
                            [fx, fy, fz],
                            [fx, fy, fz + 1.0],
                            [fx, fy + 1.0, fz + 1.0],
                            [fx, fy + 1.0, fz],
                            [-1.0, 0.0, 0.0],
                            side_color,
                            block.tex_side(),
                        );
                    }
                }
            }
        }

        ((vertices, indices), (water_vertices, water_indices))
    }
}

fn add_quad(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    v0: [f32; 3],
    v1: [f32; 3],
    v2: [f32; 3],
    v3: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    tex_index: f32,
) {
    let base_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: v0,
        normal,
        color,
        uv: [0.0, 1.0],
        tex_index,
    });
    vertices.push(Vertex {
        position: v1,
        normal,
        color,
        uv: [1.0, 1.0],
        tex_index,
    });
    vertices.push(Vertex {
        position: v2,
        normal,
        color,
        uv: [1.0, 0.0],
        tex_index,
    });
    vertices.push(Vertex {
        position: v3,
        normal,
        color,
        uv: [0.0, 0.0],
        tex_index,
    });
    indices.extend_from_slice(&[
        base_idx,
        base_idx + 1,
        base_idx + 2,
        base_idx,
        base_idx + 2,
        base_idx + 3,
    ]);
}

fn extract_frustum_planes(view_proj: &Matrix4<f32>) -> [Vector4<f32>; 6] {
    let m = view_proj;
    [
        Vector4::new(
            m[0][3] + m[0][0],
            m[1][3] + m[1][0],
            m[2][3] + m[2][0],
            m[3][3] + m[3][0],
        ),
        Vector4::new(
            m[0][3] - m[0][0],
            m[1][3] - m[1][0],
            m[2][3] - m[2][0],
            m[3][3] - m[3][0],
        ),
        Vector4::new(
            m[0][3] + m[0][1],
            m[1][3] + m[1][1],
            m[2][3] + m[2][1],
            m[3][3] + m[3][1],
        ),
        Vector4::new(
            m[0][3] - m[0][1],
            m[1][3] - m[1][1],
            m[2][3] - m[2][1],
            m[3][3] - m[3][1],
        ),
        Vector4::new(
            m[0][3] + m[0][2],
            m[1][3] + m[1][2],
            m[2][3] + m[2][2],
            m[3][3] + m[3][2],
        ),
        Vector4::new(
            m[0][3] - m[0][2],
            m[1][3] - m[1][2],
            m[2][3] - m[2][2],
            m[3][3] - m[3][2],
        ),
    ]
}

struct Camera {
    position: Point3<f32>,
    yaw: f32,
    pitch: f32,
    velocity: Vector3<f32>,
    on_ground: bool,
}

impl Camera {
    fn new(spawn: (f32, f32, f32)) -> Self {
        Camera {
            position: Point3::new(spawn.0, spawn.1, spawn.2),
            yaw: 0.0,
            pitch: 0.0,
            velocity: Vector3::zero(),
            on_ground: false,
        }
    }

    fn forward(&self) -> Vector3<f32> {
        Vector3::new(self.yaw.cos(), 0.0, self.yaw.sin()).normalize()
    }

    fn right(&self) -> Vector3<f32> {
        Vector3::new(-self.yaw.sin(), 0.0, self.yaw.cos()).normalize()
    }

    fn look_direction(&self) -> Vector3<f32> {
        Vector3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    fn eye_position(&self) -> Point3<f32> {
        Point3::new(self.position.x, self.position.y + 1.62, self.position.z)
    }

    fn view_matrix(&self) -> Matrix4<f32> {
        let eye = self.eye_position();
        let target = eye + self.look_direction();
        Matrix4::look_at_rh(eye, target, Vector3::unit_y())
    }

    fn update(&mut self, world: &World, dt: f32, input: &InputState) {
        let speed = if input.sprint { 12.0 } else { 6.0 };
        let mut move_dir = Vector3::zero();

        if input.forward {
            move_dir += self.forward();
        }
        if input.backward {
            move_dir -= self.forward();
        }
        if input.left {
            move_dir -= self.right();
        }
        if input.right {
            move_dir += self.right();
        }

        if move_dir.magnitude2() > 0.0 {
            move_dir = move_dir.normalize() * speed;
        }

        self.velocity.x = move_dir.x;
        self.velocity.z = move_dir.z;

        if input.jump && self.on_ground {
            self.velocity.y = 8.0;
            self.on_ground = false;
        }

        self.velocity.y -= 25.0 * dt;
        self.velocity.y = self.velocity.y.max(-50.0);

        let new_pos = self.position + self.velocity * dt;

        if !self.check_collision(world, new_pos.x, self.position.y, self.position.z) {
            self.position.x = new_pos.x;
        } else {
            self.velocity.x = 0.0;
        }

        if !self.check_collision(world, self.position.x, self.position.y, new_pos.z) {
            self.position.z = new_pos.z;
        } else {
            self.velocity.z = 0.0;
        }

        if !self.check_collision(world, self.position.x, new_pos.y, self.position.z) {
            self.position.y = new_pos.y;
        } else {
            if self.velocity.y < 0.0 {
                self.on_ground = true;
            }
            self.velocity.y = 0.0;
        }

        self.position.y = self.position.y.max(1.0);
    }

    fn check_collision(&self, world: &World, x: f32, y: f32, z: f32) -> bool {
        let player_width = 0.35;
        let player_height = 1.8;

        let min_x = (x - player_width).floor() as i32;
        let max_x = (x + player_width).floor() as i32;
        let min_y = y.floor() as i32;
        let max_y = (y + player_height).floor() as i32;
        let min_z = (z - player_width).floor() as i32;
        let max_z = (z + player_width).floor() as i32;

        for bx in min_x..=max_x {
            for by in min_y..=max_y {
                for bz in min_z..=max_z {
                    if world.is_solid(bx, by, bz) {
                        let block_min_x = bx as f32;
                        let block_max_x = (bx + 1) as f32;
                        let block_min_y = by as f32;
                        let block_max_y = (by + 1) as f32;
                        let block_min_z = bz as f32;
                        let block_max_z = (bz + 1) as f32;

                        let player_min_x = x - player_width;
                        let player_max_x = x + player_width;
                        let player_min_y = y;
                        let player_max_y = y + player_height;
                        let player_min_z = z - player_width;
                        let player_max_z = z + player_width;

                        if player_max_x > block_min_x
                            && player_min_x < block_max_x
                            && player_max_y > block_min_y
                            && player_min_y < block_max_y
                            && player_max_z > block_min_z
                            && player_min_z < block_max_z
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn raycast(&self, world: &World, max_dist: f32) -> Option<(i32, i32, i32, i32, i32, i32)> {
        let dir = self.look_direction();
        let eye = self.eye_position();
        let mut pos = Vector3::new(eye.x, eye.y, eye.z);
        let step = 0.1;
        let mut prev = (
            pos.x.floor() as i32,
            pos.y.floor() as i32,
            pos.z.floor() as i32,
        );

        for _ in 0..(max_dist / step) as i32 {
            pos += dir * step;
            let current = (
                pos.x.floor() as i32,
                pos.y.floor() as i32,
                pos.z.floor() as i32,
            );
            if current != prev {
                if world.is_solid(current.0, current.1, current.2) {
                    return Some((current.0, current.1, current.2, prev.0, prev.1, prev.2));
                }
                prev = current;
            }
        }
        None
    }
}

#[derive(Default)]
struct InputState {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    jump: bool,
    sprint: bool,
    left_mouse: bool,
    right_mouse: bool,
}

#[derive(Default)]
struct DiggingState {
    target: Option<(i32, i32, i32)>,
    progress: f32,
    break_time: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    time: f32,
}

fn build_crosshair() -> (Vec<Vertex>, Vec<u32>) {
    let size = 0.015;
    let thickness = 0.003;
    let color = [1.0, 1.0, 1.0];
    let normal = [0.0, 0.0, 1.0];

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    vertices.push(Vertex {
        position: [-size, -thickness, 0.0],
        normal,
        color,
        uv: [0.0, 0.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [size, -thickness, 0.0],
        normal,
        color,
        uv: [1.0, 0.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [size, thickness, 0.0],
        normal,
        color,
        uv: [1.0, 1.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [-size, thickness, 0.0],
        normal,
        color,
        uv: [0.0, 1.0],
        tex_index: 0.0,
    });
    indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);

    vertices.push(Vertex {
        position: [-thickness, -size, 0.0],
        normal,
        color,
        uv: [0.0, 0.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [thickness, -size, 0.0],
        normal,
        color,
        uv: [1.0, 0.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [thickness, size, 0.0],
        normal,
        color,
        uv: [1.0, 1.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [-thickness, size, 0.0],
        normal,
        color,
        uv: [0.0, 1.0],
        tex_index: 0.0,
    });
    indices.extend_from_slice(&[4, 5, 6, 4, 6, 7]);

    (vertices, indices)
}
struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    water_pipeline: wgpu::RenderPipeline,
    crosshair_pipeline: wgpu::RenderPipeline,
    crosshair_vertex_buffer: wgpu::Buffer,
    crosshair_index_buffer: wgpu::Buffer,
    num_crosshair_indices: u32,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    depth_texture: wgpu::TextureView,
    world: World,
    camera: Camera,
    input: InputState,
    digging: DiggingState,
    window: Arc<Window>,
    frame_count: u32,
    last_fps_update: Instant,
    current_fps: f32,
    last_frame: Instant,
    mouse_captured: bool,
    chunks_rendered: u32,
    subchunks_rendered: u32,
    game_start_time: Instant,
    coords_vertex_buffer: Option<wgpu::Buffer>,
    coords_index_buffer: Option<wgpu::Buffer>,
    coords_num_indices: u32,
    texture_atlas: wgpu::Texture,
    texture_view: wgpu::TextureView,
    texture_sampler: wgpu::Sampler,
    glyph_brush: GlyphBrush<(), ab_glyph::FontArc>,
    staging_belt: wgpu::util::StagingBelt,
}

impl State {
    async fn new(window: Window) -> Self {
        let window = Arc::new(window);
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: Default::default(),
            })
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let depth_texture = Self::create_depth_texture(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[Uniforms {
                view_proj: Matrix4::identity().into(),
                camera_pos: [0.0, 0.0, 0.0],
                time: 0.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let (atlas_data, atlas_width, atlas_height) =
            match load_texture_atlas_from_file("assets/textures.png") {
                Ok((data, width, height)) => {
                    println!("Loaded texture atlas from PNG: {}x{}", width, height);
                    (data, width, height)
                }
                Err(e) => {
                    eprintln!("Failed to load assets/textures.png: {}", e);
                    match load_texture_atlas_from_file("assets/textures.jpg") {
                        Ok((data, width, height)) => {
                            println!("Loaded texture atlas from JPG: {}x{}", width, height);
                            (data, width, height)
                        }
                        Err(e) => {
                            eprintln!("Failed to load assets/textures.jpg: {}", e);
                            // Try loading from current directory (in case running from assets folder)
                            match load_texture_atlas_from_file("textures.png") {
                                Ok((data, width, height)) => {
                                    println!(
                                        "Loaded texture atlas from textures.png: {}x{}",
                                        width, height
                                    );
                                    (data, width, height)
                                }
                                Err(e) => {
                                    eprintln!("Failed to load textures.png: {}", e);
                                    match load_texture_atlas_from_file("textures.jpg") {
                                        Ok((data, width, height)) => {
                                            println!(
                                                "Loaded texture atlas from textures.jpg: {}x{}",
                                                width, height
                                            );
                                            (data, width, height)
                                        }
                                        Err(e) => {
                                            eprintln!("Failed to load textures.jpg: {}", e);
                                            println!("Using procedural texture atlas generation.");
                                            let data = generate_texture_atlas();
                                            (data, TEXTURE_SIZE, TEXTURE_SIZE)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };

        let texture_atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture Array"),
            size: wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 16,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture_atlas,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * atlas_width),
                rows_per_image: Some(atlas_height),
            },
            wgpu::Extent3d {
                width: atlas_width,
                height: atlas_height,
                depth_or_array_layers: 16,
            },
        );

        let texture_view = texture_atlas.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Texture Array View"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        let texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Texture Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("uniform_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture_sampler),
                },
            ],
            label: Some("uniform_bind_group"),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let water_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Water Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_water"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_water"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let crosshair_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Pipeline"),
            layout: Some(&pipeline_layout),
            cache: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_ui"),
                compilation_options: Default::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_ui"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        println!("Generating world...");
        let world = World::new();
        let spawn = world.find_spawn_point();
        let camera = Camera::new(spawn);
        println!("World generated! Spawn: {:?}", spawn);

        let (crosshair_vertices, crosshair_indices) = build_crosshair();
        let num_crosshair_indices = crosshair_indices.len() as u32;

        let crosshair_vertex_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Crosshair Vertex Buffer"),
                contents: bytemuck::cast_slice(&crosshair_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let crosshair_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Crosshair Index Buffer"),
            contents: bytemuck::cast_slice(&crosshair_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let staging_belt = wgpu::util::StagingBelt::new(1024);
        let font = ab_glyph::FontArc::try_from_slice(include_bytes!("c:/Windows/Fonts/arial.ttf"))
            .or_else(|_| {
                ab_glyph::FontArc::try_from_slice(include_bytes!("c:/Windows/Fonts/consola.ttf"))
            })
            .expect("Could not load font");
        let glyph_brush = GlyphBrushBuilder::using_font(font).build(&device, surface_format);

        Self {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            water_pipeline,
            crosshair_pipeline,
            crosshair_vertex_buffer,
            crosshair_index_buffer,
            num_crosshair_indices,
            uniform_buffer,
            uniform_bind_group,
            depth_texture,
            world,
            camera,
            input: InputState::default(),
            digging: DiggingState::default(),
            window,
            frame_count: 0,
            last_fps_update: Instant::now(),
            current_fps: 0.0,
            last_frame: Instant::now(),
            mouse_captured: false,
            chunks_rendered: 0,
            subchunks_rendered: 0,
            game_start_time: Instant::now(),
            coords_vertex_buffer: None,
            coords_index_buffer: None,
            coords_num_indices: 0,
            texture_atlas,
            texture_view,
            texture_sampler,
            glyph_brush,
            staging_belt,
        }
    }

    fn update_coords_ui(&mut self) {
        let x = self.camera.position.x;
        let y = self.camera.position.y;
        let z = self.camera.position.z;

        let text = format!("X:{:.0} Y:{:.0} Z:{:.0}", x, y, z);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let char_width = 0.018;
        let char_height = 0.032;
        let line_thickness = 0.004;
        let char_spacing = char_width * 0.6;
        let gap_spacing = char_width + 0.005;

        let mut total_width = 0.0;
        for ch in text.chars() {
            if ch == ' ' {
                total_width += char_spacing;
            } else {
                total_width += gap_spacing;
            }
        }

        let start_x = 0.98 - total_width;
        let start_y = 0.95;

        let mut cursor_x = start_x;
        let cursor_y = start_y;
        let color = [1.0, 1.0, 1.0];
        let normal = [0.0, 0.0, 1.0];

        let add_segment =
            |x1: f32, y1: f32, x2: f32, y2: f32, verts: &mut Vec<Vertex>, inds: &mut Vec<u32>| {
                let base_idx = verts.len() as u32;
                let dx = x2 - x1;
                let dy = y2 - y1;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 0.001 {
                    return;
                }
                let nx = -dy / len * line_thickness * 0.5;
                let ny = dx / len * line_thickness * 0.5;

                verts.push(Vertex {
                    position: [x1 - nx, y1 - ny, 0.0],
                    normal,
                    color,
                    uv: [0.0, 0.0],
                    tex_index: 0.0,
                });
                verts.push(Vertex {
                    position: [x2 - nx, y2 - ny, 0.0],
                    normal,
                    color,
                    uv: [1.0, 0.0],
                    tex_index: 0.0,
                });
                verts.push(Vertex {
                    position: [x2 + nx, y2 + ny, 0.0],
                    normal,
                    color,
                    uv: [1.0, 1.0],
                    tex_index: 0.0,
                });
                verts.push(Vertex {
                    position: [x1 + nx, y1 + ny, 0.0],
                    normal,
                    color,
                    uv: [0.0, 1.0],
                    tex_index: 0.0,
                });
                inds.extend_from_slice(&[
                    base_idx,
                    base_idx + 1,
                    base_idx + 2,
                    base_idx,
                    base_idx + 2,
                    base_idx + 3,
                ]);
            };

        fn get_char_segments(ch: char) -> Vec<(f32, f32, f32, f32)> {
            let seg_top = (0.0, 1.0, 1.0, 1.0);
            let seg_tr = (1.0, 1.0, 1.0, 0.5);
            let seg_br = (1.0, 0.5, 1.0, 0.0);
            let seg_bot = (0.0, 0.0, 1.0, 0.0);
            let seg_bl = (0.0, 0.5, 0.0, 0.0);
            let seg_tl = (0.0, 1.0, 0.0, 0.5);
            let seg_mid = (0.0, 0.5, 1.0, 0.5);

            match ch {
                '0' => vec![seg_top, seg_tr, seg_br, seg_bot, seg_bl, seg_tl],
                '1' => vec![seg_tr, seg_br],
                '2' => vec![seg_top, seg_tr, seg_mid, seg_bl, seg_bot],
                '3' => vec![seg_top, seg_tr, seg_mid, seg_br, seg_bot],
                '4' => vec![seg_tl, seg_mid, seg_tr, seg_br],
                '5' => vec![seg_top, seg_tl, seg_mid, seg_br, seg_bot],
                '6' => vec![seg_top, seg_tl, seg_mid, seg_br, seg_bot, seg_bl],
                '7' => vec![seg_top, seg_tr, seg_br],
                '8' => vec![seg_top, seg_tr, seg_br, seg_bot, seg_bl, seg_tl, seg_mid],
                '9' => vec![seg_top, seg_tr, seg_br, seg_bot, seg_tl, seg_mid],
                'X' => vec![(0.0, 1.0, 1.0, 0.0), (0.0, 0.0, 1.0, 1.0)],
                'Y' => vec![
                    (0.0, 1.0, 0.5, 0.5),
                    (1.0, 1.0, 0.5, 0.5),
                    (0.5, 0.5, 0.5, 0.0),
                ],
                'Z' => vec![seg_top, (1.0, 1.0, 0.0, 0.0), seg_bot],
                ':' => vec![(0.4, 0.7, 0.6, 0.7), (0.4, 0.3, 0.6, 0.3)],
                '.' => vec![(0.4, 0.1, 0.6, 0.1)],
                '-' => vec![seg_mid],
                _ => vec![],
            }
        }

        for ch in text.chars() {
            if ch == ' ' {
                cursor_x += char_spacing;
                continue;
            }

            let segments = get_char_segments(ch);
            for (x1, y1, x2, y2) in segments {
                let px1 = cursor_x + x1 * char_width;
                let py1 = cursor_y - char_height + y1 * char_height;
                let px2 = cursor_x + x2 * char_width;
                let py2 = cursor_y - char_height + y2 * char_height;
                add_segment(px1, py1, px2, py2, &mut vertices, &mut indices);
            }

            cursor_x += gap_spacing;
        }

        if vertices.is_empty() {
            self.coords_vertex_buffer = None;
            self.coords_index_buffer = None;
            self.coords_num_indices = 0;
            return;
        }

        let vb = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Coords Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let ib = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Coords Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        self.coords_vertex_buffer = Some(vb);
        self.coords_index_buffer = Some(ib);
        self.coords_num_indices = indices.len() as u32;
    }

    fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::TextureView {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        depth_texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = Self::create_depth_texture(&self.device, &self.config);
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        self.camera.update(&self.world, dt, &self.input);

        self.world
            .update_chunks_around_player(self.camera.position.x, self.camera.position.z);

        self.update_coords_ui();

        if self.mouse_captured && self.input.left_mouse {
            if let Some((bx, by, bz, _, _, _)) = self.camera.raycast(&self.world, 5.0) {
                let target = (bx, by, bz);
                let block = self.world.get_block(bx, by, bz);
                let break_time = block.break_time();

                if break_time.is_finite() && break_time > 0.0 {
                    if self.digging.target == Some(target) {
                        self.digging.progress += dt;
                        if self.digging.progress >= break_time {
                            self.world.set_block(bx, by, bz, BlockType::Air);
                            self.mark_chunk_dirty(bx, by, bz);
                            self.digging.target = None;
                            self.digging.progress = 0.0;
                        }
                    } else {
                        self.digging.target = Some(target);
                        self.digging.progress = 0.0;
                        self.digging.break_time = break_time;
                    }
                }
            } else {
                self.digging.target = None;
                self.digging.progress = 0.0;
            }
        } else {
            self.digging.target = None;
            self.digging.progress = 0.0;
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.staging_belt.recall();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let aspect = self.config.width as f32 / self.config.height as f32;
        let proj = cgmath::perspective(Rad(std::f32::consts::FRAC_PI_2), aspect, 0.1, 500.0);
        let view_mat = self.camera.view_matrix();
        let view_proj = proj * view_mat;
        let view_proj_array: [[f32; 4]; 4] = view_proj.into();

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[Uniforms {
                view_proj: view_proj_array,
                camera_pos: self.camera.eye_position().into(),
                time: self.game_start_time.elapsed().as_secs_f32(),
            }]),
        );

        let frustum_planes = extract_frustum_planes(&view_proj);

        let player_cx = (self.camera.position.x as i32) / CHUNK_SIZE;
        let player_cz = (self.camera.position.z as i32) / CHUNK_SIZE;

        let mut meshes_to_build: Vec<(i32, i32, i32)> = Vec::new();

        for cx in (player_cx - RENDER_DISTANCE)..=(player_cx + RENDER_DISTANCE) {
            for cz in (player_cz - RENDER_DISTANCE)..=(player_cz + RENDER_DISTANCE) {
                if let Some(chunk) = self.world.chunks.get(&(cx, cz)) {
                    for (sy, subchunk) in chunk.subchunks.iter().enumerate() {
                        if subchunk.mesh_dirty && !subchunk.is_empty {
                            meshes_to_build.push((cx, cz, sy as i32));
                        }
                    }
                }
            }
        }

        let built_meshes: Vec<(
            i32,
            i32,
            i32,
            (Vec<Vertex>, Vec<u32>),
            (Vec<Vertex>, Vec<u32>),
        )> = meshes_to_build
            .iter()
            .map(|&(cx, cz, sy)| {
                let meshes = self.world.build_subchunk_mesh(cx, cz, sy);
                (cx, cz, sy, meshes.0, meshes.1)
            })
            .collect();

        for (cx, cz, sy, (vertices, indices), (w_vertices, w_indices)) in built_meshes {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz)) {
                let subchunk = &mut chunk.subchunks[sy as usize];

                subchunk.num_indices = indices.len() as u32;
                if !vertices.is_empty() {
                    subchunk.vertex_buffer = Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Subchunk Vertex Buffer"),
                            contents: bytemuck::cast_slice(&vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        },
                    ));
                    subchunk.index_buffer = Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Subchunk Index Buffer"),
                            contents: bytemuck::cast_slice(&indices),
                            usage: wgpu::BufferUsages::INDEX,
                        },
                    ));
                } else {
                    subchunk.vertex_buffer = None;
                    subchunk.index_buffer = None;
                }

                subchunk.num_water_indices = w_indices.len() as u32;
                if !w_vertices.is_empty() {
                    subchunk.water_vertex_buffer = Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Water Vertex Buffer"),
                            contents: bytemuck::cast_slice(&w_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        },
                    ));
                    subchunk.water_index_buffer = Some(self.device.create_buffer_init(
                        &wgpu::util::BufferInitDescriptor {
                            label: Some("Water Index Buffer"),
                            contents: bytemuck::cast_slice(&w_indices),
                            usage: wgpu::BufferUsages::INDEX,
                        },
                    ));
                } else {
                    subchunk.water_vertex_buffer = None;
                    subchunk.water_index_buffer = None;
                }

                subchunk.mesh_dirty = false;
            }
        }

        let mut chunks_rendered = 0u32;
        let mut subchunks_rendered = 0u32;

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.53,
                            g: 0.81,
                            b: 0.98,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            for cx in (player_cx - RENDER_DISTANCE)..=(player_cx + RENDER_DISTANCE) {
                for cz in (player_cz - RENDER_DISTANCE)..=(player_cz + RENDER_DISTANCE) {
                    if let Some(chunk) = self.world.chunks.get(&(cx, cz)) {
                        let mut chunk_visible = false;
                        for subchunk in &chunk.subchunks {
                            if subchunk.is_empty || subchunk.num_indices == 0 {
                                continue;
                            }
                            if !subchunk.aabb.is_visible(&frustum_planes) {
                                continue;
                            }
                            if let (Some(vb), Some(ib)) =
                                (&subchunk.vertex_buffer, &subchunk.index_buffer)
                            {
                                render_pass.set_vertex_buffer(0, vb.slice(..));
                                render_pass
                                    .set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                                render_pass.draw_indexed(0..subchunk.num_indices, 0, 0..1);
                                subchunks_rendered += 1;
                                chunk_visible = true;
                            }
                        }
                        if chunk_visible {
                            chunks_rendered += 1;
                        }
                    }
                }
            }

            render_pass.set_pipeline(&self.water_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            for cx in (player_cx - RENDER_DISTANCE)..=(player_cx + RENDER_DISTANCE) {
                for cz in (player_cz - RENDER_DISTANCE)..=(player_cz + RENDER_DISTANCE) {
                    if let Some(chunk) = self.world.chunks.get(&(cx, cz)) {
                        for subchunk in &chunk.subchunks {
                            if subchunk.is_empty || subchunk.num_water_indices == 0 {
                                continue;
                            }
                            if !subchunk.aabb.is_visible(&frustum_planes) {
                                continue;
                            }
                            if let (Some(vb), Some(ib)) =
                                (&subchunk.water_vertex_buffer, &subchunk.water_index_buffer)
                            {
                                render_pass.set_vertex_buffer(0, vb.slice(..));
                                render_pass
                                    .set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                                render_pass.draw_indexed(0..subchunk.num_water_indices, 0, 0..1);
                            }
                        }
                    }
                }
            }
        }

        self.chunks_rendered = chunks_rendered;
        self.subchunks_rendered = subchunks_rendered;

        {
            let mut ui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("UI Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            ui_pass.set_pipeline(&self.crosshair_pipeline);
            ui_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            ui_pass.set_vertex_buffer(0, self.crosshair_vertex_buffer.slice(..));
            ui_pass.set_index_buffer(
                self.crosshair_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            ui_pass.draw_indexed(0..self.num_crosshair_indices, 0, 0..1);
            if let (Some(vb), Some(ib)) = (&self.coords_vertex_buffer, &self.coords_index_buffer) {
                if self.coords_num_indices > 0 {
                    ui_pass.set_vertex_buffer(0, vb.slice(..));
                    ui_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    ui_pass.draw_indexed(0..self.coords_num_indices, 0, 0..1);
                }
            }
        }

        if self.digging.target.is_some() && self.digging.break_time > 0.0 {
            let progress = (self.digging.progress / self.digging.break_time).min(1.0);

            let bar_width = 0.15;
            let bar_height = 0.015;
            let bar_y = -0.05;

            let bg_color = [0.2, 0.2, 0.2];
            let prog_color = [1.0 - progress, progress, 0.0];

            let mut vertices = Vec::new();
            let mut indices = Vec::new();
            let normal = [0.0, 0.0, 1.0];

            vertices.push(Vertex {
                position: [-bar_width, bar_y - bar_height, 0.0],
                normal,
                color: bg_color,
                uv: [0.0, 0.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [bar_width, bar_y - bar_height, 0.0],
                normal,
                color: bg_color,
                uv: [1.0, 0.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [bar_width, bar_y + bar_height, 0.0],
                normal,
                color: bg_color,
                uv: [1.0, 1.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [-bar_width, bar_y + bar_height, 0.0],
                normal,
                color: bg_color,
                uv: [0.0, 1.0],
                tex_index: 0.0,
            });
            indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);

            let prog_width = bar_width * 2.0 * progress - bar_width;
            vertices.push(Vertex {
                position: [-bar_width + 0.005, bar_y - bar_height + 0.003, 0.0],
                normal,
                color: prog_color,
                uv: [0.0, 0.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [prog_width - 0.005, bar_y - bar_height + 0.003, 0.0],
                normal,
                color: prog_color,
                uv: [1.0, 0.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [prog_width - 0.005, bar_y + bar_height - 0.003, 0.0],
                normal,
                color: prog_color,
                uv: [1.0, 1.0],
                tex_index: 0.0,
            });
            vertices.push(Vertex {
                position: [-bar_width + 0.005, bar_y + bar_height - 0.003, 0.0],
                normal,
                color: prog_color,
                uv: [0.0, 1.0],
                tex_index: 0.0,
            });
            indices.extend_from_slice(&[4, 5, 6, 4, 6, 7]);

            let progress_vb = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Progress Bar VB"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let progress_ib = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Progress Bar IB"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

            let mut progress_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Progress Bar Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            progress_pass.set_pipeline(&self.crosshair_pipeline);
            progress_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            progress_pass.set_vertex_buffer(0, progress_vb.slice(..));
            progress_pass.set_index_buffer(progress_ib.slice(..), wgpu::IndexFormat::Uint32);
            progress_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
        }

        {
            let fps_text = format!("FPS: {:.0}", self.current_fps);
            self.glyph_brush.queue(Section {
                screen_position: (10.0, 10.0),
                bounds: (self.config.width as f32, self.config.height as f32),
                text: vec![
                    Text::new(&fps_text)
                        .with_color([1.0, 1.0, 1.0, 1.0])
                        .with_scale(40.0),
                ],
                ..Section::default()
            });

            self.glyph_brush
                .draw_queued(
                    &self.device,
                    &mut self.staging_belt,
                    &mut encoder,
                    &view,
                    self.config.width,
                    self.config.height,
                )
                .expect("Draw queued");
        }

        self.staging_belt.finish();
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn handle_mouse_input(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::Left => self.input.left_mouse = pressed,
            MouseButton::Right => self.input.right_mouse = pressed,
            _ => {}
        }

        if !self.mouse_captured {
            return;
        }

        if button == MouseButton::Right && pressed {
            if let Some((_, _, _, px, py, pz)) = self.camera.raycast(&self.world, 5.0) {
                self.world.set_block(px, py, pz, BlockType::Stone);
                self.mark_chunk_dirty(px, py, pz);
            }
        }
    }

    fn mark_chunk_dirty(&mut self, x: i32, y: i32, z: i32) {
        let cx = x / CHUNK_SIZE;
        let cz = z / CHUNK_SIZE;
        let sy = y / SUBCHUNK_HEIGHT;

        if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz)) {
            if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                chunk.subchunks[sy as usize].mesh_dirty = true;
            }
        }

        let lx = x % CHUNK_SIZE;
        let lz = z % CHUNK_SIZE;
        let ly = y % SUBCHUNK_HEIGHT;

        if lx == 0 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx - 1, cz)) {
                if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                    chunk.subchunks[sy as usize].mesh_dirty = true;
                }
            }
        }
        if lx == CHUNK_SIZE - 1 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx + 1, cz)) {
                if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                    chunk.subchunks[sy as usize].mesh_dirty = true;
                }
            }
        }
        if lz == 0 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz - 1)) {
                if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                    chunk.subchunks[sy as usize].mesh_dirty = true;
                }
            }
        }
        if lz == CHUNK_SIZE - 1 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz + 1)) {
                if sy >= 0 && (sy as usize) < chunk.subchunks.len() {
                    chunk.subchunks[sy as usize].mesh_dirty = true;
                }
            }
        }
        if ly == 0 && sy > 0 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz)) {
                chunk.subchunks[(sy - 1) as usize].mesh_dirty = true;
            }
        }
        if ly == SUBCHUNK_HEIGHT - 1 && sy < NUM_SUBCHUNKS - 1 {
            if let Some(chunk) = self.world.chunks.get_mut(&(cx, cz)) {
                chunk.subchunks[(sy + 1) as usize].mesh_dirty = true;
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("🎮 Mini Minecraft 256x256 | Loading...")
        .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
        .build(&event_loop)
        .unwrap();

    let mut state = pollster::block_on(State::new(window));

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    state.resize(size);
                    state.window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::RedrawRequested,
                    ..
                } => {
                    state.frame_count += 1;
                    let now = Instant::now();
                    let elapsed = now.duration_since(state.last_fps_update).as_secs_f32();

                    if elapsed >= 0.5 {
                        state.current_fps = state.frame_count as f32 / elapsed;
                        state.frame_count = 0;
                        state.last_fps_update = now;
                    }

                    state.update();

                    match state.render() {
                        Ok(_) => {}
                        Err(wgpu::SurfaceError::Lost) => state.resize(state.window.inner_size()),
                        Err(wgpu::SurfaceError::OutOfMemory) => elwt.exit(),
                        Err(e) => eprintln!("Render error: {:?}", e),
                    }

                    state.window.request_redraw();
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    physical_key: PhysicalKey::Code(key),
                                    state: key_state,
                                    ..
                                },
                            ..
                        },
                    ..
                } => {
                    let pressed = key_state == ElementState::Pressed;
                    match key {
                        KeyCode::KeyW => state.input.forward = pressed,
                        KeyCode::KeyS => state.input.backward = pressed,
                        KeyCode::KeyA => state.input.left = pressed,
                        KeyCode::KeyD => state.input.right = pressed,
                        KeyCode::Space => state.input.jump = pressed,
                        KeyCode::ShiftLeft => state.input.sprint = pressed,
                        KeyCode::Escape if pressed => {
                            state.mouse_captured = false;
                            let _ = state.window.set_cursor_grab(CursorGrabMode::None);
                            state.window.set_cursor_visible(true);
                        }
                        KeyCode::F11 if pressed => {
                            if state.window.fullscreen().is_some() {
                                state.window.set_fullscreen(None);
                            } else {
                                state.window.set_fullscreen(Some(
                                    winit::window::Fullscreen::Borderless(None),
                                ));
                            }
                        }
                        _ => {}
                    }
                }
                Event::WindowEvent {
                    event:
                        WindowEvent::MouseInput {
                            state: btn_state,
                            button,
                            ..
                        },
                    ..
                } => {
                    let pressed = btn_state == ElementState::Pressed;

                    if pressed && !state.mouse_captured {
                        state.mouse_captured = true;
                        let _ = state
                            .window
                            .set_cursor_grab(CursorGrabMode::Confined)
                            .or_else(|_| state.window.set_cursor_grab(CursorGrabMode::Locked));
                        state.window.set_cursor_visible(false);
                        let _ = state.window.set_cursor_position(PhysicalPosition::new(
                            state.config.width / 2,
                            state.config.height / 2,
                        ));
                    } else {
                        state.handle_mouse_input(button, pressed);
                    }
                }
                Event::DeviceEvent {
                    event: DeviceEvent::MouseMotion { delta },
                    ..
                } => {
                    if state.mouse_captured {
                        let sensitivity = 0.002;
                        state.camera.yaw += delta.0 as f32 * sensitivity;
                        state.camera.pitch -= delta.1 as f32 * sensitivity;
                        state.camera.pitch = state.camera.pitch.clamp(
                            -std::f32::consts::FRAC_PI_2 + 0.1,
                            std::f32::consts::FRAC_PI_2 - 0.1,
                        );
                    }
                }
                Event::AboutToWait => {
                    state.window.request_redraw();
                }
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => elwt.exit(),
                _ => {}
            }
        })
        .unwrap();
}
