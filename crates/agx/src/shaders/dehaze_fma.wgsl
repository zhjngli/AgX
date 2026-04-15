// Fused multiply-add: output[i] = a[i] * b[i] + c[i]
// Used for guided filter output: t_refined = mean_a * guide + mean_b

@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read> c: array<f32>;
@group(0) @binding(3) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    let n = arrayLength(&output);
    if idx >= n { return; }
    output[idx] = a[idx] * b[idx] + c[idx];
}
