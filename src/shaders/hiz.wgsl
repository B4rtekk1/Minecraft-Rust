@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var dst_tex: texture_storage_2d<r32float, write>;

fn load(pos: vec2<i32>, src_max: vec2<i32>) -> f32 {
    return textureLoad(src_tex, clamp(pos, vec2<i32>(0), src_max), 0).r;
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dst_size = vec2<i32>(textureDimensions(dst_tex));
    let src_size = vec2<i32>(textureDimensions(src_tex));
    let src_max  = src_size - vec2<i32>(1);

    if i32(id.x) >= dst_size.x || i32(id.y) >= dst_size.y {
        return;
    }

    let base = vec2<i32>(id.xy) * 2;

    var d = max(
        max(load(base,                       src_max),
            load(base + vec2<i32>(1, 0),     src_max)),
        max(load(base + vec2<i32>(0, 1),     src_max),
            load(base + vec2<i32>(1, 1),     src_max))
    );

    if (src_size.x & 1) != 0 {
        d = max(d, max(
            load(base + vec2<i32>(2, 0), src_max),
            load(base + vec2<i32>(2, 1), src_max)
        ));
    }
    if (src_size.y & 1) != 0 {
        d = max(d, max(
            load(base + vec2<i32>(0, 2), src_max),
            load(base + vec2<i32>(1, 2), src_max)
        ));
    }
    if (src_size.x & 1) != 0 && (src_size.y & 1) != 0 {
        d = max(d, load(base + vec2<i32>(2, 2), src_max));
    }

    textureStore(dst_tex, vec2<i32>(id.xy), vec4<f32>(d, 0.0, 0.0, 1.0));
}