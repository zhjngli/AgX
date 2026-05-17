// Algorithm: Gamma-space per-pixel adjustment stack for contrast, tone curves, HSL, color grading, and LUT
// Canonical explanation: crates/agx/src/adjust/basic_tone.md, crates/agx/src/adjust/tone_curves.md, crates/agx/src/adjust/hsl.md, crates/agx/src/adjust/color_grading.md
// CPU equivalent: crates/agx/src/adjust/basic_tone.rs (apply_contrast/apply_highlights/apply_shadows/apply_whites/apply_blacks), crates/agx/src/adjust/tone_curves.rs (apply_tone_curves_pre), crates/agx/src/adjust/hsl.rs (apply_hsl), crates/agx/src/adjust/color_grading.rs (apply_color_grading_pre)
// Bindings: storage pixels/params/tone_curves plus 3D LUT texture+sampler
// Entry points: main

// Gamma-space per-pixel adjustments: contrast, highlights, shadows, whites, blacks,
// tone curves, HSL, color grading, LUT.

#import common::tone
#import common::color
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

// HSL channel centers (Red, Orange, Yellow, Green, Aqua, Blue, Purple, Magenta)
const CHANNEL_CENTERS = array<f32, 8>(0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 330.0);
const CHANNEL_HALF_WIDTHS = array<f32, 8>(30.0, 30.0, 30.0, 60.0, 60.0, 30.0, 30.0, 30.0);

@group(0) @binding(0) var<storage, read_write> pixels: array<f32>;
@group(0) @binding(1) var<storage, read> params: Params;
@group(0) @binding(2) var<storage, read> tone_curves: array<f32>;
@group(0) @binding(3) var lut_texture: texture_3d<f32>;
@group(0) @binding(4) var lut_sampler: sampler;

// --- Tone curve helpers ---

fn tone_curve_lookup(curve_offset: u32, value: f32) -> f32 {
    let idx = clamp(value * 255.0, 0.0, 255.0);
    let lo = u32(floor(idx));
    let hi = min(lo + 1u, 255u);
    let frac = idx - floor(idx);
    let v_lo = tone_curves[curve_offset + lo];
    let v_hi = tone_curves[curve_offset + hi];
    return v_lo + frac * (v_hi - v_lo);
}

fn apply_tone_curves(r_in: f32, g_in: f32, b_in: f32) -> vec3f {
    var r = r_in;
    var g = g_in;
    var b = b_in;

    // Step 1: RGB master curve (offset 0)
    if params.tc_rgb_active > 0.5 {
        r = tone_curve_lookup(0u, r);
        g = tone_curve_lookup(0u, g);
        b = tone_curve_lookup(0u, b);
    }

    // Step 2: Per-channel curves (red=512, green=768, blue=1024)
    if params.tc_red_active > 0.5 {
        r = tone_curve_lookup(512u, r);
    }
    if params.tc_green_active > 0.5 {
        g = tone_curve_lookup(768u, g);
    }
    if params.tc_blue_active > 0.5 {
        b = tone_curve_lookup(1024u, b);
    }

    // Step 3: Luminance curve (offset 256); scale is unclamped.
    if params.tc_luma_active > 0.5 {
        let l = common::math::luminance(r, g, b);
        let l_new = tone_curve_lookup(256u, l);
        if l > 1e-6 {
            let scale = l_new / l;
            r = r * scale;
            g = g * scale;
            b = b * scale;
        } else {
            r = l_new;
            g = l_new;
            b = l_new;
        }
    }

    return vec3f(r, g, b);
}

// --- HSL ---

fn apply_hsl_pixel(r: f32, g: f32, b: f32) -> vec3f {
    // HSL works in [0, 1] RGB; wide-gamut headroom is lost here.
    // OKHsl is the long-term fix tracked in docs/backlog/color-management.md.
    let r_in = clamp(r, 0.0, 1.0);
    let g_in = clamp(g, 0.0, 1.0);
    let b_in = clamp(b, 0.0, 1.0);
    let hsl = common::color::rgb_to_hsl(vec3f(r_in, g_in, b_in));
    let pixel_hue = hsl.x;
    let pixel_sat = hsl.y;

    // Gray/near-gray pixels: hue is undefined, skip HSL adjustments
    if pixel_sat < 1e-4 {
        return vec3f(r, g, b);
    }

    var total_hue_shift = 0.0;
    var total_sat_shift = 0.0;
    var total_lum_shift = 0.0;

    for (var i = 0u; i < 8u; i = i + 1u) {
        let dist = common::color::hue_distance(pixel_hue, CHANNEL_CENTERS[i]);
        // Scale weight by pixel saturation to fade effect for low-saturation pixels
        let weight = common::color::cosine_weight(dist, CHANNEL_HALF_WIDTHS[i]) * pixel_sat;
        if weight > 0.0 {
            total_hue_shift = total_hue_shift + weight * params.hue_shifts[i];
            total_sat_shift = total_sat_shift + weight * (params.sat_shifts[i] / 100.0);
            total_lum_shift = total_lum_shift + weight * (params.lum_shifts[i] / 100.0);
        }
    }

    let new_hue = ((pixel_hue + total_hue_shift) % 360.0 + 360.0) % 360.0;
    let new_sat = clamp(hsl.y + total_sat_shift, 0.0, 1.0);
    let new_lum = clamp(hsl.z + total_lum_shift, 0.0, 1.0);

    return common::color::hsl_to_rgb(vec3f(new_hue, new_sat, new_lum));
}

