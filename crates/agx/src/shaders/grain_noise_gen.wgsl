// Algorithm: Grain noise synthesis pass generating Gaussian-distributed noise for the grain compositor
// Canonical explanation: crates/agx/src/adjust/grain.md
// CPU equivalent: crates/agx/src/adjust/grain.rs (generate_white_noise_buffer)
// Bindings: storage noise/params
// Entry points: main

// Generate Gaussian-distributed noise into a single-channel buffer.
// Uses PCG hash + Box-Muller transform for quality noise generation.

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
    lut_encoding: f32,
    _pad_tc: f32,

    width: f32,
    height: f32,
    _pad5: vec2f,

    detail_strength: f32,
    detail_threshold: f32,
    detail_masking: f32,
    kernel_size: f32,
}

@group(0) @binding(0) var<storage, read_write> noise: array<f32>;
@group(0) @binding(1) var<storage, read> params: Params;

// PCG-style hash for high-quality pseudorandom numbers.
fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) nwg: vec3u) {
    let idx = id.x + id.y * nwg.x * 256u;
    let pixel_count = arrayLength(&noise);
    if idx >= pixel_count { return; }

    // Combine pixel index with seed for unique per-pixel, per-render noise
    let seed = idx + u32(params.grain_seed);
    let u1 = f32(pcg_hash(seed * 2u)) / 4294967295.0;
    let u2 = f32(pcg_hash(seed * 2u + 1u)) / 4294967295.0;

    // Box-Muller transform: uniform -> Gaussian
    let u1_safe = max(u1, 1e-10);  // avoid log(0)
    let r = sqrt(-2.0 * log(u1_safe));
    noise[idx] = r * cos(6.28318530718 * u2);
}
