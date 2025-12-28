use crate::vertex::Vertex;

pub fn add_quad(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    v0: [f32; 3],
    v1: [f32; 3],
    v2: [f32; 3],
    v3: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    tex_index: f32,
) {
    let base_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: v0,
        normal,
        color,
        uv: [0.0, 1.0],
        tex_index,
    });
    vertices.push(Vertex {
        position: v1,
        normal,
        color,
        uv: [1.0, 1.0],
        tex_index,
    });
    vertices.push(Vertex {
        position: v2,
        normal,
        color,
        uv: [1.0, 0.0],
        tex_index,
    });
    vertices.push(Vertex {
        position: v3,
        normal,
        color,
        uv: [0.0, 0.0],
        tex_index,
    });
    indices.extend_from_slice(&[
        base_idx,
        base_idx + 1,
        base_idx + 2,
        base_idx,
        base_idx + 2,
        base_idx + 3,
    ]);
}

pub fn build_crosshair() -> (Vec<Vertex>, Vec<u32>) {
    let size = 0.015;
    let thickness = 0.005;
    let color = [1.0, 1.0, 1.0];
    let normal = [0.0, 0.0, 1.0];

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    vertices.push(Vertex {
        position: [-size, -thickness, 0.0],
        normal,
        color,
        uv: [0.0, 0.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [size, -thickness, 0.0],
        normal,
        color,
        uv: [1.0, 0.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [size, thickness, 0.0],
        normal,
        color,
        uv: [1.0, 1.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [-size, thickness, 0.0],
        normal,
        color,
        uv: [0.0, 1.0],
        tex_index: 0.0,
    });
    indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);

    vertices.push(Vertex {
        position: [-thickness, -size, 0.0],
        normal,
        color,
        uv: [0.0, 0.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [thickness, -size, 0.0],
        normal,
        color,
        uv: [1.0, 0.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [thickness, size, 0.0],
        normal,
        color,
        uv: [1.0, 1.0],
        tex_index: 0.0,
    });
    vertices.push(Vertex {
        position: [-thickness, size, 0.0],
        normal,
        color,
        uv: [0.0, 1.0],
        tex_index: 0.0,
    });
    indices.extend_from_slice(&[4, 5, 6, 4, 6, 7]);

    (vertices, indices)
}
