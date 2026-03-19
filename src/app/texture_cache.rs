use std::fs;
use std::path::Path;

use minerust::{ATLAS_SIZE, TEXTURE_SIZE, generate_texture_atlas, load_texture_atlas_from_file};

/// A simple file-backed cache for the raw texture atlas pixel data.
///
/// The cache stores the flat RGBA8 byte array produced by either
/// [`generate_texture_atlas`] or [`load_texture_atlas_from_file`].  No size
/// header is embedded; the caller is responsible for knowing the atlas
/// dimensions (they are fixed by the application constants).
struct TextureAtlasCache {
    /// Filesystem path to the cache file.
    cache_path: String,
}

impl TextureAtlasCache {
    /// Creates a `TextureAtlasCache` that reads from and writes to `cache_path`.
    fn new(cache_path: &str) -> Self {
        Self {
            cache_path: cache_path.to_string(),
        }
    }

    /// Returns `true` if the cache file exists on disk.
    fn exists(&self) -> bool {
        Path::new(&self.cache_path).exists()
    }

    /// Reads the cache file and returns its contents, or `None` on I/O error.
    fn load(&self) -> Option<Vec<u8>> {
        fs::read(&self.cache_path).ok()
    }
}

/// Generates all mip levels for a 16-layer `Texture2DArray` atlas.
///
/// Starting from `atlas_data` (mip 0), each subsequent level is produced by
/// downsampling every layer independently with a bilinear (Triangle) filter
/// until both dimensions reach 1×1.  The total number of levels is
/// `floor(log2(max(width, height))) + 1`.
///
/// # Arguments
/// * `atlas_data`   – Raw RGBA8 pixel data for all 16 layers at mip 0, in
///   layer-major order (layer `i` at byte offset `i * width * height * 4`).
/// * `atlas_width`  – Width of a single layer in pixels at mip 0.
/// * `atlas_height` – Height of a single layer in pixels at mip 0.
///
/// # Returns
/// A `Vec` with one entry per mip level, each containing the concatenated RGBA8
/// data for all 16 layers at that level, in the same layer-major layout.
///
/// # Panics
/// Panics if `atlas_data` is too short to cover `width * height * 4 * 16` bytes
/// at mip 0 (surfaces a bug in the caller's size calculation).
pub fn generate_texture_atlas_with_mipmaps(
    atlas_data: &[u8],
    atlas_width: u32,
    atlas_height: u32,
) -> Vec<Vec<u8>> {
    let mip_level_count = (atlas_width.max(atlas_height) as f32).log2().floor() as u32 + 1;
    let mut mip_levels = Vec::with_capacity(mip_level_count as usize);

    // Mip 0 is the original data passed in.
    mip_levels.push(atlas_data.to_vec());

    for level in 1..mip_level_count {
        let src_level = level - 1;
        // Each dimension halves per level, but never goes below 1.
        let src_width  = (atlas_width  >> src_level).max(1);
        let src_height = (atlas_height >> src_level).max(1);
        let dst_width  = (atlas_width  >> level).max(1);
        let dst_height = (atlas_height >> level).max(1);

        // Pre-allocate for all 16 layers at this mip level.
        let mut level_data = Vec::with_capacity((dst_width * dst_height * 4 * 16) as usize);

        for layer in 0..16usize {
            let layer_size   = (src_width * src_height * 4) as usize;
            let layer_offset = layer * layer_size;
            let src_data     = &mip_levels[src_level as usize];
            let layer_pixels = &src_data[layer_offset..layer_offset + layer_size];

            // Wrap the raw pixels in an RgbaImage for the `imageops` resizer.
            let img = image::RgbaImage::from_raw(src_width, src_height, layer_pixels.to_vec())
                .expect("Failed to create image from mipmap level");

            // Triangle (bilinear) filter gives acceptable quality at low cost.
            let resized = image::imageops::resize(
                &img,
                dst_width,
                dst_height,
                image::imageops::FilterType::Triangle,
            );
            level_data.extend_from_slice(&resized.into_raw());
        }

        mip_levels.push(level_data);
    }

    mip_levels
}

