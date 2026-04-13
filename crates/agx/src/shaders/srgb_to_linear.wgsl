// sRGB gamma to linear sRGB conversion compute shader.

#import common::color

@group(0) @binding(0) var<storage, read_write> pixels: array<vec3f>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    if idx >= arrayLength(&pixels) { return; }
    pixels[idx] = common::color::srgb_to_linear(pixels[idx]);
}
