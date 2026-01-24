use noise::{NoiseFn, Simplex};
use std::collections::HashMap;

use crate::constants::*;
use crate::core::biome::Biome;
use crate::core::block::BlockType;
use crate::core::chunk::Chunk;
use crate::core::vertex::Vertex;
use crate::render::mesh::{add_greedy_quad, add_quad};
use crate::world::structures::{House, Structure};

pub struct World {
    pub chunks: HashMap<(i32, i32), Chunk>,
    simplex_continents: Simplex,
    simplex_terrain: Simplex,
    simplex_detail: Simplex,
    simplex_temperature: Simplex,
    simplex_moisture: Simplex,
    simplex_river: Simplex,
    simplex_lake: Simplex,
    simplex_trees: Simplex,
    simplex_island: Simplex,
    simplex_cave1: Simplex,
    simplex_cave2: Simplex,
    simplex_ore: Simplex,
    simplex_erosion: Simplex,
    pub seed: u32,
}

impl World {
    pub fn new() -> Self {
        Self::new_with_seed(2137) // TODO: Randomize seed, for now it's fixed, easier to debug
    }

    pub fn new_with_seed(seed: u32) -> Self {
        use crate::world::generator::ChunkGenerator;

        let generator = ChunkGenerator::new(seed);

        let mut world = World {
            chunks: HashMap::new(),
            simplex_continents: Simplex::new(seed),
            simplex_terrain: Simplex::new(seed.wrapping_add(1)),
            simplex_detail: Simplex::new(seed.wrapping_add(2)),
            simplex_temperature: Simplex::new(seed.wrapping_add(3)),
            simplex_moisture: Simplex::new(seed.wrapping_add(4)),
            simplex_river: Simplex::new(seed.wrapping_add(5)),
            simplex_lake: Simplex::new(seed.wrapping_add(6)),
            simplex_trees: Simplex::new(seed.wrapping_add(7)),
            simplex_island: Simplex::new(seed.wrapping_add(8)),
            simplex_cave1: Simplex::new(seed.wrapping_add(9)),
            simplex_cave2: Simplex::new(seed.wrapping_add(10)),
            simplex_ore: Simplex::new(seed.wrapping_add(11)),
            simplex_erosion: Simplex::new(seed.wrapping_add(12)),
            seed,
        };

        let spawn_cx = 0;
        let spawn_cz = 0;
        // Generate only a smaller initial radius for faster startup
        // More chunks will be generated as the player moves
        let initial_radius = 6;
        for cx in (spawn_cx - initial_radius)..=(spawn_cx + initial_radius) {
            for cz in (spawn_cz - initial_radius)..=(spawn_cz + initial_radius) {
                // Use ChunkGenerator for consistent terrain with async loader
                if !world.chunks.contains_key(&(cx, cz)) {
                    let chunk = generator.generate_chunk(cx, cz);
                    world.chunks.insert((cx, cz), chunk);
                }
            }
        }

        world
    }

    pub fn print_nearby_cave_entrances(&self, center_x: i32, center_z: i32, radius: i32) {
        let mut found = 0;

        for x in (center_x - radius)..=(center_x + radius) {
            for z in (center_z - radius)..=(center_z + radius) {
                let height = self.get_terrain_height(x, z);
                if self.is_cave_entrance(x, z, height) {
                    println!("Caves entrances: X={}, Y={}, Z={}", x, height - 1, z);
                    found += 1;
                }
            }
        }

        if found == 0 {
            println!("Cave entrances not found in this area.");
            println!("Try digging down or look for caves above!");
        } else {
            println!("Found {} cave entrances", found);
        }
    }

    pub fn ensure_chunk_generated(&mut self, cx: i32, cz: i32) {
        if self.chunks.contains_key(&(cx, cz)) {
            return;
        }
        self.generate_chunk(cx, cz);
    }

