use crate::core::block::BlockType;

#[derive(Debug, Clone)]
pub struct Structure {
    pub name: String,
    pub biomes: Vec<String>,
    pub min_height: Option<i32>, // minimum spawn y coordinate
    pub max_height: Option<i32>, // maximum spawn y coordinate
    pub in_water: bool,
    pub min_distance: Option<i32>, // minimum distance between structures
    pub blocks: Vec<(i32, i32, i32, BlockType)>,
}

impl Structure {
    pub fn new(name: &str, biomes: Vec<&str>) -> Self {
        Self {
            name: name.to_string(),
            biomes: biomes.into_iter().map(|s| s.to_owned()).collect(),
            min_height: None,
            max_height: None,
            in_water: false,
            min_distance: Some(10),
            blocks: Vec::new(),
        }
    }

    pub fn with_block(mut self, x: i32, y: i32, z: i32, block: BlockType) -> Self {
        self.blocks.push((x, y, z, block));
        self
    }
}
