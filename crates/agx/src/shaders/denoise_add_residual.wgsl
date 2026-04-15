// Add the final approximation (residual) to the denoise accumulator.
// After all wavelet levels, accum holds the sum of thresholded details.
// Adding the residual completes the reconstruction.

@group(0) @binding(0) var<storage, read> approx: array<f32>;
@group(0) @binding(1) var<storage, read_write> accum: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    let pixel_count = arrayLength(&approx);
    if idx >= pixel_count { return; }

    accum[idx] = accum[idx] + approx[idx];
}
