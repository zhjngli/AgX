// Algorithm: Dehaze elementwise multiply pass used to build guided-filter covariance terms
// Canonical explanation: crates/agx/src/adjust/dehaze.md
// CPU equivalent: crates/agx/src/adjust/dehaze.rs (guided_filter)
// Bindings: storage input_a/input_b/output
// Entry points: main

// Element-wise multiply: output[i] = input_a[i] * input_b[i]

@group(0) @binding(0) var<storage, read> input_a: array<f32>;
@group(0) @binding(1) var<storage, read> input_b: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let n = arrayLength(&output);
    if idx >= n { return; }
    output[idx] = input_a[idx] * input_b[idx];
}
