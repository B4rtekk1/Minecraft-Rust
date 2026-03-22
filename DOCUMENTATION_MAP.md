# Minerust Documentation - Quick Navigation

## 📚 Documentation Structure

This project includes comprehensive documentation across multiple files. Here's your guide to navigate them:

## 🎯 Start Here

1. **[README.md](README.md)** ⭐ *Main Project Overview*
   - High-level project description
   - Key features and capabilities
   - Performance highlights
   - Getting started instructions

2. **[FOLDER_STRUCTURE.md](FOLDER_STRUCTURE.md)** ⭐ *Complete Folder Guide*
   - Overview of all directories
   - What each folder contains
   - Architecture overview
   - Key dependencies

3. **[DEVELOPMENT.md](DEVELOPMENT.md)** - Development Reference
   - Build instructions
   - Code organization principles
   - Core systems explanation
   - Debugging and optimization tips

## 📁 Module Documentation

Each major module has its own detailed README:

### Core Modules

- **[src/README.md](src/README.md)** - Source code organization
- **[src/app/README.md](src/app/README.md)** - Application logic & main loop
- **[src/core/README.md](src/core/README.md)** - World data structures
- **[src/render/README.md](src/render/README.md)** - GPU rendering pipeline
- **[src/world/README.md](src/world/README.md)** - Procedural generation
- **[src/player/README.md](src/player/README.md)** - Camera & movement
- **[src/multiplayer/README.md](src/multiplayer/README.md)** - Networking

### Supporting Modules

- **[src/ui/README.md](src/ui/README.md)** - User interface system
- **[src/utils/README.md](src/utils/README.md)** - Configuration & utilities
- **[assets/README.md](assets/README.md)** - Game assets guide

## 🗂️ Documentation Map

```
minerust/
├── README.md                          ← Project overview
├── FOLDER_STRUCTURE.md                ← Complete folder guide
├── DEVELOPMENT.md                     ← Development reference
├── DOCUMENTATION_MAP.md               ← This file
│
├── src/README.md                      ← Source code overview
│   ├── app/README.md                  ← Application module
│   ├── core/README.md                 ← Core systems
│   ├── render/README.md               ← Rendering pipeline
│   ├── world/README.md                ← World generation
│   ├── player/README.md               ← Player system
│   ├── multiplayer/README.md          ← Networking
│   ├── ui/README.md                   ← UI system
│   └── utils/README.md                ← Utilities
│
└── assets/README.md                   ← Assets guide
```

## 🔍 Quick Reference by Topic

### If You Want to Understand...

**How the game starts and runs:**
→ Read [src/app/README.md](src/app/README.md)

**How blocks and chunks work:**
→ Read [src/core/README.md](src/core/README.md)

**How rendering works:**
→ Read [src/render/README.md](src/render/README.md)

**How the world is generated:**
→ Read [src/world/README.md](src/world/README.md)

**How the player moves and sees:**
→ Read [src/player/README.md](src/player/README.md)

**How multiplayer works:**
→ Read [src/multiplayer/README.md](src/multiplayer/README.md)

**How UI and menus work:**
→ Read [src/ui/README.md](src/ui/README.md)

**Configuration and settings:**
→ Read [src/utils/README.md](src/utils/README.md)

**All assets and resources:**
→ Read [assets/README.md](assets/README.md)

**Building and development:**
→ Read [DEVELOPMENT.md](DEVELOPMENT.md)

**Complete overview:**
→ Read [FOLDER_STRUCTURE.md](FOLDER_STRUCTURE.md)

## 📊 Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│              Minerust Game Engine                   │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌─────────────────────────────────────────┐       │
│  │         Application Layer (app/)        │       │
│  │  • Main game loop                       │       │
│  │  • Input handling                       │       │
│  │  • Window management                    │       │
│  └──────────────────┬──────────────────────┘       │
│                     │                              │
│  ┌──────────────────▼──────────────────────┐       │
│  │       Game Systems Integration          │       │
│  │                                         │       │
│  ├─────────────────────────────────────┬──┤       │
│  │                                     │  │       │
│  ▼                                     ▼  ▼       │
│ ┌──────────────┐  ┌──────────────┐  ┌────────┐  │
│ │  Rendering  │  │  World Mgmt  │  │ Player │  │
│ │   (render/) │  │  (world/)    │  │(player/)│  │
│ └──────┬───────┘  └──────┬───────┘  └─────┬──┘  │
│        │                 │               │      │
│  ┌─────▼─────────────────▼───────────────▼──┐  │
│  │       Core Data Structures (core/)       │  │
│  │  • Block types                           │  │
│  │  • Chunks                                │  │
│  │  • Biomes                                │  │
│  └──────────────────────────────────────────┘  │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │    Multiplayer Layer (multiplayer/)      │   │
│  │  • Network protocol                      │   │
│  │  • Client/Server communication           │   │
│  └──────────────────────────────────────────┘   │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │     UI & Utils (ui/, utils/)             │   │
│  │  • Menus & HUD                           │   │
│  │  • Settings & configuration              │   │
│  └──────────────────────────────────────────┘   │
│                                                  │
└─────────────────────────────────────────────────┘
           │
           ▼
