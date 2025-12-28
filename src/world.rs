use noise::{NoiseFn, Perlin};
use std::collections::HashMap;

use crate::biome::Biome;
use crate::block::BlockType;
use crate::chunk::Chunk;
use crate::constants::*;
use crate::mesh::add_quad;
use crate::vertex::Vertex;

pub struct World {
    pub chunks: HashMap<(i32, i32), Chunk>,
    perlin_continents: Perlin,
    perlin_terrain: Perlin,
    perlin_detail: Perlin,
    perlin_temperature: Perlin,
    perlin_moisture: Perlin,
    perlin_river: Perlin,
    perlin_lake: Perlin,
    perlin_trees: Perlin,
    perlin_island: Perlin,
    perlin_cave1: Perlin,
    perlin_cave2: Perlin,
    perlin_ore: Perlin,
    perlin_erosion: Perlin,
    pub seed: u32,
}

impl World {
    pub fn new() -> Self {
        Self::new_with_seed(2147)
    }

    pub fn new_with_seed(seed: u32) -> Self {
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
            perlin_cave1: Perlin::new(seed.wrapping_add(9)),
            perlin_cave2: Perlin::new(seed.wrapping_add(10)),
            perlin_ore: Perlin::new(seed.wrapping_add(11)),
            perlin_erosion: Perlin::new(seed.wrapping_add(12)),
            seed,
        };

        let spawn_cx = 0;
        let spawn_cz = 0;
        for cx in (spawn_cx - GENERATION_DISTANCE)..=(spawn_cx + GENERATION_DISTANCE) {
            for cz in (spawn_cz - GENERATION_DISTANCE)..=(spawn_cz + GENERATION_DISTANCE) {
                world.ensure_chunk_generated(cx, cz);
            }
        }

        world.print_nearby_cave_entrances(0, 0, 512);

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

    pub fn get_biome(&self, x: i32, z: i32) -> Biome {
        let scale_continent = 0.002;
        let scale_temp = 0.008;
        let scale_moist = 0.01;
        let scale_river = 0.03;
        let scale_lake = 0.025;

        let continent = self
            .perlin_continents
            .get([x as f64 * scale_continent, z as f64 * scale_continent]);
        let river_noise = self
            .perlin_river
            .get([x as f64 * scale_river, z as f64 * scale_river]);
        let river_value = 1.0 - river_noise.abs() * 1.5;

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

    fn sample_fbm(
        &self,
        perlin: &Perlin,
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
            total += perlin.get([x * frequency, z * frequency]) * amplitude;
            max_value += amplitude;
            amplitude *= persistence;
            frequency *= lacunarity;
        }

        total / max_value
    }

    pub fn get_terrain_height(&self, x: i32, z: i32) -> i32 {
        let biome = self.get_biome(x, z);
        let fx = x as f64;
        let fz = z as f64;

        let continental = self.sample_fbm(&self.perlin_continents, fx, fz, 4, 0.5, 2.0, 0.001);

        let terrain = self.sample_fbm(&self.perlin_terrain, fx, fz, 4, 0.5, 2.0, 0.008);

        let detail = self.sample_fbm(&self.perlin_detail, fx, fz, 3, 0.4, 2.0, 0.015);

        let erosion = self.sample_fbm(&self.perlin_erosion, fx, fz, 2, 0.5, 2.0, 0.005);

        let base_height = match biome {
            Biome::Ocean => {
                let depth = (continental + 1.0) * 0.5 * 15.0 + 35.0;
                depth + detail * 3.0
            }
            Biome::River => (SEA_LEVEL - 3) as f64 + detail * 1.5,
            Biome::Lake => (SEA_LEVEL - 4) as f64 + detail * 2.0,
            Biome::Beach => SEA_LEVEL as f64 + terrain * 2.0 + detail * 1.0,
            Biome::Island => {
                let island_noise = self.perlin_island.get([fx * 0.05, fz * 0.05]);
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
                let dune_noise = self.perlin_detail.get([fx * 0.02, fz * 0.02]);
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
                    &self.perlin_terrain,
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
        };

        (base_height as i32).clamp(1, WORLD_HEIGHT - 20)
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
            self.perlin_cave1
                .get([fx * cave_scale, fy * cave_scale * 0.5, fz * cave_scale]);
        let cave2 = self.perlin_cave2.get([
            fx * cave_scale * 0.7,
            fy * cave_scale * 0.4,
            fz * cave_scale * 0.7,
        ]);

        let cheese_threshold = 0.7;
        let is_cheese_cave = cave1 > cheese_threshold && cave2 > cheese_threshold;
        let spaghetti_scale = 0.08;
        let spag1 = self.perlin_cave1.get([
            fx * spaghetti_scale + 500.0,
            fy * spaghetti_scale,
            fz * spaghetti_scale,
        ]);
        let spag2 = self.perlin_cave2.get([
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
        let entrance_noise = self.perlin_cave1.get([
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
                self.perlin_cave1
                    .get([fx * cave_scale, fy * cave_scale * 0.5, fz * cave_scale]);
            let cave2 = self.perlin_cave2.get([
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
            .perlin_ore
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

        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let world_x = base_x + lx;
                let world_z = base_z + lz;
                let biome = self.get_biome(world_x, world_z);
                let height = self.get_terrain_height(world_x, world_z);

                for y in 0..WORLD_HEIGHT.min(height + 10) {
                    let block = self.get_block_for_biome(biome, y, height, world_x, world_z);
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

        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let world_x = base_x + lx;
                let world_z = base_z + lz;
                let height = self.get_terrain_height(world_x, world_z);

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
                if depth_from_surface > 6 {
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
                    // Snow-capped peaks
                    if y == surface_height - 1 {
                        BlockType::Snow
                    } else if depth_from_surface > 0 {
                        BlockType::Stone
                    } else {
                        BlockType::Air
                    }
                } else if y > 100 {
                    // High altitude - mostly stone with some grass
                    if depth_from_surface > 2 {
                        BlockType::Stone
                    } else if y == surface_height - 1 {
                        BlockType::Grass
                    } else if depth_from_surface > 0 {
                        BlockType::Stone
                    } else {
                        BlockType::Air
                    }
                } else {
                    // Lower mountains - normal dirt/grass
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
                    .perlin_trees
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
                            if chunk.get_block(nlx, nly, nlz) == BlockType::Air {
                                chunk.set_block(nlx, nly, nlz, BlockType::Leaves);
                            }
                        }
                    }
                }
            }
        } else {
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

        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let world_x = base_x + lx;
                let world_z = base_z + lz;

                let biome = self.get_biome(world_x, world_z);

                for ly in 0..SUBCHUNK_HEIGHT {
                    let world_y = base_y + ly;

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
