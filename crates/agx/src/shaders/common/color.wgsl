// Color space conversion and HSL utilities for AgX compute shaders.

#import common::math

// Convert a single linear sRGB channel to sRGB gamma.
fn linear_to_srgb_channel(v: f32) -> f32 {
    if v <= 0.0031308 {
        return v * 12.92;
    }
    return 1.055 * pow(v, 1.0 / 2.4) - 0.055;
}

// Convert a single sRGB gamma channel to linear sRGB.
fn srgb_to_linear_channel(v: f32) -> f32 {
    if v <= 0.04045 {
        return v / 12.92;
    }
    return pow((v + 0.055) / 1.055, 2.4);
}

// Convert linear sRGB pixel to sRGB gamma.
fn linear_to_srgb(rgb: vec3f) -> vec3f {
    return vec3f(
        linear_to_srgb_channel(rgb.x),
        linear_to_srgb_channel(rgb.y),
        linear_to_srgb_channel(rgb.z),
    );
}

// Convert sRGB gamma pixel to linear sRGB.
fn srgb_to_linear(rgb: vec3f) -> vec3f {
    return vec3f(
        srgb_to_linear_channel(rgb.x),
        srgb_to_linear_channel(rgb.y),
        srgb_to_linear_channel(rgb.z),
    );
}

// Convert sRGB to HSL. Returns (hue 0-360, saturation 0-1, lightness 0-1).
fn rgb_to_hsl(rgb: vec3f) -> vec3f {
    let r = rgb.x;
    let g = rgb.y;
    let b = rgb.z;

    let max_c = max(r, max(g, b));
    let min_c = min(r, min(g, b));
    let delta = max_c - min_c;
    let l = (max_c + min_c) * 0.5;

    if delta < 1e-6 {
        return vec3f(0.0, 0.0, l);
    }

    let s = select(
        delta / (2.0 - max_c - min_c),
        delta / (max_c + min_c),
        l < 0.5
    );

    var h: f32;
    if max_c == r {
        h = ((g - b) / delta) % 6.0;
    } else if max_c == g {
        h = (b - r) / delta + 2.0;
    } else {
        h = (r - g) / delta + 4.0;
    }
    h = h * 60.0;
    if h < 0.0 {
        h = h + 360.0;
    }

    return vec3f(h, s, l);
}

// Convert HSL to sRGB. Input: (hue 0-360, saturation 0-1, lightness 0-1).
fn hsl_to_rgb(hsl: vec3f) -> vec3f {
    let h = hsl.x;
    let s = hsl.y;
    let l = hsl.z;

    if s < 1e-6 {
        return vec3f(l, l, l);
    }

    let q = select(l + s - l * s, l * (1.0 + s), l < 0.5);
    let p = 2.0 * l - q;
    let h_norm = h / 360.0;

    return vec3f(
        hue_to_rgb(p, q, h_norm + 1.0 / 3.0),
        hue_to_rgb(p, q, h_norm),
        hue_to_rgb(p, q, h_norm - 1.0 / 3.0),
    );
}

fn hue_to_rgb(p: f32, q: f32, t_in: f32) -> f32 {
    var t = t_in;
    if t < 0.0 { t = t + 1.0; }
    if t > 1.0 { t = t - 1.0; }
    if t < 1.0 / 6.0 { return p + (q - p) * 6.0 * t; }
    if t < 1.0 / 2.0 { return q; }
    if t < 2.0 / 3.0 { return p + (q - p) * (2.0 / 3.0 - t) * 6.0; }
    return p;
}

// Shortest angular distance between two hue angles in degrees. Result in [0, 180].
fn hue_distance(a: f32, b: f32) -> f32 {
    let d = ((a - b) % 360.0 + 360.0) % 360.0;
    return select(360.0 - d, d, d <= 180.0);
}

// Cosine falloff weight: 1.0 at center, 0.0 at half_width.
fn cosine_weight(hue_dist: f32, half_width: f32) -> f32 {
    if hue_dist >= half_width {
        return 0.0;
    }
    return cos(hue_dist / half_width * 3.14159265) * 0.5 + 0.5;
}
