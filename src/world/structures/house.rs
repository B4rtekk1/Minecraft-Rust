use super::Structure;
use crate::core::block::BlockType;

#[derive(Debug, Clone)]
pub struct House {
    pub structure: Structure,
}

impl House {
    pub fn new() -> Self {
        let mut structure = Structure::new("House", vec!["Plains", "Forest", "Desert"]);

        // Foundation (Stone) - 5x5 platform at y=0
        for x in 0..5 {
            for z in 0..5 {
                structure.blocks.push((x, 0, z, BlockType::Stone));
            }
        }

        // Walls (Wood) - 5x5 ring at y=1..=3
        for y in 1..=3 {
            for x in 0..5 {
                for z in 0..5 {
                    // Only outer ring
                    if x == 0 || x == 4 || z == 0 || z == 4 {
                        structure.blocks.push((x, y, z, BlockType::Wood));
                    } else {
                        // Interior air to clear any potential existing blocks
                        structure.blocks.push((x, y, z, BlockType::Air));
                    }
                }
            }
        }

        // Roof (WoodStairs) - Pyramid style
        // Level 4 (Overhangs slightly? Let's just do 5x5 for now as ceiling/base of roof)
        for x in 0..5 {
            for z in 0..5 {
                structure.blocks.push((x, 4, z, BlockType::WoodStairs));
            }
        }

        // Level 5 - 3x3
        for x in 1..4 {
            for z in 1..4 {
                structure.blocks.push((x, 5, z, BlockType::WoodStairs));
            }
        }

        // Level 6 - 1x1 peak
        structure.blocks.push((2, 6, 2, BlockType::WoodStairs));

        // Door (Air) - Front wall (z=0) at x=2
        structure.blocks.push((2, 1, 0, BlockType::Air));
        structure.blocks.push((2, 2, 0, BlockType::Air));

        // Windows (Glass/Air)
        // Left wall (x=0)
        structure.blocks.push((0, 2, 2, BlockType::Air));
        // Right wall (x=4)
        structure.blocks.push((4, 2, 2, BlockType::Air));
        // Back wall (z=4)
        structure.blocks.push((2, 2, 4, BlockType::Air));

        Self { structure }
    }
}
