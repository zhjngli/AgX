#define_import_path common::color
// Color space conversion and HSL utilities for AgX compute shaders.

// Linear Rec.2020 -> linear sRGB. WGSL mat3x3 is column-major, so the
// vec3 arguments below are columns of the matrix -- equivalent to the
// row-major declaration in crate::color_space.
const LINEAR_REC2020_TO_LINEAR_SRGB: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>( 1.660491, -0.124550, -0.018151),
    vec3<f32>(-0.587641,  1.132899, -0.100579),
    vec3<f32>(-0.072850, -0.008349,  1.118730),
);

// Linear sRGB -> linear Rec.2020 (inverse of the above). Same column-major
// layout caveat applies.
const LINEAR_SRGB_TO_LINEAR_REC2020: mat3x3<f32> = mat3x3<f32>(
    vec3<f32>(0.627404, 0.069097, 0.016391),
    vec3<f32>(0.329283, 0.919541, 0.088013),
    vec3<f32>(0.043313, 0.011362, 0.895595),
);

// Sign-preserving sRGB transfer: sign(x) * srgb_curve(abs(x)). The standard
// sRGB curve is defined only for non-negative inputs; for negative inputs
// (which arise from out-of-gamut math in wide working spaces), apply the
// curve to abs(x) and negate. Matches crate::color_space::srgb_curve_signed.
fn srgb_curve_signed(x: f32) -> f32 {
    let sign_factor = select(1.0, -1.0, x < 0.0);
    let ax = abs(x);
    let curved = select(
        1.055 * pow(ax, 1.0 / 2.4) - 0.055,
        12.92 * ax,
        ax <= 0.0031308
    );
    return sign_factor * curved;
}

// Inverse of srgb_curve_signed: sign(x) * srgb_curve_inverse(abs(x)).
// Matches crate::color_space::srgb_curve_signed_inverse.
fn srgb_curve_signed_inverse(x: f32) -> f32 {
    let sign_factor = select(1.0, -1.0, x < 0.0);
    let ax = abs(x);
    let lin = select(
        pow((ax + 0.055) / 1.055, 2.4),
        ax / 12.92,
        ax <= 0.04045
    );
    return sign_factor * lin;
}

// Sign-preserving sRGB transfer applied per channel.
// CPU equivalent: srgb_curve_signed in crate::color_space, applied to each channel.
fn linear_to_gamma(rgb: vec3f) -> vec3f {
    return vec3f(
        srgb_curve_signed(rgb.x),
        srgb_curve_signed(rgb.y),
        srgb_curve_signed(rgb.z),
    );
}

fn gamma_to_linear(rgb: vec3f) -> vec3f {
    return vec3f(
        srgb_curve_signed_inverse(rgb.x),
        srgb_curve_signed_inverse(rgb.y),
        srgb_curve_signed_inverse(rgb.z),
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
const PI: f32 = 3.14159265358979323846;

fn cosine_weight(hue_dist: f32, half_width: f32) -> f32 {
    if hue_dist >= half_width {
        return 0.0;
    }
    return cos(hue_dist / half_width * PI) * 0.5 + 0.5;
}
