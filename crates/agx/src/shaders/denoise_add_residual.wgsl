// Algorithm: Noise reduction reconstruction pass adding the final approximation residual
// Canonical explanation: crates/agx/src/adjust/denoise.md
// CPU equivalent: crates/agx/src/adjust/denoise.rs (apply_noise_reduction)
// Bindings: storage approx/accum
// Entry points: main

// Add the final approximation (residual) to the denoise accumulator.
// After all wavelet levels, accum holds the sum of thresholded details.
// Adding the residual completes the reconstruction.

@group(0) @binding(0) var<storage, read> approx: array<f32>;
@group(0) @binding(1) var<storage, read_write> accum: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&approx);
    if idx >= pixel_count { return; }

    accum[idx] = accum[idx] + approx[idx];
}
