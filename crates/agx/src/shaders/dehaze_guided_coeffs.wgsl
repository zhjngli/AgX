// Guided filter coefficient computation.
// a = cov(guide,input) / (var(guide) + eps)
// b = mean_p - a * mean_g
//
// cov(G,P) = mean_GP - mean_G * mean_P
// var(G)   = mean_GG - mean_G^2

const GUIDED_EPS: f32 = 0.001;

@group(0) @binding(0) var<storage, read> mean_g: array<f32>;
@group(0) @binding(1) var<storage, read> mean_p: array<f32>;
@group(0) @binding(2) var<storage, read> mean_gp: array<f32>;
@group(0) @binding(3) var<storage, read> mean_gg: array<f32>;
@group(0) @binding(4) var<storage, read_write> a_out: array<f32>;
@group(0) @binding(5) var<storage, read_write> b_out: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let n = arrayLength(&a_out);
    if idx >= n { return; }

    let mg = mean_g[idx];
    let mp = mean_p[idx];
    let cov_gp = mean_gp[idx] - mg * mp;
    let var_g = mean_gg[idx] - mg * mg;
    let a = cov_gp / (var_g + GUIDED_EPS);
    let b = mp - a * mg;

    a_out[idx] = a;
    b_out[idx] = b;
}
