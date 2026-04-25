// Algorithm: Dehaze scene recovery or fog addition pass using transmission and airlight
// Canonical explanation: crates/agx/src/adjust/dehaze.md
// CPU equivalent: crates/agx/src/adjust/dehaze.rs (apply_dehaze)
// Bindings: storage pixels/transmission/params
// Entry points: main

// Dehaze scene recovery (positive) or fog addition (negative).
// dehaze_mode: 0 = positive (remove haze), 1 = negative (add fog)
//
// Positive: result[c] = (pixel[c] - A[c]) / max(t, T_MIN) + A[c]
// Negative: result[c] = pixel[c] * (1 - strength) + A[c] * strength

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

const T_MIN: f32 = 0.1;

@group(0) @binding(0) var<storage, read_write> pixels: array<f32>;
@group(0) @binding(1) var<storage, read> transmission: array<f32>;
@group(0) @binding(2) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&transmission);
    if idx >= pixel_count { return; }

    let base = idx * 3u;
    var r = pixels[base];
    var g = pixels[base + 1u];
    var b = pixels[base + 2u];

    let ar = params.dehaze_airlight_r;
    let ag = params.dehaze_airlight_g;
    let ab = params.dehaze_airlight_b;

    if params.dehaze_mode < 0.5 {
        // Positive dehaze: scene recovery
        let t = max(transmission[idx], T_MIN);
        r = clamp((r - ar) / t + ar, 0.0, 1.0);
        g = clamp((g - ag) / t + ag, 0.0, 1.0);
        b = clamp((b - ab) / t + ab, 0.0, 1.0);
    } else {
        // Negative dehaze: add fog (blend toward airlight)
        let strength = params.dehaze_omega; // pre-computed as -amount/100
        r = clamp(r * (1.0 - strength) + ar * strength, 0.0, 1.0);
        g = clamp(g * (1.0 - strength) + ag * strength, 0.0, 1.0);
        b = clamp(b * (1.0 - strength) + ab * strength, 0.0, 1.0);
    }

    pixels[base] = r;
    pixels[base + 1u] = g;
    pixels[base + 2u] = b;
}
