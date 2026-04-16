// Compute wavelet detail, apply soft threshold, accumulate into result,
// and update approximation buffer for next level.
//
// detail = approx - smoothed
// soft_threshold(detail, nr_threshold)
// accum += thresholded detail
// approx = smoothed (prepare next level)

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

@group(0) @binding(0) var<storage, read_write> approx: array<f32>;
@group(0) @binding(1) var<storage, read> smoothed: array<f32>;
@group(0) @binding(2) var<storage, read_write> accum: array<f32>;
@group(0) @binding(3) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    let pixel_count = arrayLength(&approx);
    if idx >= pixel_count { return; }

    let detail = approx[idx] - smoothed[idx];
    let threshold = params.nr_threshold;

    // Soft thresholding: sign(x) * max(|x| - threshold, 0)
    let abs_d = abs(detail);
    var thresholded: f32;
    if abs_d <= threshold {
        thresholded = 0.0;
    } else {
        thresholded = sign(detail) * (abs_d - threshold);
    }

    accum[idx] = accum[idx] + thresholded;

    // Update approximation for next level
    approx[idx] = smoothed[idx];
}
