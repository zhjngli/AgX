// Extract a YCbCr channel from RGB pixel buffer into a single-channel buffer.
// nr_channel: 0 = Y (luminance), 1 = Cb (blue chroma), 2 = Cr (red chroma)

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

const LUMA_R: f32 = 0.2126;
const LUMA_G: f32 = 0.7152;
const LUMA_B: f32 = 0.0722;

@group(0) @binding(0) var<storage, read> pixels: array<f32>;
@group(0) @binding(1) var<storage, read_write> channel_out: array<f32>;
@group(0) @binding(2) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&channel_out);
    if idx >= pixel_count { return; }

    let base = idx * 3u;
    let r = pixels[base];
    let g = pixels[base + 1u];
    let b = pixels[base + 2u];

    let y = LUMA_R * r + LUMA_G * g + LUMA_B * b;

    let ch = u32(params.nr_channel);
    if ch == 0u {
        channel_out[idx] = y;
    } else if ch == 1u {
        channel_out[idx] = b - y; // Cb
    } else {
        channel_out[idx] = r - y; // Cr
    }
}
