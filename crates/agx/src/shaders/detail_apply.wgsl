// Apply unsharp mask: adds (original_lum - blurred_lum) * strength to RGB pixels.

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
@group(0) @binding(1) var<storage, read> original_lum: array<f32>;
@group(0) @binding(2) var<storage, read> blurred_lum: array<f32>;
@group(0) @binding(3) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    let pixel_count = arrayLength(&original_lum);
    if idx >= pixel_count { return; }

    let high_freq = original_lum[idx] - blurred_lum[idx];

    // If threshold > 0 and |high_freq| < threshold, skip this pixel
    if params.detail_threshold > 0.0 && abs(high_freq) < params.detail_threshold {
        return;
    }

    // Masking not implemented in GPU path (TODO: edge map)
    let delta = params.detail_strength * high_freq;

    let base = idx * 3u;
    pixels[base]      = clamp(pixels[base]      + delta, 0.0, 1.0);
    pixels[base + 1u] = clamp(pixels[base + 1u] + delta, 0.0, 1.0);
    pixels[base + 2u] = clamp(pixels[base + 2u] + delta, 0.0, 1.0);
}
