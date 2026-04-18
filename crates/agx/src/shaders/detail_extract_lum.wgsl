// Extract luminance from RGB pixel buffer into single-channel luminance buffer.

@group(0) @binding(0) var<storage, read> pixels: array<f32>;
@group(0) @binding(1) var<storage, read_write> lum: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&lum);
    if idx >= pixel_count { return; }

    let base = idx * 3u;
    let r = pixels[base];
    let g = pixels[base + 1u];
    let b = pixels[base + 2u];

    // Rec. 709 luminance
    lum[idx] = 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
