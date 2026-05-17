// Algorithm: Grain compositing pass for film grain with luminance weighting and blur-based size control
// Canonical explanation: crates/agx/src/adjust/grain.md
// CPU equivalent: crates/agx/src/adjust/grain.rs (apply_grain_buffer)
// Bindings: storage pixels/noise/params
// Entry points: main

// Apply blurred noise to pixel buffer with luminance-weighted grain.
// Implements per-type config selection and additive/multiplicative blending.

#import common::math

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
@group(0) @binding(1) var<storage, read> noise: array<f32>;
@group(0) @binding(2) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&pixels) / 3u;
    if idx >= pixel_count { return; }

    let amount = params.grain_amount;
    if amount == 0.0 { return; }

    // Per-type config: (contrast, luma_falloff, amount_curve)
    var grain_contrast: f32;
    var luma_falloff: f32;
    var amount_curve: f32;
    let gt = params.grain_type;
    if gt < 0.5 {
        // Fine
        grain_contrast = 0.95;
        luma_falloff = 2.5;
        amount_curve = 0.7;
    } else if gt < 1.5 {
        // Silver
        grain_contrast = 1.2;
        luma_falloff = 1.5;
        amount_curve = 0.6;
    } else {
        // Harsh
        grain_contrast = 1.5;
        luma_falloff = 0.8;
        amount_curve = 0.5;
    }

    let amount_factor = pow(amount / 100.0, amount_curve);
    let scale = grain_contrast * 0.04 * amount_factor;
    let effective_falloff = luma_falloff * (1.0 - 0.4 * amount_factor);

    let base_idx = idx * 3u;
    let r = pixels[base_idx];
    let g = pixels[base_idx + 1u];
    let b = pixels[base_idx + 2u];

    let luma = common::math::luminance(r, g, b);
    let luminance_weight = pow(1.0 - clamp(luma, 0.0, 1.0), 0.5 * effective_falloff);
    let blend = common::math::smoothstep_agx(0.1, 0.2, luma);

    let n = noise[idx];
    let nws = n * luminance_weight * scale;

    // Apply grain to each channel; output unclamped — the final clamp is at encode.
    let additive_r = nws * 0.35;
    let multiplicative_r = r * (exp(nws) - 1.0);
    let delta_r = additive_r + (multiplicative_r - additive_r) * blend;
    pixels[base_idx] = r + delta_r;

    let additive_g = nws * 0.35;
    let multiplicative_g = g * (exp(nws) - 1.0);
    let delta_g = additive_g + (multiplicative_g - additive_g) * blend;
    pixels[base_idx + 1u] = g + delta_g;

    let additive_b = nws * 0.35;
    let multiplicative_b = b * (exp(nws) - 1.0);
    let delta_b = additive_b + (multiplicative_b - additive_b) * blend;
    pixels[base_idx + 2u] = b + delta_b;
}