    pub fn update_chunks_around_player(&mut self, player_x: f32, player_z: f32) {
        let player_cx = (player_x / CHUNK_SIZE as f32).floor() as i32;
        let player_cz = (player_z / CHUNK_SIZE as f32).floor() as i32;

        // Synchronous generation removed - now handled asynchronously by ChunkLoader in main.rs
        // This prevents "dead frames" and GPU usage drops during exploration.

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

    pub fn get_biome(&self, x: i32, z: i32) -> Biome {
        let scale_continent = 0.002;
        let scale_temp = 0.008;
        let scale_moist = 0.01;
        let scale_river = 0.06;
        let scale_lake = 0.025;

        let continent = self
            .simplex_continents
            .get([x as f64 * scale_continent, z as f64 * scale_continent]);
        let river_noise = self
            .simplex_river
            .get([x as f64 * scale_river, z as f64 * scale_river]);
        let river_value = 1.0 - river_noise.abs() * 1.5;

        let lake_noise = self
            .simplex_lake
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
                .simplex_island
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
            .simplex_temperature
            .get([x as f64 * scale_temp, z as f64 * scale_temp]);
        let moist = self
            .simplex_moisture
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
                    .simplex_terrain
                    .get([x as f64 * 0.005, z as f64 * 0.005]);
                if mountain_noise > 0.4 {
                    Biome::Mountains
                } else {
                    Biome::Plains
                }
            }
        }
    }

    fn sample_fbm(
        &self,
        noise: &Simplex,
        x: f64,
        z: f64,
        octaves: u32,
        persistence: f64,
        lacunarity: f64,
        scale: f64,
    ) -> f64 {
        let mut total = 0.0;
        let mut amplitude = 1.0;
        let mut frequency = scale;
        let mut max_value = 0.0;

        for _ in 0..octaves {
            total += noise.get([x * frequency, z * frequency]) * amplitude;
            max_value += amplitude;
            amplitude *= persistence;
            frequency *= lacunarity;
        }

        total / max_value
    }

    pub fn get_terrain_height(&self, x: i32, z: i32) -> i32 {
        let blend_radius = 1; // Reduced from 2 for 64% fewer calculations
        let mut total_height = 0.0;
        let mut weights = 0.0;

        for dx in -blend_radius..=blend_radius {
            for dz in -blend_radius..=blend_radius {
                let wx = x + dx;
                let wz = z + dz;
                let dist_sq = (dx * dx + dz * dz) as f64;
                let weight = 1.0 / (1.0 + dist_sq);

                let height = self.calculate_base_height_at(wx, wz);
                total_height += height * weight;
                weights += weight;
            }
        }

        let base_height = total_height / weights;
        (base_height as i32).clamp(1, WORLD_HEIGHT - 20)
    }

    fn calculate_base_height_at(&self, x: i32, z: i32) -> f64 {
        let biome = self.get_biome(x, z);
        let fx = x as f64;
        let fz = z as f64;

        let continental = self.sample_fbm(&self.simplex_continents, fx, fz, 3, 0.5, 2.0, 0.001);
        let terrain = self.sample_fbm(&self.simplex_terrain, fx, fz, 3, 0.5, 2.0, 0.008);
        let detail = self.sample_fbm(&self.simplex_detail, fx, fz, 3, 0.4, 2.0, 0.015);
        let erosion = self.sample_fbm(&self.simplex_erosion, fx, fz, 2, 0.5, 2.0, 0.005);

        match biome {
            Biome::Ocean => {
                let depth = (continental + 1.0) * 0.5 * 15.0 + 35.0;
                depth + detail * 3.0
            }
            Biome::River => (SEA_LEVEL - 3) as f64 + detail * 2.0,
            Biome::Lake => (SEA_LEVEL - 4) as f64 + detail * 2.0,
            Biome::Beach => SEA_LEVEL as f64 + terrain * 2.0 + detail * 1.0,
            Biome::Island => {
                let island_noise = self.simplex_island.get([fx * 0.05, fz * 0.05]);
                let island_height = (island_noise + 1.0) * 0.5 * 25.0;
                (SEA_LEVEL as f64 + island_height + detail * 3.0).max(SEA_LEVEL as f64 - 5.0)
            }
            Biome::Plains => {
                let flatness = 1.0 - erosion.abs() * 0.5;
                let base = 66.0;
                base + terrain * 4.0 * flatness + detail * 2.0
            }
            Biome::Forest => {
                let base = 68.0;
                base + terrain * 8.0 + detail * 3.0
            }
            Biome::Desert => {
                let dune_noise = self.simplex_detail.get([fx * 0.02, fz * 0.02]);
                let dune = (dune_noise + 1.0) * 0.5 * 8.0;
                let base = 65.0;
                base + terrain * 5.0 + dune + detail * 2.0
            }
            Biome::Tundra => {
                let base = 68.0;
                base + terrain * 6.0 + detail * 2.0
            }
            Biome::Mountains => {
                let peaks = self.sample_fbm(
                    &self.simplex_terrain,
                    fx + 1000.0,
                    fz + 1000.0,
                    3,
                    0.6,
                    2.5,
                    0.01,
                );
                let base = 80.0;
                let mountain_height = (terrain + 1.0) * 0.5 * 60.0;
                let peak_factor = (peaks + 1.0) * 0.5;
                base + mountain_height * (0.5 + peak_factor * 0.5) + detail * 5.0
            }
            Biome::Swamp => {
                let base = SEA_LEVEL as f64 + 1.0;
                base + terrain * 2.0 + detail * 1.0
            }
        }
    }

    fn is_cave(&self, x: i32, y: i32, z: i32, surface_height: i32) -> bool {
        if y <= 5 {
            return false;
        }

        let fx = x as f64;
        let fy = y as f64;
        let fz = z as f64;

        let is_entrance = self.is_cave_entrance(x, z, surface_height);

        let min_surface_distance = if is_entrance { 0 } else { 8 };
        if y >= surface_height - min_surface_distance {
            return false;
        }

        let cave_scale = 0.05;
        let cave1 =
            self.simplex_cave1
                .get([fx * cave_scale, fy * cave_scale * 0.5, fz * cave_scale]);
        let cave2 = self.simplex_cave2.get([
            fx * cave_scale * 0.7,
            fy * cave_scale * 0.4,
            fz * cave_scale * 0.7,
        ]);

        let cheese_threshold = 0.7;
        let is_cheese_cave = cave1 > cheese_threshold && cave2 > cheese_threshold;
        let spaghetti_scale = 0.08;
        let spag1 = self.simplex_cave1.get([
            fx * spaghetti_scale + 500.0,
            fy * spaghetti_scale,
            fz * spaghetti_scale,
        ]);
        let spag2 = self.simplex_cave2.get([
            fx * spaghetti_scale + 500.0,
            fy * spaghetti_scale,
            fz * spaghetti_scale,
        ]);
        let spaghetti_threshold = 0.88;
        let is_spaghetti_cave =
            spag1.abs() < (1.0 - spaghetti_threshold) && spag2.abs() < (1.0 - spaghetti_threshold);

        let depth_factor = if y < 30 {
            1.0
        } else if y < 50 {
            0.8
        } else {
            0.5
        };

        (is_cheese_cave || is_spaghetti_cave)
            && (self.position_hash_3d(x, y, z) % 100) as f64 / 100.0 < depth_factor
    }

    fn is_cave_entrance(&self, x: i32, z: i32, surface_height: i32) -> bool {
        if surface_height <= SEA_LEVEL + 2 {
            return false;
        }

        let entrance_scale = 0.02;
        let entrance_noise = self.simplex_cave1.get([
            x as f64 * entrance_scale + 1000.0,
            z as f64 * entrance_scale + 1000.0,
        ]);
        if entrance_noise < 0.85 {
            return false;
        }

        let hash = self.position_hash(x, z);
        if hash % 10 != 0 {
            return false;
        }
        for check_y in (surface_height - 30).max(10)..=(surface_height - 10) {
            let fx = x as f64;
            let fy = check_y as f64;
            let fz = z as f64;

            let cave_scale = 0.05;
            let cave1 =
                self.simplex_cave1
                    .get([fx * cave_scale, fy * cave_scale * 0.5, fz * cave_scale]);
            let cave2 = self.simplex_cave2.get([
                fx * cave_scale * 0.7,
                fy * cave_scale * 0.4,
                fz * cave_scale * 0.7,
            ]);

            if cave1 > 0.7 && cave2 > 0.7 {
                return true;
            }
        }

        false
    }

    fn get_ore_at(&self, x: i32, y: i32, z: i32) -> Option<BlockType> {
        let hash = self.position_hash_3d(x, y, z);
        let ore_noise = self
            .simplex_ore
            .get([x as f64 * 0.1, y as f64 * 0.1, z as f64 * 0.1]);

        if ore_noise < 0.3 {
            return None;
        }

        let rarity = (hash % 1000) as f64 / 1000.0;

        if y < 128 && rarity < 0.02 {
            return Some(BlockType::Stone);
        }
        if y < 64 && rarity < 0.015 {
            return Some(BlockType::Stone);
        }

        if y < 32 && rarity < 0.005 {
            return Some(BlockType::Stone);
        }
        if y < 16 && rarity < 0.002 {
            return Some(BlockType::Stone);
        }

        None
    }

    fn get_3d_density(&self, x: i32, y: i32, z: i32, biome: Biome, surface_height: i32) -> f64 {
        let fx = x as f64;
        let fy = y as f64;
        let fz = z as f64;

        let vertical_gradient = (surface_height as f64 - fy) / 8.0;

        let density_noise = match biome {
            Biome::Mountains => {
                let scale = 0.02;
                self.sample_fbm(&self.simplex_terrain, fx, fz, 3, 0.5, 2.0, scale) * 0.5
                    + self.simplex_detail.get([fx * 0.04, fy * 0.04, fz * 0.04]) * 0.5
            }
            Biome::Island => {
                let scale = 0.03;
                self.simplex_terrain
                    .get([fx * scale, fy * scale, fz * scale])
                    * 0.4
            }
            _ => 0.0,
        };

        vertical_gradient + density_noise
    }

    fn position_hash_3d(&self, x: i32, y: i32, z: i32) -> u32 {
        let mut hash = self.seed;
        hash = hash.wrapping_add(x as u32).wrapping_mul(73856093);
        hash = hash.wrapping_add(y as u32).wrapping_mul(19349663);
        hash = hash.wrapping_add(z as u32).wrapping_mul(83492791);
        hash ^ (hash >> 16)
    }

    fn generate_chunk(&mut self, cx: i32, cz: i32) {
        let mut chunk = Chunk::new(cx, cz);
        let base_x = cx * CHUNK_SIZE;
        let base_z = cz * CHUNK_SIZE;

        // Pre-compute biome and height maps for this chunk (major optimization)
        let mut biome_map = [[Biome::Plains; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        let mut height_map = [[0i32; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let world_x = base_x + lx;
                let world_z = base_z + lz;
                biome_map[lx as usize][lz as usize] = self.get_biome(world_x, world_z);
                height_map[lx as usize][lz as usize] = self.get_terrain_height(world_x, world_z);
            }
        }

        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let world_x = base_x + lx;
                let world_z = base_z + lz;
                let biome = biome_map[lx as usize][lz as usize];
                let surface_height = height_map[lx as usize][lz as usize];

                let max_y = if matches!(biome, Biome::Mountains | Biome::Island) {
                    WORLD_HEIGHT - 20
                } else {
                    // Ensure we iterate up to at least SEA_LEVEL to generate water correctly
                    (surface_height + 5).max(SEA_LEVEL)
                };

                for y in 0..max_y {
                    let mut is_solid = false;
                    if y < surface_height {
                        is_solid = true;
                    }

                    if matches!(biome, Biome::Mountains | Biome::Island) && y >= surface_height - 8
                    {
                        let density =
                            self.get_3d_density(world_x, y, world_z, biome, surface_height);
                        if density > 0.0 {
                            is_solid = true;
                        }
                    }

                    if is_solid {
                        let block =
                            self.get_block_for_biome(biome, y, surface_height, world_x, world_z);
                        if block != BlockType::Air {
                            chunk.set_block(lx, y, lz, block);
                        }
                    } else if y >= surface_height && y < SEA_LEVEL {
                        if biome == Biome::Tundra && y == SEA_LEVEL - 1 {
                            chunk.set_block(lx, y, lz, BlockType::Ice);
                        } else {
                            chunk.set_block(lx, y, lz, BlockType::Water);
                        }
                    }
                }
            }
        }

        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let world_x = base_x + lx;
                let world_z = base_z + lz;
                let height = height_map[lx as usize][lz as usize]; // Use cached height

                for y in 1..height.min(WORLD_HEIGHT - 1) {
                    if self.is_cave(world_x, y, world_z, height) {
                        let current = chunk.get_block(lx, y, lz);
                        if current != BlockType::Water
                            && current != BlockType::Bedrock
                            && current != BlockType::Air
                        {
                            if y < SEA_LEVEL {
                                chunk.set_block(lx, y, lz, BlockType::Water);
                            } else {
                                chunk.set_block(lx, y, lz, BlockType::Air);
                            }
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

    fn get_block_for_biome(
        &self,
        biome: Biome,
        y: i32,
        surface_height: i32,
        world_x: i32,
        world_z: i32,
    ) -> BlockType {
        if y == 0 {
            return BlockType::Bedrock;
        }
        if y <= 4 {
            let bedrock_chance = (5 - y) as u32 * 20;
            let hash = self.position_hash_3d(world_x, y, world_z);
            if (hash % 100) < bedrock_chance {
                return BlockType::Bedrock;
            }
        }

        let depth_from_surface = surface_height - y;
        let dirt_depth = 3 + (self.position_hash(world_x, world_z) % 3) as i32;

        match biome {
            Biome::Ocean | Biome::River | Biome::Lake => {
                if depth_from_surface > 4 {
                    BlockType::Stone
                } else if depth_from_surface > 1 {
                    BlockType::Gravel
                } else if y < surface_height {
                    BlockType::Sand
                } else {
                    BlockType::Air
                }
            }
            Biome::Beach | Biome::Island => {
                if depth_from_surface > 6 {
                    BlockType::Stone
                } else if depth_from_surface > 0 {
                    BlockType::Sand
                } else if y == surface_height - 1 {
                    if biome == Biome::Island && y > SEA_LEVEL {
                        BlockType::Grass
                    } else {
                        BlockType::Sand
                    }
                } else {
                    BlockType::Air
                }
            }
            Biome::Desert => {
                if depth_from_surface > 10 {
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
                if depth_from_surface > dirt_depth + 2 {
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
                if y > 140 {
                    if y == surface_height - 1 {
                        BlockType::Snow
                    } else if depth_from_surface > 0 {
                        BlockType::Stone
                    } else {
                        BlockType::Air
                    }
                } else if y > 110 {
                    if depth_from_surface > 2 {
                        BlockType::Stone
                    } else if y == surface_height - 1 {
                        BlockType::Grass
                    } else {
                        BlockType::Stone
                    }
                } else {
                    if depth_from_surface > dirt_depth {
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
            Biome::Swamp => {
                if depth_from_surface > dirt_depth {
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
                if depth_from_surface > dirt_depth {
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

    fn is_valid_tree_ground(&self, block: BlockType) -> bool {
        matches!(block, BlockType::Grass | BlockType::Dirt)
    }

    fn position_hash(&self, x: i32, z: i32) -> u32 {
        let mut hash = self.seed;
        hash = hash.wrapping_add(x as u32).wrapping_mul(73856093);
        hash = hash.wrapping_add(z as u32).wrapping_mul(19349663);
        hash ^ (hash >> 16)
    }

    fn can_place_tree(&self, chunk: &Chunk, lx: i32, y: i32, lz: i32, is_large: bool) -> bool {
        let min_distance = if is_large { 5 } else { 3 };
        for dx in -min_distance..=min_distance {
            for dz in -min_distance..=min_distance {
                let check_x = lx + dx;
                let check_z = lz + dz;

                if check_x < 0 || check_x >= CHUNK_SIZE || check_z < 0 || check_z >= CHUNK_SIZE {
                    continue;
                }

                for dy in -1..=8 {
                    let check_y = y + dy;
                    if check_y < 0 || check_y >= WORLD_HEIGHT {
                        continue;
                    }
                    if chunk.get_block(check_x, check_y, check_z) == BlockType::Wood {
                        return false;
                    }
                }
            }
        }

        if is_large {
            for dx in 0..=1 {
                for dz in 0..=1 {
                    let check_x = lx + dx;
                    let check_z = lz + dz;

                    if check_x < 0 || check_x >= CHUNK_SIZE || check_z < 0 || check_z >= CHUNK_SIZE
                    {
                        return false;
                    }

                    let ground_block = chunk.get_block(check_x, y - 1, check_z);
                    if !self.is_valid_tree_ground(ground_block) {
                        return false;
                    }

                    for neighbor_dx in -1..=1 {
                        for neighbor_dz in -1..=1 {
                            let nx = check_x + neighbor_dx;
                            let nz = check_z + neighbor_dz;
                            if nx >= 0 && nx < CHUNK_SIZE && nz >= 0 && nz < CHUNK_SIZE {
                                let neighbor = chunk.get_block(nx, y - 1, nz);
                                if matches!(
                                    neighbor,
                                    BlockType::Stone
                                        | BlockType::Gravel
                                        | BlockType::Sand
                                        | BlockType::Water
                                        | BlockType::Ice
                                ) {
                                    return false;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            let ground_block = chunk.get_block(lx, y - 1, lz);
            if !self.is_valid_tree_ground(ground_block) {
                return false;
            }
            for dx in -1..=1 {
                for dz in -1..=1 {
                    let nx = lx + dx;
                    let nz = lz + dz;
                    if nx >= 0 && nx < CHUNK_SIZE && nz >= 0 && nz < CHUNK_SIZE {
                        let neighbor = chunk.get_block(nx, y - 1, nz);
                        if matches!(
                            neighbor,
                            BlockType::Stone
                                | BlockType::Gravel
                                | BlockType::Sand
                                | BlockType::Water
                                | BlockType::Ice
                        ) {
                            return false;
                        }
                    }
                }
            }
        }

        true
    }

    fn generate_chunk_decorations(&self, chunk: &mut Chunk, cx: i32, cz: i32) {
        let base_x = cx * CHUNK_SIZE;
        let base_z = cz * CHUNK_SIZE;

        let margin = 4;

        for lx in margin..(CHUNK_SIZE - margin) {
            for lz in margin..(CHUNK_SIZE - margin) {
                let world_x = base_x + lx;
                let world_z = base_z + lz;
                let biome = self.get_biome(world_x, world_z);
                let height = self.get_terrain_height(world_x, world_z);

                if height < SEA_LEVEL {
                    continue;
                }

                let tree_noise = self
                    .simplex_trees
                    .get([world_x as f64 * 0.3, world_z as f64 * 0.3]);

                if biome.has_trees() && tree_noise > biome.tree_density() {
                    let hash = self.position_hash(world_x, world_z);
                    let is_large = (hash % 100) < 25;

                    if self.can_place_tree(chunk, lx, height, lz, is_large) {
                        self.place_tree_in_chunk(chunk, lx, height, lz, biome, is_large);
                    }
                }

                if biome == Biome::Desert {
                    let cactus_noise = self
                        .simplex_trees
                        .get([world_x as f64 * 0.5 + 100.0, world_z as f64 * 0.5]);
                    if cactus_noise > 0.8 {
                        self.place_cactus_in_chunk(chunk, lx, height, lz);
                    } else if cactus_noise > 0.7 {
                        chunk.set_block(lx, height, lz, BlockType::DeadBush);
                    }
                }

                // Temporary House Generation

                // Use a cell-based approach for sparse, non-overlapping, randomized placement
                let cell_size = 50;
                let cx = world_x.div_euclid(cell_size);
                let cz = world_z.div_euclid(cell_size);

                let cell_hash = self.position_hash(cx, cz);
                let offset_x = (cell_hash % (cell_size as u32)) as i32;
                let offset_z = ((cell_hash >> 8) % (cell_size as u32)) as i32;

                let target_x = cx * cell_size + offset_x;
                let target_z = cz * cell_size + offset_z;

                if world_x == target_x && world_z == target_z {
                    let house_noise = self
                        .simplex_trees
                        .get([world_x as f64 * 0.02 + 2000.0, world_z as f64 * 0.02]);

                    if house_noise > 0.3 {
                        let house = House::new();
                        let biome_name = format!("{:?}", biome);
                        if house.structure.biomes.contains(&biome_name) {
                            println!(
                                "Generated House at X: {}, Y: {}, Z: {}",
                                world_x, height, world_z
                            );
                            self.place_structure(chunk, lx, height, lz, &house.structure);
                        }
                    }
                }
            }
        }
    }

    fn place_structure(&self, chunk: &mut Chunk, lx: i32, y: i32, lz: i32, structure: &Structure) {
        for (sx, sy, sz, block) in &structure.blocks {
            let tx = lx + sx;
            let ty = y + sy;
            let tz = lz + sz;

            if tx >= 0
                && tx < CHUNK_SIZE
                && tz >= 0
                && tz < CHUNK_SIZE
                && ty >= 0
                && ty < WORLD_HEIGHT
            {
                // Determine logic for overwrite.
                // For now, overwrite everything except Air if we want to preserve some things?
                // But structure usually contains Air for hollow parts.
                // If structure has explicit Air, we set Air (clearing terrain).
                // If structure has block, we set block.
                chunk.set_block(tx, ty, tz, *block);
            }
        }
    }

    fn place_tree_in_chunk(
        &self,
        chunk: &mut Chunk,
        lx: i32,
        y: i32,
        lz: i32,
        biome: Biome,
        is_large: bool,
    ) {
        let base_trunk_height = match biome {
            Biome::Forest => 6,
            Biome::Swamp => 7,
            Biome::Tundra => 4,
            _ => 5,
        };

        let trunk_height = if is_large {
            base_trunk_height + 2
        } else {
            base_trunk_height
        };

        if is_large {
            // Convert grass to dirt under the large tree trunk (2x2)
            for dx in 0..=1 {
                for dz in 0..=1 {
                    let tx = lx + dx;
                    let tz = lz + dz;
                    if tx >= 0 && tx < CHUNK_SIZE && tz >= 0 && tz < CHUNK_SIZE {
                        // Replace grass with dirt under trunk
                        if chunk.get_block(tx, y - 1, tz) == BlockType::Grass {
                            chunk.set_block(tx, y - 1, tz, BlockType::Dirt);
                        }
                    }
                }
            }

            // Place trunk
            for ty in 0..trunk_height {
                for dx in 0..=1 {
                    for dz in 0..=1 {
                        let tx = lx + dx;
                        let tz = lz + dz;
                        if tx >= 0 && tx < CHUNK_SIZE && tz >= 0 && tz < CHUNK_SIZE {
                            chunk.set_block(tx, y + ty, tz, BlockType::Wood);
                        }
                    }
                }
            }

            let crown_center_y = y + trunk_height;
            let crown_radius = if biome == Biome::Tundra {
                3.0_f32
            } else {
                4.0_f32
            };
            let crown_center_x = lx as f32 + 0.5;
            let crown_center_z = lz as f32 + 0.5;

            for dx in -5..=5 {
                for dy in -2..=5 {
                    for dz in -5..=5 {
                        let nlx = lx + dx;
                        let nly = crown_center_y + dy;
                        let nlz = lz + dz;

                        if nlx < 0 || nlx >= CHUNK_SIZE || nlz < 0 || nlz >= CHUNK_SIZE {
                            continue;
                        }

                        let fx = nlx as f32 - crown_center_x;
                        let fz = nlz as f32 - crown_center_z;
                        let dist = (fx * fx + (dy as f32 - 1.0).powi(2) + fz * fz).sqrt();

                        if dist <= crown_radius {
                            let existing = chunk.get_block(nlx, nly, nlz);
                            // Only place leaves on air or other leaves
                            if existing == BlockType::Air || existing == BlockType::Leaves {
                                chunk.set_block(nlx, nly, nlz, BlockType::Leaves);
                            }
                        }
                    }
                }
            }
        } else {
            // Convert grass to dirt under the small tree trunk
            if chunk.get_block(lx, y - 1, lz) == BlockType::Grass {
                chunk.set_block(lx, y - 1, lz, BlockType::Dirt);
            }

            // Place trunk
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
                            let existing = chunk.get_block(nlx, nly, nlz);
                            // Only place leaves on air or other leaves
                            if existing == BlockType::Air || existing == BlockType::Leaves {
                                chunk.set_block(nlx, nly, nlz, BlockType::Leaves);
                            }
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

    pub fn get_block(&self, x: i32, y: i32, z: i32) -> BlockType {
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

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockType) {
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

    pub fn set_block_player(&mut self, x: i32, y: i32, z: i32, block: BlockType) {
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
            chunk.player_modified = true;
        }
    }

    pub fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.get_block(x, y, z).is_solid()
    }

    pub fn is_subchunk_occluded(&self, cx: i32, cz: i32, sy: i32) -> bool {
        // A subchunk is occluded if it's fully opaque and all 6 adjacent subchunks are also fully opaque.
        // This is a conservative but very fast check.
        if let Some(chunk) = self.chunks.get(&(cx, cz)) {
            if !chunk.subchunks[sy as usize].is_fully_opaque {
                return false;
            }

            // Check +Y and -Y (within same chunk)
            if sy > 0 && !chunk.subchunks[(sy - 1) as usize].is_fully_opaque {
                return false;
            }
            if sy < NUM_SUBCHUNKS as i32 - 1 && !chunk.subchunks[(sy + 1) as usize].is_fully_opaque
            {
                return false;
            }
            // Borders of world height are not occluded
            if sy == 0 || sy == NUM_SUBCHUNKS as i32 - 1 {
                return false;
            }

            // Check X and Z neighbors
            let neighbors = [(cx - 1, cz), (cx + 1, cz), (cx, cz - 1), (cx, cz + 1)];
            for (ncx, ncz) in neighbors {
                if let Some(nchunk) = self.chunks.get(&(ncx, ncz)) {
                    if !nchunk.subchunks[sy as usize].is_fully_opaque {
                        return false;
                    }
                } else {
                    // If neighbor chunk is not loaded, we can't be sure it's occluded
                    return false;
                }
            }

            return true;
        }
        false
    }

    pub fn find_spawn_point(&self) -> (f32, f32, f32) {
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
                        return (x as f32 + 0.3, (height + 1) as f32, z as f32 + 0.5);
                    }
                }
            }
        }
        (0.5, 80.0, 0.5)
    }

    pub fn build_subchunk_mesh(
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

        // Cache chunk references to avoid HashMap lookups in the hot loop
        // This eliminates ~24,576 HashMap lookups per subchunk (6 neighbors × 16³ blocks)
        let chunk_center = self.chunks.get(&(chunk_x, chunk_z));
        let chunk_nx = self.chunks.get(&(chunk_x - 1, chunk_z));
        let chunk_px = self.chunks.get(&(chunk_x + 1, chunk_z));
        let chunk_nz = self.chunks.get(&(chunk_x, chunk_z - 1));
        let chunk_pz = self.chunks.get(&(chunk_x, chunk_z + 1));

        // Pre-compute biome map to avoid expensive noise calculations per-block
        let mut biome_map: [[Option<crate::biome::Biome>; CHUNK_SIZE as usize];
            CHUNK_SIZE as usize] = [[None; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];

        // Helper to get block from cached chunks
        let get_block_fast = |wx: i32, wy: i32, wz: i32| -> BlockType {
            if wy < 0 || wy >= WORLD_HEIGHT {
                return BlockType::Air;
            }

            let cx = if wx >= 0 {
                wx / CHUNK_SIZE
            } else {
                (wx - CHUNK_SIZE + 1) / CHUNK_SIZE
            };
            let cz = if wz >= 0 {
                wz / CHUNK_SIZE
            } else {
                (wz - CHUNK_SIZE + 1) / CHUNK_SIZE
            };
            let lx = wx.rem_euclid(CHUNK_SIZE);
            let lz = wz.rem_euclid(CHUNK_SIZE);

            let chunk = if cx == chunk_x && cz == chunk_z {
                chunk_center
            } else if cx == chunk_x - 1 && cz == chunk_z {
                chunk_nx
            } else if cx == chunk_x + 1 && cz == chunk_z {
                chunk_px
            } else if cx == chunk_x && cz == chunk_z - 1 {
                chunk_nz
            } else if cx == chunk_x && cz == chunk_z + 1 {
                chunk_pz
            } else {
                return BlockType::Air;
            };

            chunk
                .map(|c| c.get_block(lx, wy, lz))
                .unwrap_or(BlockType::Air)
        };

        // ============= GREEDY MESHING IMPLEMENTATION =============
        //
        // Instead of creating one quad per visible block face, we merge adjacent faces
        // of the same type into larger quads. This reduces vertex count by 50-90%.
        //
        // Algorithm:
        // 1. First pass: Handle special blocks (stairs) with naive approach
        // 2. For each face direction, for each slice perpendicular to that direction:
        //    a. Build a 2D mask of visible faces with their attributes
        //    b. Greedily merge adjacent faces with matching attributes
        //    c. Emit merged quads with tiled UVs

        // Face attributes for greedy merge comparison
        #[derive(Clone, Copy, PartialEq)]
        struct FaceAttrs {
            block: BlockType,
            color: [u8; 3], // Quantized color for comparison
            tex_index: u8,
            is_active: bool,
        }

        impl Default for FaceAttrs {
            fn default() -> Self {
                FaceAttrs {
                    block: BlockType::Air,
                    color: [0, 0, 0],
                    tex_index: 0,
                    is_active: false,
                }
            }
        }

        // Helper to quantize color for comparison (avoids floating point issues)
        let quantize_color = |c: [f32; 3]| -> [u8; 3] {
            [
                (c[0] * 255.0) as u8,
                (c[1] * 255.0) as u8,
                (c[2] * 255.0) as u8,
            ]
        };

        // First pass: Handle special blocks (WoodStairs, etc.) with naive approach
        for lx in 0..CHUNK_SIZE {
            for ly in 0..SUBCHUNK_HEIGHT {
                for lz in 0..CHUNK_SIZE {
                    let y = base_y + ly;
                    let world_x = base_x + lx;
                    let world_z = base_z + lz;
                    let block = get_block_fast(world_x, y, world_z);

                    if block == BlockType::Air {
                        continue;
                    }

                    let is_water = block == BlockType::Water;
                    let (target_verts, target_inds) = if is_water {
                        (&mut water_vertices, &mut water_indices)
                    } else {
                        (&mut vertices, &mut indices)
                    };

                    // WoodStairs uses naive approach (complex geometry)
                    if block == BlockType::WoodStairs {
                        let x = world_x as f32;
                        let y_f = y as f32;
                        let z = world_z as f32;
                        let color = block.color();
                        let tex_top = block.tex_top() as f32;
                        let tex_side = block.tex_side() as f32;
                        let r = block.roughness();
                        let m = block.metallic();

                        let neighbors = [
                            get_block_fast(world_x - 1, y, world_z),
                            get_block_fast(world_x + 1, y, world_z),
                            get_block_fast(world_x, y - 1, world_z),
                            get_block_fast(world_x, y + 1, world_z),
                            get_block_fast(world_x, y, world_z - 1),
                            get_block_fast(world_x, y, world_z + 1),
                        ];

                        // Bottom Face (-Y)
                        if block.should_render_face_against(neighbors[2]) {
                            add_quad(
                                target_verts,
                                target_inds,
                                [x, y_f, z + 1.0],
                                [x, y_f, z],
                                [x + 1.0, y_f, z],
                                [x + 1.0, y_f, z + 1.0],
                                [0.0, -1.0, 0.0],
                                color,
                                tex_top,
                                r,
                                m,
                            );
                        }
                        // Top of bottom slab
                        add_quad(
                            target_verts,
                            target_inds,
                            [x, y_f + 0.5, z],
                            [x, y_f + 0.5, z + 0.5],
                            [x + 1.0, y_f + 0.5, z + 0.5],
                            [x + 1.0, y_f + 0.5, z],
                            [0.0, 1.0, 0.0],
                            color,
                            tex_top,
                            r,
                            m,
                        );
                        // Top of top slab
                        if block.should_render_face_against(neighbors[3]) {
                            add_quad(
                                target_verts,
                                target_inds,
                                [x, y_f + 1.0, z + 0.5],
                                [x, y_f + 1.0, z + 1.0],
                                [x + 1.0, y_f + 1.0, z + 1.0],
                                [x + 1.0, y_f + 1.0, z + 0.5],
                                [0.0, 1.0, 0.0],
                                color,
                                tex_top,
                                r,
                                m,
                            );
                        }
                        // Front face (-Z)
                        if block.should_render_face_against(neighbors[4]) {
                            add_quad(
                                target_verts,
                                target_inds,
                                [x + 1.0, y_f, z],
                                [x, y_f, z],
                                [x, y_f + 0.5, z],
                                [x + 1.0, y_f + 0.5, z],
                                [0.0, 0.0, -1.0],
                                color,
                                tex_side,
                                r,
                                m,
                            );
                        }
                        // Riser
                        add_quad(
                            target_verts,
                            target_inds,
                            [x + 1.0, y_f + 0.5, z + 0.5],
                            [x, y_f + 0.5, z + 0.5],
                            [x, y_f + 1.0, z + 0.5],
                            [x + 1.0, y_f + 1.0, z + 0.5],
                            [0.0, 0.0, -1.0],
                            color,
                            tex_side,
                            r,
                            m,
                        );
                        // Back face (+Z)
                        if block.should_render_face_against(neighbors[5]) {
                            add_quad(
                                target_verts,
                                target_inds,
                                [x, y_f, z + 1.0],
                                [x + 1.0, y_f, z + 1.0],
                                [x + 1.0, y_f + 1.0, z + 1.0],
                                [x, y_f + 1.0, z + 1.0],
                                [0.0, 0.0, 1.0],
                                color,
                                tex_side,
                                r,
                                m,
                            );
                        }
                        // Left face (-X)
                        if block.should_render_face_against(neighbors[0]) {
                            add_quad(
                                target_verts,
                                target_inds,
                                [x, y_f, z],
                                [x, y_f, z + 1.0],
                                [x, y_f + 0.5, z + 1.0],
                                [x, y_f + 0.5, z],
                                [-1.0, 0.0, 0.0],
                                color,
                                tex_side,
                                r,
                                m,
                            );
                            add_quad(
                                target_verts,
                                target_inds,
                                [x, y_f + 0.5, z + 0.5],
                                [x, y_f + 0.5, z + 1.0],
                                [x, y_f + 1.0, z + 1.0],
                                [x, y_f + 1.0, z + 0.5],
                                [-1.0, 0.0, 0.0],
                                color,
                                tex_side,
                                r,
                                m,
                            );
                        }
                        // Right face (+X)
                        if block.should_render_face_against(neighbors[1]) {
                            add_quad(
                                target_verts,
                                target_inds,
                                [x + 1.0, y_f, z + 1.0],
                                [x + 1.0, y_f, z],
                                [x + 1.0, y_f + 0.5, z],
                                [x + 1.0, y_f + 0.5, z + 1.0],
                                [1.0, 0.0, 0.0],
                                color,
                                tex_side,
                                r,
                                m,
                            );
                            add_quad(
                                target_verts,
                                target_inds,
                                [x + 1.0, y_f + 0.5, z + 1.0],
                                [x + 1.0, y_f + 0.5, z + 0.5],
                                [x + 1.0, y_f + 1.0, z + 0.5],
                                [x + 1.0, y_f + 1.0, z + 1.0],
                                [1.0, 0.0, 0.0],
                                color,
                                tex_side,
                                r,
                                m,
                            );
                        }
                        continue;
                    }
                }
            }
        }

        // Second pass: Greedy meshing for regular solid blocks
        // Process each of 6 face directions separately

        // Face direction: 0=-X, 1=+X, 2=-Y, 3=+Y, 4=-Z, 5=+Z
        for face_dir in 0..6 {
            // Determine slice dimensions based on face direction
            // For X faces: slices are in YZ plane, iterate over X
            // For Y faces: slices are in XZ plane, iterate over Y
            // For Z faces: slices are in XY plane, iterate over Z

            let (slice_count, dim1_size, dim2_size): (i32, i32, i32) = match face_dir {
                0 | 1 => (CHUNK_SIZE, SUBCHUNK_HEIGHT, CHUNK_SIZE), // X faces: YZ slices
                2 | 3 => (SUBCHUNK_HEIGHT, CHUNK_SIZE, CHUNK_SIZE), // Y faces: XZ slices
                4 | 5 => (CHUNK_SIZE, CHUNK_SIZE, SUBCHUNK_HEIGHT), // Z faces: XY slices
                _ => unreachable!(),
            };

            // For each slice perpendicular to face direction
            for slice in 0..slice_count {
                // Build mask of visible faces in this slice
                let mut mask: Vec<FaceAttrs> =
                    vec![FaceAttrs::default(); (dim1_size * dim2_size) as usize];

                for d1 in 0..dim1_size {
                    for d2 in 0..dim2_size {
                        // Convert slice coordinates to world coordinates
                        let (lx, ly, lz): (i32, i32, i32) = match face_dir {
                            0 | 1 => (slice, d1, d2),
                            2 | 3 => (d1, slice, d2),
                            4 | 5 => (d1, d2, slice),
                            _ => unreachable!(),
                        };

                        let y = base_y + ly;
                        let world_x = base_x + lx;
                        let world_z = base_z + lz;
                        let block = get_block_fast(world_x, y, world_z);

                        // Water uses naive approach even in the greedy pass to prevent gaps
                        // caused by vertex displacement on large quads. We still draw it via IndirectManager.
                        if block == BlockType::Water {
                            let neighbors = [
                                get_block_fast(world_x - 1, y, world_z),
                                get_block_fast(world_x + 1, y, world_z),
                                get_block_fast(world_x, y - 1, world_z),
                                get_block_fast(world_x, y + 1, world_z),
                                get_block_fast(world_x, y, world_z - 1),
                                get_block_fast(world_x, y, world_z + 1),
                            ];

                            if block.should_render_face_against(neighbors[face_dir as usize]) {
                                let x = world_x as f32;
                                let y_f = y as f32;
                                let z = world_z as f32;
                                let color = block.color();
                                let tex = block.tex_top() as f32;
                                let r = block.roughness();
                                let m = block.metallic();

                                match face_dir {
                                    0 => add_quad(
                                        &mut water_vertices,
                                        &mut water_indices,
                                        [x, y_f, z],
                                        [x, y_f, z + 1.0],
                                        [x, y_f + 1.0, z + 1.0],
                                        [x, y_f + 1.0, z],
                                        [-1.0, 0.0, 0.0],
                                        color,
                                        tex,
                                        r,
                                        m,
                                    ),
                                    1 => add_quad(
                                        &mut water_vertices,
                                        &mut water_indices,
                                        [x + 1.0, y_f, z + 1.0],
                                        [x + 1.0, y_f, z],
                                        [x + 1.0, y_f + 1.0, z],
                                        [x + 1.0, y_f + 1.0, z + 1.0],
                                        [1.0, 0.0, 0.0],
                                        color,
                                        tex,
                                        r,
                                        m,
                                    ),
                                    2 => add_quad(
                                        &mut water_vertices,
                                        &mut water_indices,
                                        [x, y_f, z + 1.0],
                                        [x, y_f, z],
                                        [x + 1.0, y_f, z],
                                        [x + 1.0, y_f, z + 1.0],
                                        [0.0, -1.0, 0.0],
                                        color,
                                        tex,
                                        r,
                                        m,
                                    ),
                                    3 => add_quad(
                                        &mut water_vertices,
                                        &mut water_indices,
                                        [x, y_f + 1.0, z],
                                        [x, y_f + 1.0, z + 1.0],
                                        [x + 1.0, y_f + 1.0, z + 1.0],
                                        [x + 1.0, y_f + 1.0, z],
                                        [0.0, 1.0, 0.0],
                                        color,
                                        tex,
                                        r,
                                        m,
                                    ),
                                    4 => add_quad(
                                        &mut water_vertices,
                                        &mut water_indices,
                                        [x + 1.0, y_f, z],
                                        [x, y_f, z],
                                        [x, y_f + 1.0, z],
                                        [x + 1.0, y_f + 1.0, z],
                                        [0.0, 0.0, -1.0],
                                        color,
                                        tex,
                                        r,
                                        m,
                                    ),
                                    5 => add_quad(
                                        &mut water_vertices,
                                        &mut water_indices,
                                        [x, y_f, z + 1.0],
                                        [x + 1.0, y_f, z + 1.0],
                                        [x + 1.0, y_f + 1.0, z + 1.0],
                                        [x, y_f + 1.0, z + 1.0],
                                        [0.0, 0.0, 1.0],
                                        color,
                                        tex,
                                        r,
                                        m,
                                    ),
                                    _ => {}
                                }
                            }
                            continue;
                        }

                        // Skip air and other special blocks
                        if block == BlockType::Air || block == BlockType::WoodStairs {
                            continue;
                        }

                        // Get neighbor in face direction
                        let (nx, ny, nz) = match face_dir {
                            0 => (world_x - 1, y, world_z),
                            1 => (world_x + 1, y, world_z),
                            2 => (world_x, y - 1, world_z),
                            3 => (world_x, y + 1, world_z),
                            4 => (world_x, y, world_z - 1),
                            5 => (world_x, y, world_z + 1),
                            _ => unreachable!(),
                        };
                        let neighbor = get_block_fast(nx, ny, nz);

                        if !block.should_render_face_against(neighbor) {
                            continue;
                        }

                        // Compute color (with biome for grass/leaves)
                        let needs_biome = block == BlockType::Grass || block == BlockType::Leaves;
                        let biome = if needs_biome {
                            let lx_idx = lx as usize;
                            let lz_idx = lz as usize;
                            if biome_map[lx_idx][lz_idx].is_none() {
                                biome_map[lx_idx][lz_idx] = Some(self.get_biome(world_x, world_z));
                            }
                            biome_map[lx_idx][lz_idx]
                        } else {
                            None
                        };

                        let color = match face_dir {
                            2 => block.bottom_color(),
                            3 => {
                                if block == BlockType::Grass {
                                    biome.unwrap().grass_color()
                                } else {
                                    block.top_color()
                                }
                            }
                            _ => {
                                if block == BlockType::Grass {
                                    block.color()
                                } else if block == BlockType::Leaves {
                                    biome.unwrap().leaves_color()
                                } else {
                                    block.color()
                                }
                            }
                        };

                        let tex_index = match face_dir {
                            2 => block.tex_bottom(),
                            3 => block.tex_top(),
                            _ => block.tex_side(),
                        };

                        let idx = (d1 * dim2_size + d2) as usize;
                        mask[idx] = FaceAttrs {
                            block,
                            color: quantize_color(color),
                            tex_index: tex_index as u8,
                            is_active: true,
                        };
                    }
                }

                // Greedy merge: scan the mask and merge adjacent faces
                for d1 in 0..dim1_size {
                    let mut d2 = 0;
                    while d2 < dim2_size {
                        let idx = (d1 * dim2_size + d2) as usize;
                        let face = mask[idx];

                        if !face.is_active {
                            d2 += 1;
                            continue;
                        }

                        // Found an active face, try to extend it
                        let mut width = 1i32;
                        while d2 + width < dim2_size {
                            let next_idx = (d1 * dim2_size + d2 + width) as usize;
                            if mask[next_idx] == face {
                                width += 1;
                            } else {
                                break;
                            }
                        }

                        // Try to extend height (d1 direction)
                        let mut height = 1i32;
                        'height_loop: while d1 + height < dim1_size {
                            for w in 0..width {
                                let check_idx = ((d1 + height) * dim2_size + d2 + w) as usize;
                                if mask[check_idx] != face {
                                    break 'height_loop;
                                }
                            }
                            height += 1;
                        }

                        // Mark merged faces as inactive
                        for h in 0..height {
                            for w in 0..width {
                                let clear_idx = ((d1 + h) * dim2_size + d2 + w) as usize;
                                mask[clear_idx].is_active = false;
                            }
                        }

                        // Emit the merged quad
                        let _block = face.block;
                        // Water is no longer here, but we keep the target selection for safety/future use
                        let (target_verts, target_inds) = (&mut vertices, &mut indices);

                        let color = [
                            face.color[0] as f32 / 255.0,
                            face.color[1] as f32 / 255.0,
                            face.color[2] as f32 / 255.0,
                        ];
                        let tex_index = face.tex_index as f32;
                        let roughness = 1.0;
                        let metallic = 0.0;

                        // Convert slice + d1/d2 + width/height to world coordinates
                        let (x0, y0, z0, x1, y1, z1) = match face_dir {
                            0 => {
                                // -X face
                                let x = (base_x + slice) as f32;
                                let y0 = (base_y + d1) as f32;
                                let z0 = (base_z + d2) as f32;
                                let y1 = y0 + height as f32;
                                let z1 = z0 + width as f32;
                                (x, y0, z0, x, y1, z1)
                            }
                            1 => {
                                // +X face
                                let x = (base_x + slice + 1) as f32;
                                let y0 = (base_y + d1) as f32;
                                let z0 = (base_z + d2) as f32;
                                let y1 = y0 + height as f32;
                                let z1 = z0 + width as f32;
                                (x, y0, z0, x, y1, z1)
                            }
                            2 => {
                                // -Y face
                                let y = (base_y + slice) as f32;
                                let x0 = (base_x + d1) as f32;
                                let z0 = (base_z + d2) as f32;
                                let x1 = x0 + height as f32;
                                let z1 = z0 + width as f32;
                                (x0, y, z0, x1, y, z1)
                            }
                            3 => {
                                // +Y face
                                let y = (base_y + slice + 1) as f32;
                                let x0 = (base_x + d1) as f32;
                                let z0 = (base_z + d2) as f32;
                                let x1 = x0 + height as f32;
                                let z1 = z0 + width as f32;
                                (x0, y, z0, x1, y, z1)
                            }
                            4 => {
                                // -Z face
                                let z = (base_z + slice) as f32;
                                let x0 = (base_x + d1) as f32;
                                let y0 = (base_y + d2) as f32;
                                let x1 = x0 + height as f32;
                                let y1 = y0 + width as f32;
                                (x0, y0, z, x1, y1, z)
                            }
                            5 => {
                                // +Z face
                                let z = (base_z + slice + 1) as f32;
                                let x0 = (base_x + d1) as f32;
                                let y0 = (base_y + d2) as f32;
                                let x1 = x0 + height as f32;
                                let y1 = y0 + width as f32;
                                (x0, y0, z, x1, y1, z)
                            }
                            _ => unreachable!(),
                        };

                        // Emit quad with proper vertex order and tiled UVs
                        match face_dir {
                            0 => add_greedy_quad(
                                target_verts,
                                target_inds,
                                [x0, y0, z0],
                                [x0, y0, z1],
                                [x0, y1, z1],
                                [x0, y1, z0],
                                [-1.0, 0.0, 0.0],
                                color,
                                tex_index,
                                roughness,
                                metallic,
                                width as f32,
                                height as f32,
                            ),
                            1 => add_greedy_quad(
                                target_verts,
                                target_inds,
                                [x1, y0, z1],
                                [x1, y0, z0],
                                [x1, y1, z0],
                                [x1, y1, z1],
                                [1.0, 0.0, 0.0],
                                color,
                                tex_index,
                                roughness,
                                metallic,
                                width as f32,
                                height as f32,
                            ),
                            2 => add_greedy_quad(
                                target_verts,
                                target_inds,
                                [x0, y0, z1],
                                [x0, y0, z0],
                                [x1, y0, z0],
                                [x1, y0, z1],
                                [0.0, -1.0, 0.0],
                                color,
                                tex_index,
                                roughness,
                                metallic,
                                width as f32,
                                height as f32,
                            ),
                            3 => add_greedy_quad(
                                target_verts,
                                target_inds,
                                [x0, y1, z0],
                                [x0, y1, z1],
                                [x1, y1, z1],
                                [x1, y1, z0],
                                [0.0, 1.0, 0.0],
                                color,
                                tex_index,
                                roughness,
                                metallic,
                                width as f32,
                                height as f32,
                            ),
                            4 => add_greedy_quad(
                                target_verts,
                                target_inds,
                                [x1, y0, z0],
                                [x0, y0, z0],
                                [x0, y1, z0],
                                [x1, y1, z0],
                                [0.0, 0.0, -1.0],
                                color,
                                tex_index,
                                roughness,
                                metallic,
                                height as f32,
                                width as f32,
                            ),
                            5 => add_greedy_quad(
                                target_verts,
                                target_inds,
                                [x0, y0, z1],
                                [x1, y0, z1],
                                [x1, y1, z1],
                                [x0, y1, z1],
                                [0.0, 0.0, 1.0],
                                color,
                                tex_index,
                                roughness,
                                metallic,
                                height as f32,
                                width as f32,
                            ),
                            _ => {}
                        }

                        d2 += width;
                    }
                }
            }
        }

        ((vertices, indices), (water_vertices, water_indices))
    }
}
