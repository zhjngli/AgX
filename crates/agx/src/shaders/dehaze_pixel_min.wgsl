// Algorithm: Dehaze per-pixel RGB minimum pass, optionally normalized by airlight, for dark-channel estimation
// Canonical explanation: crates/agx/src/adjust/dehaze.md
// CPU equivalent: crates/agx/src/adjust/dehaze.rs (dark_channel)
// Bindings: storage pixels/output/params
// Entry points: main

// Per-pixel minimum across RGB channels, optionally normalized by airlight.
// dehaze_mode: 0 = raw min(R,G,B), 1 = normalized min(R/Ar, G/Ag, B/Ab)

struct Params {
    exposure: f32, temperature: f32, tint: f32, _pad0: f32,
    contrast: f32, highlights: f32, shadows: f32, whites: f32,
    blacks: f32, _pad1: array<f32, 3>,
    hue_shifts: array<f32, 8>, sat_shifts: array<f32, 8>, lum_shifts: array<f32, 8>,
    cg_shadow_tint: vec4f, cg_midtone_tint: vec4f, cg_highlight_tint: vec4f, cg_global_tint: vec4f,
    cg_balance_factor: f32, cg_balance_active: f32, cg_active: f32, _pad2: f32,
    vignette_amount: f32, vignette_shape: f32, hsl_active: f32, _pad3: f32,
    dehaze_amount: f32, _pad4: array<f32, 3>,
    grain_amount: f32, grain_size: f32, grain_type: f32, grain_seed: f32,
    tc_rgb_active: f32, tc_luma_active: f32, tc_red_active: f32, tc_green_active: f32,
    tc_blue_active: f32, lut_active: f32, _pad_tc: vec2f,
    width: f32, height: f32, _pad5: vec2f,
    detail_strength: f32, detail_threshold: f32, detail_masking: f32, kernel_size: f32,
    nr_luminance: f32, nr_color: f32, nr_detail: f32, nr_channel: f32,
    nr_gap: f32, nr_threshold: f32, nr_is_luma: f32, _pad_nr: f32,
    dehaze_airlight_r: f32, dehaze_airlight_g: f32, dehaze_airlight_b: f32, dehaze_omega: f32,
    dehaze_filter_radius: f32, dehaze_mode: f32, _pad_dh: vec2f,
}

@group(0) @binding(0) var<storage, read> pixels: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&output);
    if idx >= pixel_count { return; }

    let base = idx * 3u;
    var r = pixels[base];
    var g = pixels[base + 1u];
    var b = pixels[base + 2u];

    if params.dehaze_mode > 0.5 {
        // Normalized by airlight
        r = r / max(params.dehaze_airlight_r, 0.01);
        g = g / max(params.dehaze_airlight_g, 0.01);
        b = b / max(params.dehaze_airlight_b, 0.01);
    }

    output[idx] = min(r, min(g, b));
}
