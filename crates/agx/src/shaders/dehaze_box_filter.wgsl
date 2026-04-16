// 1D box mean filter (separable).
// dehaze_mode: 0 = horizontal, 1 = vertical
// dehaze_filter_radius: window half-size (e.g. 40 for guided filter)

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

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<storage, read> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    let w = u32(params.width);
    let h = u32(params.height);
    let pixel_count = w * h;
    if idx >= pixel_count { return; }

    let radius = i32(params.dehaze_filter_radius);
    let x = i32(idx % w);
    let y = i32(idx / w);

    var sum = 0.0;
    var count = 0.0;

    if params.dehaze_mode < 0.5 {
        // Horizontal
        let left = max(x - radius, 0);
        let right = min(x + radius, i32(w) - 1);
        for (var i = left; i <= right; i = i + 1) {
            sum += input[u32(y) * w + u32(i)];
            count += 1.0;
        }
    } else {
        // Vertical
        let top = max(y - radius, 0);
        let bottom = min(y + radius, i32(h) - 1);
        for (var j = top; j <= bottom; j = j + 1) {
            sum += input[u32(j) * w + u32(x)];
            count += 1.0;
        }
    }

    output[idx] = sum / count;
}
