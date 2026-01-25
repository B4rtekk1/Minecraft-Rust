use image::GenericImageView;

struct Atlas {
    width: u32,
    height: u32,
    tiles_per_row: u32,
    data: Vec<u8>
}

fn load_textures(paths: &[&str], tile_size: u32) -> Vec<Vec<u8>> {
    let mut textures = Vec::new();

    for path in paths {
        let img = image::open(path).expect("failed to open image").to_rgba8();
        assert_eq!(img.width(), tile_size);
        assert_eq!(img.height(), tile_size);

        textures.push(img.into_raw());
    }
    textures
}

fn create_atlas(tile_size: u32, textures: &[Vec<u8>]) -> Atlas {
    let tiles_per_row = (textures.len() as f32).sqrt().ceil() as u32;
    let atlas_size = tiles_per_row * tile_size;

    let mut data = vec![0u8; (atlas_size * atlas_size * 4) as usize];

    for (i, tex) in textures.iter().enumerate() {
        let x = (i as u32 % atlas_size) * tile_size;
        let y = (i as u32 / atlas_size) * tile_size;

        for row in 0..tile_size {
            let dst_start = ((y + row) * atlas_size + x) as usize * 4;
            let src_start = (row * tile_size) as usize * 4;
            data[dst_start..dst_start + (tile_size * 4) as usize]
                .copy_from_slice(&tex[src_start..src_start + (tile_size * 4) as usize]);
        }
    }

    Atlas {
        width: atlas_size,
        height: atlas_size,
        tiles_per_row,
        data
    }
}