// --- Color Grading ---

fn apply_color_grading_pixel(r: f32, g: f32, b: f32) -> vec3f {
    // Pixel luminance (Rec. 709 on gamma-encoded values)
    let lum = common::math::luminance(r, g, b);

    // Balance remapping (skip powf when balance is neutral)
    var lum_adj: f32;
    if params.cg_balance_active > 0.5 {
        lum_adj = pow(clamp(lum, 0.0, 1.0), params.cg_balance_factor);
    } else {
        lum_adj = clamp(lum, 0.0, 1.0);
    }

    // 3-way weights (always sum to 1.0) — SQUARED weights
    let w_shadow = (1.0 - lum_adj) * (1.0 - lum_adj);
    let w_highlight = lum_adj * lum_adj;
    let w_midtone = 1.0 - w_shadow - w_highlight;

    // Weighted blend of regional tints
    let regional_r = params.cg_shadow_tint.x * w_shadow + params.cg_midtone_tint.x * w_midtone + params.cg_highlight_tint.x * w_highlight;
    let regional_g = params.cg_shadow_tint.y * w_shadow + params.cg_midtone_tint.y * w_midtone + params.cg_highlight_tint.y * w_highlight;
    let regional_b = params.cg_shadow_tint.z * w_shadow + params.cg_midtone_tint.z * w_midtone + params.cg_highlight_tint.z * w_highlight;

    // Apply global tint on top
    let combined_r = regional_r * params.cg_global_tint.x;
    let combined_g = regional_g * params.cg_global_tint.y;
    let combined_b = regional_b * params.cg_global_tint.z;

    // Multiply pixel by combined tint; output unclamped — wide-gamut headroom
    // survives this stage, final clamp is at encode.
    var out_r = r * combined_r;
    var out_g = g * combined_g;
    var out_b = b * combined_b;

    // Luminance shifts (weighted additive, pre-divided by 100 via the .w component); unclamped.
    let adjustment = params.cg_shadow_tint.w * w_shadow
        + params.cg_midtone_tint.w * w_midtone
        + params.cg_highlight_tint.w * w_highlight
        + params.cg_global_tint.w;
    out_r = out_r + adjustment;
    out_g = out_g + adjustment;
    out_b = out_b + adjustment;

    return vec3f(out_r, out_g, out_b);
}

// --- LUT ---

fn apply_lut(r: f32, g: f32, b: f32) -> vec3f {
    let coord = vec3f(clamp(r, 0.0, 1.0), clamp(g, 0.0, 1.0), clamp(b, 0.0, 1.0));
    let result = textureSampleLevel(lut_texture, lut_sampler, coord, 0.0);
    return vec3f(result.x, result.y, result.z);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&pixels) / 3u;
    if idx >= pixel_count { return; }
    let base = idx * 3u;
    var r = pixels[base];
    var g = pixels[base + 1u];
    var b = pixels[base + 2u];

    // Contrast
    r = common::tone::apply_contrast(r, params.contrast);
    g = common::tone::apply_contrast(g, params.contrast);
    b = common::tone::apply_contrast(b, params.contrast);

    // Highlights
    r = common::tone::apply_highlights(r, params.highlights);
    g = common::tone::apply_highlights(g, params.highlights);
    b = common::tone::apply_highlights(b, params.highlights);

    // Shadows
    r = common::tone::apply_shadows(r, params.shadows);
    g = common::tone::apply_shadows(g, params.shadows);
    b = common::tone::apply_shadows(b, params.shadows);

    // Whites
    r = common::tone::apply_whites(r, params.whites);
    g = common::tone::apply_whites(g, params.whites);
    b = common::tone::apply_whites(b, params.whites);

    // Blacks
    r = common::tone::apply_blacks(r, params.blacks);
    g = common::tone::apply_blacks(g, params.blacks);
    b = common::tone::apply_blacks(b, params.blacks);

    // Tone curves
    let tc = apply_tone_curves(r, g, b);
    r = tc.x;
    g = tc.y;
    b = tc.z;

    // HSL (skip when neutral to avoid saturation clamping on HDR values)
    if params.hsl_active > 0.5 {
        let hsl_result = apply_hsl_pixel(r, g, b);
        r = hsl_result.x;
        g = hsl_result.y;
        b = hsl_result.z;
    }

    // Color grading (skip when neutral to avoid clamping HDR values)
    if params.cg_active > 0.5 {
        let cg = apply_color_grading_pixel(r, g, b);
        r = cg.x;
        g = cg.y;
        b = cg.z;
    }

    // LUT
    if params.lut_active > 0.5 {
        let lut_result = apply_lut(r, g, b);
        r = lut_result.x;
        g = lut_result.y;
        b = lut_result.z;
    }

    pixels[base] = r;
    pixels[base + 1u] = g;
    pixels[base + 2u] = b;
}
