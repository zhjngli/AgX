// Gamma-space per-pixel tone adjustments: contrast, highlights, shadows, whites, blacks.
// Runs after linear-to-sRGB, before HSL/color grading/LUT stages.

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
    _pad2: vec2f,

    vignette_amount: f32,
    vignette_shape: f32,
    _pad3: vec2f,

    dehaze_amount: f32,
    _pad4: array<f32, 3>,

    grain_amount: f32,
    grain_size: f32,
    grain_type: f32,
    grain_seed: f32,

    width: f32,
    height: f32,
    _pad5: vec2f,
}

@group(0) @binding(0) var<storage, read_write> pixels: array<f32>;
@group(0) @binding(1) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
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

    pixels[base] = r;
    pixels[base + 1u] = g;
    pixels[base + 2u] = b;
}
