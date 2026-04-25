// Algorithm: Linear-space white balance and exposure pass before gamma conversion
// Canonical explanation: crates/agx/src/adjust/white_balance.md, crates/agx/src/adjust/exposure.md
// CPU equivalent: crates/agx/src/adjust/white_balance.rs (apply_white_balance), crates/agx/src/adjust/exposure.rs (apply_exposure)
// Bindings: storage pixels/params
// Entry points: main

// Linear-space adjustments: white balance + exposure.
// Runs before the linear-to-sRGB conversion.

#import common::tone

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

@group(0) @binding(0) var<storage, read_write> pixels: array<f32>;
@group(0) @binding(1) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&pixels) / 3u;
    if idx >= pixel_count { return; }
    let base = idx * 3u;
    var rgb = vec3f(pixels[base], pixels[base + 1u], pixels[base + 2u]);

    // White balance (temperature + tint)
    rgb = common::tone::apply_white_balance(rgb, params.temperature, params.tint);

    // Exposure
    let factor = common::tone::exposure_factor(params.exposure);
    rgb = vec3f(
        common::tone::apply_exposure(rgb.x, factor),
        common::tone::apply_exposure(rgb.y, factor),
        common::tone::apply_exposure(rgb.z, factor),
    );

    pixels[base] = rgb.x;
    pixels[base + 1u] = rgb.y;
    pixels[base + 2u] = rgb.z;
}
