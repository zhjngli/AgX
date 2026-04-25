// Algorithm: Separable Gaussian blur horizontal pass for detail sharpening and grain sizing
// Canonical explanation: crates/agx/src/adjust/detail.md
// CPU equivalent: crates/agx/src/adjust/detail.rs (gaussian_blur)
// Bindings: storage input/output/kernel plus Params
// Entry points: main

// Horizontal Gaussian blur pass on a single-channel buffer.

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
}

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<storage, read> kernel: array<f32>;
@group(0) @binding(3) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let w = u32(params.width);
    let h = u32(params.height);
    let pixel_count = w * h;
    if idx >= pixel_count { return; }

    let x = idx % w;
    let y = idx / w;
    let kernel_len = u32(params.kernel_size);
    let half = kernel_len / 2u;

    var sum = 0.0;
    for (var ki = 0u; ki < kernel_len; ki = ki + 1u) {
        var sx = i32(x) + i32(ki) - i32(half);
        sx = clamp(sx, 0, i32(w) - 1);
        sum = sum + input[y * w + u32(sx)] * kernel[ki];
    }
    output[idx] = sum;
}
