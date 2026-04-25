// Algorithm: Linear-to-sRGB conversion pass for display-referred encoding
// Canonical explanation: docs/book/src/reference/concepts/color-spaces.md
// CPU equivalent: crates/agx/src/adjust/mod.rs (linear_to_srgb)
// Bindings: storage pixels
// Entry points: main

// Linear sRGB to sRGB gamma conversion compute shader.

#import common::color

@group(0) @binding(0) var<storage, read_write> pixels: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&pixels) / 3u;
    if idx >= pixel_count { return; }
    let base = idx * 3u;
    let rgb = vec3f(pixels[base], pixels[base + 1u], pixels[base + 2u]);
    let result = common::color::linear_to_srgb(rgb);
    pixels[base] = result.x;
    pixels[base + 1u] = result.y;
    pixels[base + 2u] = result.z;
}