/// Creates a mipmapped 16-layer `Texture2DArray` on the GPU from raw atlas data.
///
/// All mip levels are generated on the CPU via
/// [`generate_texture_atlas_with_mipmaps`] and uploaded in a single batch of
/// `queue.write_texture` calls.  The resulting texture uses the
/// `Rgba8UnormSrgb` format.
///
/// # Arguments
/// * `device`       – wgpu device used to allocate the texture.
/// * `queue`        – wgpu queue used to upload pixel data.
/// * `atlas_data`   – Raw RGBA8 pixel data for all 16 layers (layer-major).
/// * `atlas_width`  – Width of one layer at mip 0 in pixels.
/// * `atlas_height` – Height of one layer at mip 0 in pixels.
///
/// # Returns
/// A `(Texture, TextureView)` pair.  The view is configured as `D2Array` so
/// the WGSL shader can index individual layers with `texture_2d_array<f32>`.
pub fn create_texture_atlas_optimized(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    atlas_data: &[u8],
    atlas_width: u32,
    atlas_height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let mip_level_count = (atlas_width.max(atlas_height) as f32).log2().floor() as u32 + 1;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Texture Atlas"),
        size: wgpu::Extent3d {
            width: atlas_width,
            height: atlas_height,
            depth_or_array_layers: 16,
        },
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let mip_levels = generate_texture_atlas_with_mipmaps(atlas_data, atlas_width, atlas_height);

    // Upload every mip level.  All 16 layers are written in one call per level
    // because `write_texture` accepts a 3-D extent where the Z dimension maps
    // to array layers.
    for (level, level_data) in mip_levels.iter().enumerate() {
        let mip_width  = (atlas_width  >> level).max(1);
        let mip_height = (atlas_height >> level).max(1);

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            level_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * mip_width),
                rows_per_image: Some(mip_height),
            },
            wgpu::Extent3d {
                width: mip_width,
                height: mip_height,
                depth_or_array_layers: 16,
            },
        );
    }

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some("Texture Atlas View"),
        // D2Array allows shaders to index individual texture layers.
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });

    (texture, view)
}

/// Loads or generates the texture atlas and uploads it to the GPU.
///
/// The source of atlas data is selected in priority order:
///
/// 1. **Cache file** (`assets/texture_atlas.cache`) – raw RGBA8 bytes written
///    by a previous run.  Fastest path; skips PNG decoding and procedural
///    generation.
/// 2. **PNG file** (`assets/textures.png`) – a 4×4 grid atlas image whose real
///    pixel dimensions are read from the file header.
/// 3. **Procedural fallback** – calls [`generate_texture_atlas`] at runtime.
///    Used when neither file is present or readable.
///
/// # Returns
/// `(texture, view, tile_width, tile_height)` where `tile_width` and
/// `tile_height` are the dimensions of a single tile within the atlas (not the
/// full atlas dimensions), so callers can compute UV offsets correctly.
///
/// # Bug fix
/// The previous cache branch returned `(cached_data, TEXTURE_SIZE, TEXTURE_SIZE)`
/// where `TEXTURE_SIZE` is the *tile* size.  The atlas side length is
/// `TEXTURE_SIZE * ATLAS_SIZE`, so the wrong dimensions were being forwarded to
/// `create_texture_atlas_optimized`, resulting in a mis-sized GPU texture and
/// incorrect mip chain.  The cache branch now passes the correct computed atlas
/// dimensions.
pub fn load_or_generate_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> (wgpu::Texture, wgpu::TextureView, u32, u32) {
    let cache = TextureAtlasCache::new("assets/texture_atlas.cache");

    // Correct atlas side length: TEXTURE_SIZE pixels per tile × ATLAS_SIZE tiles per side.
    let atlas_side = TEXTURE_SIZE * ATLAS_SIZE;

    let (atlas_data, atlas_width, atlas_height) = if cache.exists() {
        match cache.load() {
            Some(cached_data) => {
                tracing::info!(
                    "Loaded texture atlas from cache ({} bytes)",
                    cached_data.len()
                );
                // BUG FIX: was (cached_data, TEXTURE_SIZE, TEXTURE_SIZE).
                // TEXTURE_SIZE is the tile size, not the full atlas size.
                // The full atlas is atlas_side × atlas_side pixels.
                (cached_data, atlas_side, atlas_side)
            }
            None => {
                // Cache file exists but could not be read; fall back to procedural.
                let data = generate_texture_atlas();
                (data, atlas_side, atlas_side)
            }
        }
    } else {
        match load_texture_atlas_from_file("assets/textures.png") {
            Ok((data, width, height)) => {
                tracing::info!("Loaded texture atlas from PNG: {}x{}", width, height);
                // PNG branch: use the actual dimensions reported by the image decoder.
                (data, width, height)
            }
            Err(e) => {
                tracing::warn!("Failed to load texture atlas from PNG: {}", e);
                let data = generate_texture_atlas();
                (data, atlas_side, atlas_side)
            }
        }
    };

    let (texture, view) =
        create_texture_atlas_optimized(device, queue, &atlas_data, atlas_width, atlas_height);

    // Return tile dimensions (not atlas dimensions) so callers can compute
    // per-tile UV offsets without needing to know ATLAS_SIZE themselves.
    (texture, view, TEXTURE_SIZE, TEXTURE_SIZE)
}