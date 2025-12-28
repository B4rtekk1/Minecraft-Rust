#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Biome {
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
    pub fn grass_color(&self) -> [f32; 3] {
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

    pub fn leaves_color(&self) -> [f32; 3] {
        match self {
            Biome::Plains => [0.35, 0.65, 0.25],
            Biome::Forest => [0.20, 0.50, 0.15],
            Biome::Tundra => [0.30, 0.45, 0.35],
            Biome::Swamp => [0.30, 0.45, 0.20],
            Biome::Island => [0.35, 0.60, 0.25],
            _ => [0.30, 0.60, 0.20],
        }
    }

    pub fn tree_density(&self) -> f64 {
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

    pub fn has_trees(&self) -> bool {
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
