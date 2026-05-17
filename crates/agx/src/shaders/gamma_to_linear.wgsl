// Algorithm: gamma Rec.2020 -> linear Rec.2020 transfer (sign-preserving sRGB curve inverse)
// Canonical explanation: docs/book/src/reference/concepts/color-spaces.md
// CPU equivalent: crate::color_space::srgb_curve_signed_inverse (per channel)
// Bindings: storage pixels
// Entry points: main

#import common::color

@group(0) @binding(0) var<storage, read_write> pixels: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&pixels) / 3u;
    if idx >= pixel_count { return; }
    let base = idx * 3u;
    let rgb = vec3f(pixels[base], pixels[base + 1u], pixels[base + 2u]);
    let result = common::color::gamma_to_linear(rgb);
    pixels[base] = result.x;
    pixels[base + 1u] = result.y;
    pixels[base + 2u] = result.z;
}