┌─────────────────────────────────────────────────────┐
│            External Dependencies                   │
├─────────────────────────────────────────────────────┤
│  wgpu (GPU) │ tokio (async) │ quinn (networking) │ │
│  cgmath     │ glyphon       │ fastnoise-lite    │ │
└─────────────────────────────────────────────────────┘
```

## 🚀 Getting Started

1. **First Time?** Start with [README.md](README.md)
2. **Want to build?** See [DEVELOPMENT.md](DEVELOPMENT.md)
3. **Understanding structure?** Read [FOLDER_STRUCTURE.md](FOLDER_STRUCTURE.md)
4. **Specific module?** Find it in the [documentation map above](#-documentation-map)

## 📖 Reading Recommendations

**By Experience Level:**

### Beginners
1. [README.md](README.md) - Overview
2. [FOLDER_STRUCTURE.md](FOLDER_STRUCTURE.md) - Project organization
3. [src/README.md](src/README.md) - Source organization
4. Choose a module of interest

### Intermediate
1. [DEVELOPMENT.md](DEVELOPMENT.md) - Development setup
2. [src/app/README.md](src/app/README.md) - Main loop
3. [src/core/README.md](src/core/README.md) - Data structures
4. [src/render/README.md](src/render/README.md) - Rendering

### Advanced
1. All module READMEs
2. [DEVELOPMENT.md](DEVELOPMENT.md) - Optimization & debugging
3. Source code itself (read the implementations)
4. Performance profiling & benchmarking

## 💡 Key Concepts

### Core Concepts
- **Block** - Single voxel unit (16×16×16 per chunk)
- **Chunk** - 16×16×256 block column
- **SubChunk** - 16×16×16 block section (part of chunk)
- **Biome** - Region with specific generation rules

### Technical Concepts
- **GPU-Driven Rendering** - GPU handles culling and draw commands
- **Frustum Culling** - Skip rendering chunks outside view
- **Mesh Pooling** - Unified buffers for all chunk meshes
- **Async Generation** - Chunks generated on background threads
- **QUIC Protocol** - Modern networking with low latency

### Architecture Patterns
- **Module Separation** - Clear responsibility boundaries
- **Dependency Inversion** - Low coupling between systems
- **Resource Pooling** - Reuse allocations efficiently
- **Lazy Loading** - Load resources only when needed

## 🔧 Common Tasks

### I want to...

**Modify game graphics/rendering**
→ [src/render/README.md](src/render/README.md)

**Change world generation**
→ [src/world/README.md](src/world/README.md)

**Adjust player movement/controls**
→ [src/player/README.md](src/player/README.md)

**Add new UI elements**
→ [src/ui/README.md](src/ui/README.md)

**Change game settings**
→ [src/utils/README.md](src/utils/README.md)

**Debug a problem**
→ [DEVELOPMENT.md](DEVELOPMENT.md)

**Add multiplayer feature**
→ [src/multiplayer/README.md](src/multiplayer/README.md)

**Optimize performance**
→ [DEVELOPMENT.md](DEVELOPMENT.md) - Performance section

## 📚 External Resources

**Rust:**
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)

**Graphics:**
- [wgpu documentation](https://docs.rs/wgpu/)
- [Learn WGSL](https://learn.wgpu.rs/)

**Game Development:**
- [Minecraft Wiki](https://minecraft.fandom.com/)
- [Game Engine Architecture](https://gameenginebook.com/)

**Networking:**
- [QUIC Explained](https://quicwg.org/)
- [Game Network Programming](https://gafferongames.com/)

## 📞 Support

For questions about specific modules, refer to their README files first. They contain:
- Module purpose and responsibilities
- Key types and functions
- Data flow diagrams
- Integration points with other modules
- Performance characteristics
- Common patterns and examples

## 📝 Documentation Maintenance

All documentation files in this project follow these principles:
1. **Clear Structure** - Easy to scan and navigate
2. **Code Examples** - Practical usage patterns
3. **Diagrams** - Visual system relationships
4. **Module Focus** - Each file covers its scope
5. **Cross-References** - Links to related docs

---

**Documentation Last Updated:** 2026-03-05  
**Project Version:** 0.1.0  
**Status:** Active Development

For the latest updates, check the main [README.md](README.md).

