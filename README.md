# Render3D

Render3D is a high-performance voxel rendering engine and sandbox game drafted in Rust. It utilizes the **wgpu** graphics API to deliver modern, cross-platform hardware acceleration (Vulkan, DirectX 12, Metal). The engine is engineered for scalability, featuring an infinite procedurally generated world, a fully GPU-driven rendering pipeline, and a robust client-server network architecture.

## Table of Contents

- [Features](#features)
- [Technical Architecture](#technical-architecture)
- [Performance](#performance)
- [Getting Started](#getting-started)
- [Configuration](#configuration)
- [Controls](#controls)
- [Roadmap](#roadmap)
- [License](#license)

## Features

### Rendering Engine

- **Graphics API**: Built on [wgpu](https://wgpu.rs/), ensuring compatibility with all modern GPU backends.
- **GPU-Driven Rendering**:
  - **Indirect Drawing**: Geometry is batched into unified vertex/index buffers (~560MB/256MB capacity) to minimize CPU draw call overhead.
  - **Compute Culling**: A compute shader pre-pass aggressively culls invisible subchunks against the view frustum before the draw pass.
- **Advanced Lighting**:
  - **Cascaded Shadow Maps (CSM)**: 4-cascade shadow system (up to 2048x2048 resolution) with Percentage-Closer Filtering (PCF) for soft, stable shadows.
  - **SSAO**: Screen Space Ambient Occlusion for realistic depth and occlusion simulation.
  - **Atmospheric Scattering**: Physically based day/night cycle with dynamic sun/moon positioning and fog integration.
- **Visual Effects**:
  - **Water**: Vertex displacement waves, Fresnel reflections, and specular highlights.
  - **Bloom & God Rays**: Procedural sun rendering with limb darkening and diffraction spikes.

### World Simulation

- **Infinite Procedural Generation**: Multi-threaded generation using Fractional Brownian Motion (FBM) noise.
- **Biomes**: 11 distinct biomes including Plains, Mountains, Deserts, Swamps, and Oceans.
- **Voxel Systems**:
  - 256-block world height limit.
  - Complex cave systems ("Cheese" and "Spaghetti" noise).
  - Deterministic ore generation.

### Multiplayer

- **Architecture**: Authoritative server with predicted client movement.
- **Networking**: Hybrid protocol stack using QUIC (via `quinn`) for reliable data and UDP for high-frequency updates.
- **State Sync**: Efficient delta compression for chunk data and entity states.

## Technical Architecture

Render3D moves away from traditional CPU-bound rendering loops.

1. **Unified Geometry Buffers**: Instead of allocating a vertex buffer per chunk, the engine uses massive pre-allocated buffers (`MAX_VERTICES` = 10M). Chunks are sub-allocated regions within these buffers.
2. **Indirect Dispatch**: The CPU does not issue `draw` commands for each chunk. Instead, it maintains a buffer of draw arguments. A compute shader filters these arguments based on visibility, and a single `draw_indirect` call renders the entire terrain.
3. **Greedy Meshing**: Adjacent blocks of the same type are merged into single quads during mesh generation. This significantly reduces triangle count and memory pressure.

## Performance

The engine is optimized for high-end rendering loads:

- **Zero-Copy Mesh Uploads**: Mesh generation occurs on worker threads (Rayon thread pool), and data is mapped directly to staging buffers.
- **Asynchronous Chunk Loading**: Generation and meshing are decoupled from the render loop, preventing frame drops during travel.
- **Frustum Culling**:
  - **CPU Level**: coarse AABB checks for load prioritization.
  - **GPU Level**: exact compute shader culling for draw command generation.

## Getting Started

### Prerequisites

- **Rust**: Latest stable toolchain (install via [rustup.rs](https://rustup.rs/)).
- **GPU Drivers**: Must support Vulkan 1.2+, DirectX 12, or Metal.

### Installation

```bash
# Clone the repository
git clone https://github.com/B4rtekk1/Minecraft-Rust.git
cd Minecraft-Rust

# Build in release mode (Essential for performance)
cargo build --release
```

### Running

```bash
cargo run --release
```

## Configuration

Core engine parameters are defined in `src/constants.rs` and can be modified at compile time:

| Constant | Default | Description |
|----------|---------|-------------|
| `RENDER_DISTANCE` | 10 | Radius of chunks to render. |
| `WORLD_HEIGHT` | 256 | Maximum vertical build limit. |
| `CSM_CASCADE_COUNT`| 4 | Number of shadow cascades. |
| `CSM_SHADOW_MAP_SIZE`| 2048 | Resolution of shadow textures. |
| `MAX_CHUNKS_PER_FRAME` | 4 | Limit on chunk uploads per frame to prevent stutter. |

## Controls

| Key | Action |
|-----|--------|
| **W, A, S, D** | Movement |
| **Space** | Jump / Swim Up |
| **Shift** | Sprint / Swim Down |
| **Mouse** | Look Camera |
| **LMB** | Break Block |
| **RMB** | Place Block |
| **F11** | Toggle Fullscreen |
| **Esc** | Pause / Release Cursor |

## Roadmap

- [ ] **Dynamic Light sources**: Torch and lantern support with propagation.
- [ ] **Entity System**: Passive mobs and enemy AI.
- [ ] **Inventory UI**: Drag-and-drop item management.
- [ ] **Modding API**: Wasm-based plugin system.
- [ ] **Post-Processing**: TAA (Temporal Anti-Aliasing) and Motion Blur.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
