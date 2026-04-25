// Algorithm: Noise reduction à trous horizontal B3-spline smoothing pass
// Canonical explanation: crates/agx/src/adjust/denoise.md
// CPU equivalent: crates/agx/src/adjust/denoise.rs (atrous_decompose)
// Bindings: storage input/output/params
// Entry points: main

// Horizontal B3-spline convolution with strided taps (à trous wavelet).
// Gap (stride) is 2^level, passed via params.nr_gap.

struct Params {
    exposure: f32,
    temperature: f32,
    tint: f32,
    _pad0: f32,

    contrast: f32,
    highlights: f32,
    shadows: f32,
    whites: f32,

    blacks: f32,
    _pad1: array<f32, 3>,

    hue_shifts: array<f32, 8>,
    sat_shifts: array<f32, 8>,
    lum_shifts: array<f32, 8>,

    cg_shadow_tint: vec4f,
    cg_midtone_tint: vec4f,
    cg_highlight_tint: vec4f,
    cg_global_tint: vec4f,
    cg_balance_factor: f32,
    cg_balance_active: f32,
    cg_active: f32,
    _pad2: f32,

    vignette_amount: f32,
    vignette_shape: f32,
    hsl_active: f32,
    _pad3: f32,

    dehaze_amount: f32,
    _pad4: array<f32, 3>,

    grain_amount: f32,
    grain_size: f32,
    grain_type: f32,
    grain_seed: f32,

    tc_rgb_active: f32,
    tc_luma_active: f32,
    tc_red_active: f32,
    tc_green_active: f32,
    tc_blue_active: f32,
    lut_active: f32,
    _pad_tc: vec2f,

    width: f32,
    height: f32,
    _pad5: vec2f,

    detail_strength: f32,
    detail_threshold: f32,
    detail_masking: f32,
    kernel_size: f32,

    nr_luminance: f32,
    nr_color: f32,
    nr_detail: f32,
    nr_channel: f32,

    nr_gap: f32,
    nr_threshold: f32,
    nr_is_luma: f32,
    _pad_nr: f32,
}

// B3-spline kernel weights
const B3_0: f32 = 0.0625;  // 1/16
const B3_1: f32 = 0.25;    // 4/16
const B3_2: f32 = 0.375;   // 6/16

// Mirror-reflect index at image boundaries.
fn mirror_idx(i: i32, max_val: u32) -> u32 {
    if max_val <= 1u { return 0u; }
    let period = i32(2u * max_val - 2u);
    var pos: i32;
    if i < 0 { pos = -i; } else { pos = i; }
    let m = pos % period;
    if u32(m) >= max_val {
        return u32(period - m);
    } else {
        return u32(m);
    }
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let w = u32(params.width);
    let h = u32(params.height);
    let pixel_count = w * h;
    if idx >= pixel_count { return; }

    let x = i32(idx % w);
    let y = idx / w;
    let gap = i32(params.nr_gap);

    // B3-spline: [1/16, 4/16, 6/16, 4/16, 1/16] with offsets [-2, -1, 0, 1, 2] * gap
    var sum = 0.0;
    sum += B3_0 * input[y * w + mirror_idx(x - 2 * gap, w)];
    sum += B3_1 * input[y * w + mirror_idx(x - gap, w)];
    sum += B3_2 * input[y * w + mirror_idx(x, w)];
    sum += B3_1 * input[y * w + mirror_idx(x + gap, w)];
    sum += B3_0 * input[y * w + mirror_idx(x + 2 * gap, w)];

    output[idx] = sum;
}
