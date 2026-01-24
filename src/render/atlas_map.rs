use crate::core::block::BlockType;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct AtlasMapConfig {
    pub blocks: HashMap<String, BlockTextures>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BlockTextures {
    pub all: Option<u32>,
    pub top: Option<u32>,
    pub side: Option<u32>,
    pub bottom: Option<u32>,
    pub parts: Option<Vec<ModelPart>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ModelPart {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub textures: PartTextures,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct PartTextures {
    pub all: Option<u32>,
    pub top: Option<u32>,
    pub side: Option<u32>,
    pub bottom: Option<u32>,
    pub front: Option<u32>,
    pub back: Option<u32>,
    pub left: Option<u32>,
    pub right: Option<u32>,
}

pub enum FaceType {
    Top,
    Bottom,
    Side,
}

pub struct AtlasMap {
    config: AtlasMapConfig,
}

impl AtlasMap {
    pub fn new() -> Self {
        let path = "assets/atlas_map_structure.json";
        let content =
            std::fs::read_to_string(path).unwrap_or_else(|_| "{\"blocks\":{}}".to_string());

        let config: AtlasMapConfig = serde_json::from_str(&content).unwrap_or_else(|e| {
            println!("Failed to parse atlas map: {}", e);
            AtlasMapConfig {
                blocks: HashMap::new(),
            }
        });

        Self { config }
    }

    pub fn get_texture(&self, block: BlockType, face: FaceType) -> f32 {
        let key = format!("{:?}", block);
        if let Some(textures) = self.config.blocks.get(&key) {
            let tex_id = match face {
                FaceType::Top => textures.top.or(textures.all),
                FaceType::Bottom => textures.bottom.or(textures.all),
                FaceType::Side => textures.side.or(textures.all),
            };

            if let Some(id) = tex_id {
                return id as f32;
            }
        }

        // Fallback to hardcoded defaults if not in JSON (to prevent breaking everything immediately)
        // or just return 0.0
        // Ideally we should use the existing hardcoded logic as fallback
        // But importing BlockType and calling methods might be recursive if we change BlockType.
        // For now, we rely on the JSON being correct or valid.
        // Or we can call the block's inherent methods?
        // No, we are replacing the call site.

        // Let's use the old hardcoded logic as fallback so the game works even if JSON is partial
        match face {
            FaceType::Top => block.tex_top(),
            FaceType::Bottom => block.tex_bottom(),
            FaceType::Side => block.tex_side(),
        }
    }
    pub fn get_multipart(&self, block: BlockType) -> Option<&Vec<ModelPart>> {
        let key = format!("{:?}", block);
        self.config.blocks.get(&key).and_then(|b| b.parts.as_ref())
    }
}
