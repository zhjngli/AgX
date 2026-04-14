// Vignette: darkens/brightens image edges based on distance from center.
// Runs in gamma (sRGB) space.

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
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    let pixel_count = arrayLength(&pixels) / 3u;
    if idx >= pixel_count { return; }

    let w = u32(params.width);
    let x = idx % w;
    let y = idx / w;

    let half_w = params.width / 2.0;
    let half_h = params.height / 2.0;

    var inv_x: f32;
    var inv_y: f32;
    if params.vignette_shape < 0.5 {
        // Elliptical
        inv_x = 1.0 / half_w;
        inv_y = 1.0 / half_h;
    } else {
        // Circular
        let inv_r = 1.0 / max(half_w, half_h);
        inv_x = inv_r;
        inv_y = inv_r;
    }

    let strength = params.vignette_amount / 100.0;

    let dx = (f32(x) - half_w) * inv_x;
    let dy = (f32(y) - half_h) * inv_y;
    let d_sq = dx * dx + dy * dy;

    let base = clamp(1.0 - d_sq, 0.0, 1.0);
    let factor = base * base;
    let multiplier = 1.0 + strength * (1.0 - factor);

    let base_idx = idx * 3u;
    pixels[base_idx]      = clamp(pixels[base_idx]      * multiplier, 0.0, 1.0);
    pixels[base_idx + 1u] = clamp(pixels[base_idx + 1u] * multiplier, 0.0, 1.0);
    pixels[base_idx + 2u] = clamp(pixels[base_idx + 2u] * multiplier, 0.0, 1.0);
}
