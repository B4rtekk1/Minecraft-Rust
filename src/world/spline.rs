//! Spline-based terrain interpolation for Minecraft 1.18+ style generation
//!
//! Uses Catmull-Rom splines for smooth height transitions between biome types.

/// A point on a terrain spline
#[derive(Clone, Copy)]
struct SplinePoint {
    /// Input parameter value (e.g., continentalness from -1 to 1)
    input: f64,
    /// Output height value
    output: f64,
}

/// Terrain height spline for smooth interpolation
pub struct TerrainSpline {
    points: Vec<SplinePoint>,
}

impl TerrainSpline {
    /// Create a new spline from input-output pairs
    pub fn new(pairs: &[(f64, f64)]) -> Self {
        let points = pairs
            .iter()
            .map(|(i, o)| SplinePoint {
                input: *i,
                output: *o,
            })
            .collect();
        Self { points }
    }

    /// Sample the spline at a given input value using Catmull-Rom interpolation
    pub fn sample(&self, t: f64) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }
        if self.points.len() == 1 {
            return self.points[0].output;
        }

        // Clamp to bounds
        let first = self.points.first().unwrap();
        let last = self.points.last().unwrap();

        if t <= first.input {
            return first.output;
        }
        if t >= last.input {
            return last.output;
        }

        // Find the segment containing t
        let mut i = 0;
        while i < self.points.len() - 1 && self.points[i + 1].input < t {
            i += 1;
        }

        // Get 4 points for Catmull-Rom (duplicate endpoints if needed)
        let p0 = if i > 0 {
            &self.points[i - 1]
        } else {
            &self.points[i]
        };
        let p1 = &self.points[i];
        let p2 = &self.points[i + 1];
        let p3 = if i + 2 < self.points.len() {
            &self.points[i + 2]
        } else {
            &self.points[i + 1]
        };

        // Normalize t to [0, 1] within segment
        let segment_t = (t - p1.input) / (p2.input - p1.input);

        // Catmull-Rom interpolation
        catmull_rom(p0.output, p1.output, p2.output, p3.output, segment_t)
    }

    /// Continental spline: maps continentalness (-1 to 1) to base terrain height
    /// Minecraft-style ocean-to-mountain transition
    pub fn continental() -> Self {
        Self::new(&[
            (-1.05, 25.0), // Deep ocean floor
            (-0.5, 40.0),  // Ocean
            (-0.2, 58.0),  // Coast/Beach
            (-0.1, 62.0),  // Shore
            (0.0, 68.0),   // Lowlands (sea level + some)
            (0.2, 76.0),   // Plains
            (0.4, 90.0),   // Hills
            (0.6, 120.0),  // Highlands
            (0.8, 160.0),  // Mountains
            (1.0, 200.0),  // Extreme mountains
        ])
    }

    /// Erosion spline: multiplier for terrain roughness
    /// High erosion = smoother, low erosion = rougher
    pub fn erosion() -> Self {
        Self::new(&[
            (-1.0, 1.5), // Very rough (deep canyons)
            (-0.5, 1.2), // Rough
            (0.0, 1.0),  // Normal
            (0.5, 0.6),  // Smooth
            (1.0, 0.3),  // Very smooth (flat plains)
        ])
    }

    /// Peaks/valleys spline: height offset for dramatic features
    pub fn peaks_valleys() -> Self {
        Self::new(&[
            (-1.0, -40.0), // Deep valley
            (-0.5, -15.0), // Shallow valley
            (0.0, 0.0),    // Flat
            (0.5, 25.0),   // Hill
            (1.0, 80.0),   // Sharp peak
        ])
    }
}

/// Catmull-Rom spline interpolation
fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;

    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

/// Blend between two values based on a noise value
pub fn blend_heights(h1: f64, h2: f64, blend_factor: f64) -> f64 {
    h1 * (1.0 - blend_factor) + h2 * blend_factor
}

/// Calculate biome blend weight from distance to biome edge
pub fn biome_blend_weight(noise_value: f64, threshold: f64, blend_width: f64) -> f64 {
    let dist_from_edge = (noise_value - threshold).abs();
    if dist_from_edge >= blend_width {
        1.0
    } else {
        dist_from_edge / blend_width
    }
}
