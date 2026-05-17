#define_import_path common::tone
// Tone adjustment functions for AgX compute shaders.

// Apply contrast to a single channel. Range: -100 to +100.
// Output is unclamped; the final clamp is at encode.
fn apply_contrast(value: f32, contrast: f32) -> f32 {
    if contrast == 0.0 { return value; }
    let factor = (100.0 + contrast) / 100.0;
    return 0.5 + (value - 0.5) * factor;
}

// Apply highlights adjustment. Targets bright pixels (> 0.5). Range: -100 to +100.
// Output is unclamped; the final clamp is at encode.
fn apply_highlights(value: f32, highlights: f32) -> f32 {
    if highlights == 0.0 || value <= 0.5 { return value; }
    let weight = (value - 0.5) / 0.5;
    let adjustment = weight * (highlights / 100.0) * 0.5;
    return value + adjustment;
}

// Apply shadows adjustment. Targets dark pixels (< 0.5). Range: -100 to +100.
// Output is unclamped; the final clamp is at encode.
fn apply_shadows(value: f32, shadows: f32) -> f32 {
    if shadows == 0.0 || value >= 0.5 { return value; }
    let weight = 1.0 - value / 0.5;
    let adjustment = weight * (shadows / 100.0) * 0.5;
    return value + adjustment;
}

// Apply whites adjustment. Targets upper-range pixels (> 0.75). Range: -100 to +100.
// Output is unclamped; the final clamp is at encode.
fn apply_whites(value: f32, whites: f32) -> f32 {
    if whites == 0.0 || value <= 0.75 { return value; }
    let weight = (value - 0.75) / 0.25;
    let adjustment = weight * (whites / 100.0) * 0.25;
    return value + adjustment;
}

// Apply blacks adjustment. Targets lower-range pixels (< 0.25). Range: -100 to +100.
// Output is unclamped; the final clamp is at encode.
fn apply_blacks(value: f32, blacks: f32) -> f32 {
    if blacks == 0.0 || value >= 0.25 { return value; }
    let weight = 1.0 - value / 0.25;
    let adjustment = weight * (blacks / 100.0) * 0.25;
    return value + adjustment;
}

// Exposure multiplier: 2^stops.
fn exposure_factor(stops: f32) -> f32 {
    return pow(2.0, stops);
}

// Apply exposure to a single channel in linear space.
fn apply_exposure(value: f32, factor: f32) -> f32 {
    return max(value * factor, 0.0);
}

// Apply white balance channel multipliers in linear space.
fn apply_white_balance(rgb: vec3f, temperature: f32, tint: f32) -> vec3f {
    if temperature == 0.0 && tint == 0.0 { return rgb; }
    let r_mult = 1.0 + temperature / 200.0;
    let b_mult = 1.0 - temperature / 200.0;
    let g_mult = 1.0 - tint / 200.0;
    let sum = r_mult + g_mult + b_mult;
    let norm = 3.0 / sum;
    return vec3f(
        max(rgb.x * r_mult * norm, 0.0),
        max(rgb.y * g_mult * norm, 0.0),
        max(rgb.z * b_mult * norm, 0.0),
    );
}
