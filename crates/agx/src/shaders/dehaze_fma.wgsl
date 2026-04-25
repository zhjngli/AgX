// Algorithm: Dehaze guided-filter combine pass for `a * b + c` reconstruction
// Canonical explanation: crates/agx/src/adjust/dehaze.md
// CPU equivalent: crates/agx/src/adjust/dehaze.rs (guided_filter)
// Bindings: storage a/b/c/output
// Entry points: main

// Fused multiply-add: output[i] = a[i] * b[i] + c[i]
// Used for guided filter output: t_refined = mean_a * guide + mean_b

@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read> c: array<f32>;
@group(0) @binding(3) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let n = arrayLength(&output);
    if idx >= n { return; }
    output[idx] = a[idx] * b[idx] + c[idx];
}
