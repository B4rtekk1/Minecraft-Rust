# Render3D - Voxel Rendering Engine

Render3D is a high-performance voxel rendering engine and sandbox game built in Rust using the `wgpu` graphics API. It features an infinite procedurally generated world, advanced shaders, dynamic day/night cycle, and a robust chunk management system.

## Key Features

### World Generation

- **Infinite Procedural Terrain:** Powered by multi-layered **FBM (Fractional Brownian Motion)** Perlin noise for realistic height variations.
- **11 Distinct Biomes:** Including Plains, Forests, Deserts, Tundra, Mountains, Swamps, Oceans, Beaches, Rivers, Lakes, and Islands.
- **Underground Systems:**
  - **Caves:** Complex 3D networks using "Cheese" and "Spaghetti" noise patterns.
  - **Ores:** Deterministic ore vein generation (Coal, Iron, Gold, Diamond) at varying depths.
- **Flora:** Procedurally placed trees (standard and large variants) and desert vegetation.

### Advanced Rendering

- **Dynamic Shadow Mapping:** Real-time sun-cast shadows with **PCF (Percentage Closer Filtering)** using Vogel disk sampling for smooth, stable shadow edges.
- **Shadow Map Stabilization:** Texel-snapping algorithm prevents shadow swimming/flickering during camera movement.
- **Day/Night Cycle:**
  - Full 360-degree sun rotation creating realistic sunrise, noon, sunset, and night.
  - Dynamic sky color transitions (blue day, orange sunset, dark night).
  - Distance-based visibility system with linear interpolation between day (250 blocks) and night (12 blocks) visibility range.
  - Dynamic ambient lighting that adjusts based on sun position.
- **Water Shader:**
  - **Reflections & Fresnel:** Realistic surface reflections based on viewing angle.
  - **Wave Animations:** Vertex-displaced wave patterns.
  - **Specularity:** Sun and moon highlights with shadow masking.
- **Atmospheric Effects:** Distance fog that blends to sky color during day and to black during night.
- **Frustum Culling:** High-performance AABB-based culling to only render visible subchunks.

### Gameplay & Mechanics

- **Interaction:** Real-time block breaking (with visual progress bar) and block placement using precise raycasting.
- **Physics Engine:** Momentum-based movement, gravity, and player-centered AABB collisions.
- **Fluid Physics:** Buoyancy and swimming mechanics when in water.
- **HUD:** Real-time FPS counter, coordinates (X, Y, Z), and digging progress bar.

### Persistence

- **Optimized Save System:** Custom `.r3d` binary format using **Bincode**.
- **Smart Saving:** Only chunks modified by the player are saved, keeping world files small and efficient.

## Tech Stack

- **Language:** Rust (Edition 2024)
- **Graphics API:** [wgpu](https://wgpu.rs/) (Vulkan, DX12, Metal, WebGPU)
- **Shaders:** WGSL (WebGPU Shading Language)
- **Windowing:** [winit](https://github.com/rust-windowing/winit)
- **Math:** [cgmath](https://github.com/rustgd/cgmath)
- **Serialization:** [serde](https://serde.rs/) & [bincode](https://github.com/bincode-org/bincode)
- **Noise:** [noise-rs](https://github.com/Rye-Dream/noise-rs)
- **Text Rendering:** [wgpu_glyph](https://github.com/hecrj/wgpu_glyph)

## Getting Started

### Prerequisites

- [Rust Toolchain](https://www.rust-lang.org/tools/install) (1.75.0+)
- A graphics driver compatible with Vulkan, DirectX 12, or Metal.

### Installation & Run

1. **Clone the repository:**

    ```bash
    git clone https://github.com/B4rtekk1/Minecraft-Rust.git
    cd Minecraft-Rust
    ```

2. **Run in Release Mode:**

    ```bash
    cargo run --release
    ```

## Controls

### Movement

- **W / A / S / D:** Move Forward / Left / Backward / Right
- **Space:** Jump / Swim up
- **Left Shift:** Sprint / Sink in water
- **F11:** Toggle Fullscreen

### Interaction

- **Mouse Movement:** Look around
- **Left Click:** Break block
- **Right Click:** Place block
- **Escape:** Release mouse / Pause

## Project Structure

```
render3d/
├── src/
│   ├── main.rs          # Application entry, rendering pipeline, input handling
│   ├── world.rs         # World generation, chunk management, mesh building
│   ├── save.rs          # World persistence (save/load)
│   ├── uniforms.rs      # GPU uniform buffer definitions
│   └── shaders/
│       ├── terrain.wgsl # Terrain rendering with shadows and day/night
│       ├── water.wgsl   # Water rendering with reflections
│       ├── shadow.wgsl  # Shadow map generation
│       ├── sun.wgsl     # Sun billboard rendering
│       └── ui.wgsl      # UI elements (crosshair, coordinates)
└── assets/
    └── textures.png     # Texture atlas
```

## License

This project is open-source and available under the MIT License.
