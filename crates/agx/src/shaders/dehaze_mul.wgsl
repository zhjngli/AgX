// Element-wise multiply: output[i] = input_a[i] * input_b[i]

@group(0) @binding(0) var<storage, read> input_a: array<f32>;
@group(0) @binding(1) var<storage, read> input_b: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    let n = arrayLength(&output);
    if idx >= n { return; }
    output[idx] = input_a[idx] * input_b[idx];
}
