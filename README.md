# Render3D - Voxel Rendering Engine

Render3D is a high-performance voxel rendering engine and sandbox game built in Rust using the `wgpu` graphics API. It features an infinite procedurally generated world, directional lighting, and a robust chunk management system.

## Key Features

- **Procedural World Generation:** Infinite terrain generation using multi-layered Perlin noise, featuring distinct biomes including Plains, Forests, Deserts, Tundra, Mountains, and Oceans.
- **High-Performance Rendering:** Built on `wgpu` for cross-platform hardware acceleration. Utilizes frustum culling and efficient mesh generation to maintain high frame rates.
- **Advanced Shaders:** Custom WGSL shaders implementing:
  - Dynamic water with reflections, Fresnel effects, and wave animation.
  - Distance fog and atmospheric scattering.
  - Per-face directional lighting.
- **Voxel interactions:** Real-time block breaking and placing mechanics with raycasting.
- **Physics Engine:** Player collision detection, gravity, and momentum-based movement.

## Tech Stack

- **Language:** Rust (Edition 2024)
- **Graphics API:** wgpu (WebGPU for native)
- **Windowing:** winit
- **Texture Management:** image crate with raw texture atlas support
- **Math:** cgmath

## Prerequisites

- [Rust Toolchain](https://www.rust-lang.org/tools/install) (1.75.0 or newer recommended)
- A graphics driver compatible with Vulkan, DirectX 12, Metal, or OpenGL.

## Installation & Building

1. **Clone the repository:**

    ```bash
    git clone https://github.com/B4rtekk1/Minecraft-Rust.git
    cd Minecraft-Rust
    ```

2. **Build the project:**
    For best performance, compile in release mode:

    ```bash
    cargo build --release
    ```

3. **Run the application:**

    ```bash
    cargo run --release
    ```

## Usage & Controls

Once the application is running, the following controls are available:

### Movement

- **W / A / S / D:** Move Forward / Left / Backward / Right
- **Space:** Jump
- **Left Shift:** Sprint

### Interaction

- **Mouse Movement:** Look around
- **Left Click:** Break highlighted block
- **Right Click:** Place block (Stone)
- **Escape:** Release mouse cursor / Pause
- **F11:** Toggle Fullscreen

### Troubleshooting

**Texture Loading Issues:**
The application automatically looks for `textures.png` or `textures.jpg` in the `assets/` directory. If run from a different working directory, it attempts to fall back to looking in the current folder. Ensure your assets are correctly placed.

## License

This project is open-source and available under the MIT License.
