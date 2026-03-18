use palette::{Hsl, IntoColor, LinSrgb, Srgb};
use serde::{Deserialize, Serialize};

// --- Color space helpers ---

/// Convert linear sRGB to sRGB gamma space.
pub fn linear_to_srgb(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let srgb: Srgb<f32> = LinSrgb::new(r, g, b).into_encoding();
    (srgb.red, srgb.green, srgb.blue)
}

/// Convert sRGB gamma space to linear sRGB.
pub fn srgb_to_linear(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let lin: LinSrgb<f32> = Srgb::new(r, g, b).into_linear();
    (lin.red, lin.green, lin.blue)
}

// --- Exposure (linear space) ---

/// Compute the exposure multiplier for the given number of stops.
/// 0 stops = 1.0 (no change), +1 stop = 2.0, -1 stop = 0.5.
pub fn exposure_factor(stops: f32) -> f32 {
    2.0f32.powf(stops)
}

/// Apply exposure to a single channel value in linear space.
pub fn apply_exposure(value: f32, factor: f32) -> f32 {
    (value * factor).max(0.0)
}

// --- White balance (linear space) ---

/// Apply white balance temperature and tint shifts. Returns adjusted (r, g, b) in linear space.
///
/// Temperature > 0 = warm (boost red, reduce blue).
/// Tint > 0 = magenta (reduce green).
/// Channel multipliers are normalized to preserve overall brightness.
pub fn apply_white_balance(r: f32, g: f32, b: f32, temperature: f32, tint: f32) -> (f32, f32, f32) {
    if temperature == 0.0 && tint == 0.0 {
        return (r, g, b);
    }
    let r_mult = 1.0 + temperature / 200.0;
    let b_mult = 1.0 - temperature / 200.0;
    let g_mult = 1.0 - tint / 200.0;

    // Normalize to preserve brightness
    let sum = r_mult + g_mult + b_mult;
    let norm = 3.0 / sum;

    (
        (r * r_mult * norm).max(0.0),
        (g * g_mult * norm).max(0.0),
        (b * b_mult * norm).max(0.0),
    )
}

// --- Contrast (sRGB gamma space) ---

/// Apply contrast adjustment to a single channel value in sRGB gamma space.
/// Contrast range: -100 to +100. 0 = no change.
pub fn apply_contrast(value: f32, contrast: f32) -> f32 {
    if contrast == 0.0 {
        return value;
    }
    let factor = (100.0 + contrast) / 100.0;
    (0.5 + (value - 0.5) * factor).clamp(0.0, 1.0)
}

// --- Highlights (sRGB gamma space) ---

/// Apply highlights adjustment to a single channel value in sRGB gamma space.
/// Targets bright pixels (> 0.5). Range: -100 to +100.
pub fn apply_highlights(value: f32, highlights: f32) -> f32 {
    if highlights == 0.0 || value <= 0.5 {
        return value;
    }
    let weight = (value - 0.5) / 0.5; // 0 at 0.5, 1 at 1.0
    let adjustment = weight * (highlights / 100.0) * 0.5;
    (value + adjustment).clamp(0.0, 1.0)
}

// --- Shadows (sRGB gamma space) ---

/// Apply shadows adjustment to a single channel value in sRGB gamma space.
/// Targets dark pixels (< 0.5). Range: -100 to +100.
pub fn apply_shadows(value: f32, shadows: f32) -> f32 {
    if shadows == 0.0 || value >= 0.5 {
        return value;
    }
    let weight = 1.0 - value / 0.5; // 1 at 0.0, 0 at 0.5
    let adjustment = weight * (shadows / 100.0) * 0.5;
    (value + adjustment).clamp(0.0, 1.0)
}

// --- Whites (sRGB gamma space) ---

/// Apply whites adjustment to a single channel value in sRGB gamma space.
/// Targets upper-range pixels (> 0.75). Range: -100 to +100.
pub fn apply_whites(value: f32, whites: f32) -> f32 {
    if whites == 0.0 || value <= 0.75 {
        return value;
    }
    let weight = (value - 0.75) / 0.25; // 0 at 0.75, 1 at 1.0
    let adjustment = weight * (whites / 100.0) * 0.25;
    (value + adjustment).clamp(0.0, 1.0)
}

// --- Blacks (sRGB gamma space) ---

/// Apply blacks adjustment to a single channel value in sRGB gamma space.
/// Targets lower-range pixels (< 0.25). Range: -100 to +100.
pub fn apply_blacks(value: f32, blacks: f32) -> f32 {
    if blacks == 0.0 || value >= 0.25 {
        return value;
    }
    let weight = 1.0 - value / 0.25; // 1 at 0.0, 0 at 0.25
    let adjustment = weight * (blacks / 100.0) * 0.25;
    (value + adjustment).clamp(0.0, 1.0)
}

// --- HSL helpers ---

/// Type alias for HSL weight functions. Takes (hue_distance, half_width) in degrees,
/// returns a 0.0–1.0 weight.
pub type WeightFn = fn(f32, f32) -> f32;

/// Compute the shortest angular distance between two hue angles in degrees.
/// Result is always in [0, 180].
pub fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(360.0);
    if d > 180.0 {
        360.0 - d
    } else {
        d
    }
}

/// Cosine falloff: smooth bell curve, 1.0 at center, 0.0 at half_width.
/// hue_distance and half_width are in degrees.
pub fn cosine_weight(hue_dist: f32, half_width: f32) -> f32 {
    if hue_dist >= half_width {
        0.0
    } else {
        ((hue_dist / half_width) * std::f32::consts::PI).cos() * 0.5 + 0.5
    }
}

/// Channel center hues in degrees.
/// Order: Red, Orange, Yellow, Green, Aqua, Blue, Purple, Magenta.
const CHANNEL_CENTERS: [f32; 8] = [0.0, 30.0, 60.0, 120.0, 180.0, 240.0, 270.0, 330.0];

/// Half-width of each channel's influence zone in degrees.
/// Derived from distance to nearest neighbor. At channel boundaries (e.g. hue 300°
/// between Purple and Magenta), weight drops to zero — this is expected behavior
/// matching the Lightroom/Capture One approach with non-uniform spacing.
const CHANNEL_HALF_WIDTHS: [f32; 8] = [30.0, 30.0, 30.0, 60.0, 60.0, 30.0, 30.0, 30.0];

/// Apply per-channel HSL adjustments to an sRGB gamma pixel.
///
/// Takes 3 arrays of 8 values each (one per channel, ordered Red through Magenta):
/// - `hue_shifts`: degrees, -180 to +180
/// - `saturation_shifts`: -100 to +100
/// - `luminance_shifts`: -100 to +100
///
/// The `weight_fn(hue_distance, half_width) -> weight` is pluggable.
/// Gray pixels (saturation < 1e-4) are returned unchanged. Channel weights are
/// scaled by pixel saturation to smoothly fade the effect for low-saturation pixels.
pub fn apply_hsl(
    r: f32,
    g: f32,
    b: f32,
    hue_shifts: &[f32; 8],
    saturation_shifts: &[f32; 8],
    luminance_shifts: &[f32; 8],
    weight_fn: WeightFn,
) -> (f32, f32, f32) {
    let srgb = Srgb::new(r, g, b);
    let hsl: Hsl = srgb.into_color();
    let pixel_hue = hsl.hue.into_positive_degrees();
    let pixel_sat = hsl.saturation;

    // Gray/near-gray pixels: hue is undefined, skip HSL adjustments
    if pixel_sat < 1e-4 {
        return (r, g, b);
    }

    let mut total_hue_shift = 0.0f32;
    let mut total_sat_shift = 0.0f32;
    let mut total_lum_shift = 0.0f32;

    for i in 0..8 {
        let dist = hue_distance(pixel_hue, CHANNEL_CENTERS[i]);
        // Scale weight by pixel saturation to fade effect for low-saturation pixels
        let weight = weight_fn(dist, CHANNEL_HALF_WIDTHS[i]) * pixel_sat;
        if weight > 0.0 {
            total_hue_shift += weight * hue_shifts[i];
            total_sat_shift += weight * (saturation_shifts[i] / 100.0);
            total_lum_shift += weight * (luminance_shifts[i] / 100.0);
        }
    }

    let new_hue = (pixel_hue + total_hue_shift).rem_euclid(360.0);
    let new_sat = (hsl.saturation + total_sat_shift).clamp(0.0, 1.0);
    let new_lum = (hsl.lightness + total_lum_shift).clamp(0.0, 1.0);

    let new_hsl = Hsl::new(new_hue, new_sat, new_lum);
    let rgb: Srgb<f32> = new_hsl.into_color();
    (rgb.red, rgb.green, rgb.blue)
}

// --- Vignette (sRGB gamma space, position-dependent) ---

/// Vignette falloff geometry.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VignetteShape {
    #[default]
    Elliptical,
    Circular,
}

/// Apply creative vignette to an sRGB gamma pixel.
///
/// Darkens (negative amount) or brightens (positive amount) edges based on
/// distance from center. Amount range: -100 to +100. 0 = no effect.
///
/// `x, y` = pixel coordinates, `w, h` = image dimensions.
pub fn apply_vignette(
    r: f32, g: f32, b: f32,
    amount: f32,
    shape: VignetteShape,
    x: u32, y: u32,
    w: u32, h: u32,
) -> (f32, f32, f32) {
    if amount == 0.0 {
        return (r, g, b);
    }

    let half_w = w as f32 / 2.0;
    let half_h = h as f32 / 2.0;
    let dx = x as f32 - half_w;
    let dy = y as f32 - half_h;

    let d_sq = match shape {
        VignetteShape::Elliptical => {
            (dx / half_w).powi(2) + (dy / half_h).powi(2)
        }
        VignetteShape::Circular => {
            let radius = half_w.max(half_h);
            (dx / radius).powi(2) + (dy / radius).powi(2)
        }
    };

    let factor = (1.0 - d_sq).clamp(0.0, 1.0).powi(2);
    let strength = amount / 100.0;
    let multiplier = 1.0 + strength * (1.0 - factor);

    (
        (r * multiplier).clamp(0.0, 1.0),
        (g * multiplier).clamp(0.0, 1.0),
        (b * multiplier).clamp(0.0, 1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Exposure tests ---

    #[test]
    fn exposure_factor_zero_is_one() {
        assert_eq!(exposure_factor(0.0), 1.0);
    }

    #[test]
    fn exposure_factor_one_stop_doubles() {
        assert!((exposure_factor(1.0) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn exposure_factor_neg_one_halves() {
        assert!((exposure_factor(-1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn apply_exposure_multiplies() {
        assert!((apply_exposure(0.25, exposure_factor(1.0)) - 0.5).abs() < 1e-6);
    }

    // --- Color space roundtrip ---

    #[test]
    fn linear_srgb_roundtrip() {
        let (sr, sg, sb) = linear_to_srgb(0.5, 0.3, 0.1);
        let (lr, lg, lb) = srgb_to_linear(sr, sg, sb);
        assert!((lr - 0.5).abs() < 1e-5);
        assert!((lg - 0.3).abs() < 1e-5);
        assert!((lb - 0.1).abs() < 1e-5);
    }

    // --- Contrast tests ---

    #[test]
    fn contrast_zero_is_identity() {
        assert_eq!(apply_contrast(0.7, 0.0), 0.7);
    }

    #[test]
    fn contrast_positive_increases_deviation() {
        let mid = 0.8;
        let result = apply_contrast(mid, 50.0);
        // Above midpoint should move further from 0.5
        assert!(result > mid);
    }

    #[test]
    fn contrast_negative_decreases_deviation() {
        let mid = 0.8;
        let result = apply_contrast(mid, -50.0);
        assert!(result < mid);
    }

    #[test]
    fn contrast_output_clamped() {
        assert!(apply_contrast(1.0, 100.0) <= 1.0);
        assert!(apply_contrast(0.0, 100.0) >= 0.0);
    }

    // --- Highlights tests ---

    #[test]
    fn highlights_zero_is_identity() {
        assert_eq!(apply_highlights(0.8, 0.0), 0.8);
    }

    #[test]
    fn highlights_dark_pixels_unaffected() {
        assert_eq!(apply_highlights(0.3, 50.0), 0.3);
    }

    #[test]
    fn highlights_negative_darkens_bright() {
        assert!(apply_highlights(0.9, -50.0) < 0.9);
    }

    #[test]
    fn highlights_positive_brightens_bright() {
        assert!(apply_highlights(0.9, 50.0) > 0.9);
    }

    #[test]
    fn highlights_brighter_pixels_affected_more() {
        let change_at_60 = (apply_highlights(0.6, 50.0) - 0.6).abs();
        let change_at_90 = (apply_highlights(0.9, 50.0) - 0.9).abs();
        assert!(change_at_90 > change_at_60);
    }

    // --- Shadows tests ---

    #[test]
    fn shadows_zero_is_identity() {
        assert_eq!(apply_shadows(0.2, 0.0), 0.2);
    }

    #[test]
    fn shadows_bright_pixels_unaffected() {
        assert_eq!(apply_shadows(0.7, 50.0), 0.7);
    }

    #[test]
    fn shadows_positive_lifts_darks() {
        assert!(apply_shadows(0.1, 50.0) > 0.1);
    }

    #[test]
    fn shadows_negative_crushes_darks() {
        assert!(apply_shadows(0.1, -50.0) < 0.1);
    }

    #[test]
    fn shadows_darker_pixels_affected_more() {
        let change_at_10 = (apply_shadows(0.1, 50.0) - 0.1).abs();
        let change_at_40 = (apply_shadows(0.4, 50.0) - 0.4).abs();
        assert!(change_at_10 > change_at_40);
    }

    // --- Whites tests ---

    #[test]
    fn whites_zero_is_identity() {
        assert_eq!(apply_whites(0.9, 0.0), 0.9);
    }

    #[test]
    fn whites_dark_pixels_unaffected() {
        assert_eq!(apply_whites(0.5, 50.0), 0.5);
    }

    #[test]
    fn whites_positive_brightens_upper() {
        assert!(apply_whites(0.9, 50.0) > 0.9);
    }

    #[test]
    fn whites_negative_darkens_upper() {
        assert!(apply_whites(0.9, -50.0) < 0.9);
    }

    // --- Blacks tests ---

    #[test]
    fn blacks_zero_is_identity() {
        assert_eq!(apply_blacks(0.1, 0.0), 0.1);
    }

    #[test]
    fn blacks_bright_pixels_unaffected() {
        assert_eq!(apply_blacks(0.5, 50.0), 0.5);
    }

    #[test]
    fn blacks_positive_lifts() {
        assert!(apply_blacks(0.1, 50.0) > 0.1);
    }

    #[test]
    fn blacks_negative_crushes() {
        assert!(apply_blacks(0.1, -50.0) < 0.1);
    }

    // --- White balance tests ---

    #[test]
    fn white_balance_zero_is_identity() {
        let (r, g, b) = apply_white_balance(0.5, 0.5, 0.5, 0.0, 0.0);
        assert!((r - 0.5).abs() < 1e-6);
        assert!((g - 0.5).abs() < 1e-6);
        assert!((b - 0.5).abs() < 1e-6);
    }

    #[test]
    fn white_balance_warm_boosts_red_reduces_blue() {
        let (r, _g, b) = apply_white_balance(0.5, 0.5, 0.5, 50.0, 0.0);
        assert!(r > 0.5);
        assert!(b < 0.5);
    }

    #[test]
    fn white_balance_cool_boosts_blue_reduces_red() {
        let (r, _g, b) = apply_white_balance(0.5, 0.5, 0.5, -50.0, 0.0);
        assert!(r < 0.5);
        assert!(b > 0.5);
    }

    #[test]
    fn white_balance_tint_positive_reduces_green() {
        let (_r, g, _b) = apply_white_balance(0.5, 0.5, 0.5, 0.0, 50.0);
        assert!(g < 0.5);
    }

    #[test]
    fn white_balance_output_non_negative() {
        let (r, g, b) = apply_white_balance(0.5, 0.5, 0.5, 100.0, 100.0);
        assert!(r >= 0.0);
        assert!(g >= 0.0);
        assert!(b >= 0.0);
    }

    // --- HSL helpers ---

    #[test]
    fn hue_distance_same_is_zero() {
        assert_eq!(hue_distance(120.0, 120.0), 0.0);
    }

    #[test]
    fn hue_distance_opposite_is_180() {
        assert!((hue_distance(0.0, 180.0) - 180.0).abs() < 1e-6);
    }

    #[test]
    fn hue_distance_wraps_around() {
        assert!((hue_distance(350.0, 10.0) - 20.0).abs() < 1e-6);
        assert!((hue_distance(10.0, 350.0) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn hue_distance_is_symmetric() {
        assert!((hue_distance(30.0, 90.0) - hue_distance(90.0, 30.0)).abs() < 1e-6);
    }

    #[test]
    fn cosine_weight_at_center_is_one() {
        assert!((cosine_weight(0.0, 30.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_weight_at_half_width_is_zero() {
        assert!(cosine_weight(30.0, 30.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_weight_beyond_half_width_is_zero() {
        assert_eq!(cosine_weight(45.0, 30.0), 0.0);
    }

    #[test]
    fn cosine_weight_at_half_distance_is_between_zero_and_one() {
        let w = cosine_weight(15.0, 30.0);
        assert!(w > 0.0 && w < 1.0, "Expected 0 < {} < 1", w);
    }

    // --- apply_hsl tests ---

    #[test]
    fn apply_hsl_all_zeros_is_identity() {
        let zeros = [0.0f32; 8];
        let (r, g, b) = apply_hsl(1.0, 0.0, 0.0, &zeros, &zeros, &zeros, cosine_weight);
        assert!((r - 1.0).abs() < 1e-4, "r: expected ~1.0, got {r}");
        assert!(g.abs() < 1e-4, "g: expected ~0.0, got {g}");
        assert!(b.abs() < 1e-4, "b: expected ~0.0, got {b}");
    }

    #[test]
    fn apply_hsl_red_hue_shift_rotates_red() {
        // Pure red (hue 0°), shift hue +120° → should become green-ish
        let mut hue = [0.0f32; 8];
        hue[0] = 120.0; // red channel hue shift
        let zeros = [0.0f32; 8];
        let (r, g, _b) = apply_hsl(1.0, 0.0, 0.0, &hue, &zeros, &zeros, cosine_weight);
        assert!(
            g > r,
            "Expected green > red after +120° hue shift, got r={r} g={g}"
        );
    }

    #[test]
    fn apply_hsl_red_saturation_decrease_desaturates() {
        let zeros = [0.0f32; 8];
        let mut sat = [0.0f32; 8];
        sat[0] = -100.0; // red channel full desaturate
        let (r, g, b) = apply_hsl(1.0, 0.0, 0.0, &zeros, &sat, &zeros, cosine_weight);
        // Desaturated red → gray-ish, channel spread should decrease from input spread of 1.0
        let input_spread = 1.0f32; // pure red: r=1.0, g=0.0 → spread = 1.0
        assert!(
            (r - g).abs() < input_spread,
            "Expected channels closer after desaturation, got r={r} g={g}"
        );
        assert!(
            (r - b).abs() < input_spread,
            "Expected channels closer after desaturation, got r={r} b={b}"
        );
    }

    #[test]
    fn apply_hsl_green_shift_does_not_affect_red() {
        // Pure red pixel, only green channel has a shift → red should be unaffected
        let zeros = [0.0f32; 8];
        let mut sat = [0.0f32; 8];
        sat[3] = -100.0; // green channel (index 3)
        let (r, g, b) = apply_hsl(1.0, 0.0, 0.0, &zeros, &sat, &zeros, cosine_weight);
        assert!(
            (r - 1.0).abs() < 1e-3,
            "Red pixel should be unaffected by green channel"
        );
        assert!(g.abs() < 1e-3);
        assert!(b.abs() < 1e-3);
    }

    #[test]
    fn apply_hsl_gray_pixel_unaffected() {
        // Gray pixel (saturation ≈ 0) should not be affected by HSL
        let mut hue = [0.0f32; 8];
        let mut sat = [0.0f32; 8];
        let mut lum = [0.0f32; 8];
        hue[0] = 90.0;
        sat[0] = 50.0;
        lum[0] = 50.0;
        let (r, g, b) = apply_hsl(0.5, 0.5, 0.5, &hue, &sat, &lum, cosine_weight);
        assert!(
            (r - 0.5).abs() < 1e-3,
            "Gray should be unaffected, got r={r}"
        );
        assert!(
            (g - 0.5).abs() < 1e-3,
            "Gray should be unaffected, got g={g}"
        );
        assert!(
            (b - 0.5).abs() < 1e-3,
            "Gray should be unaffected, got b={b}"
        );
    }

    #[test]
    fn apply_hsl_luminance_brightens() {
        let zeros = [0.0f32; 8];
        let mut lum = [0.0f32; 8];
        lum[0] = 50.0; // brighten reds
        let (r, g, b) = apply_hsl(1.0, 0.0, 0.0, &zeros, &zeros, &lum, cosine_weight);
        // Pure red at full saturation (lightness=0.5) — adding luminance moves toward white.
        // r stays at 1.0 while g and b increase, so the rgb sum grows.
        let orig_sum: f32 = 1.0 + 0.0 + 0.0;
        let new_sum = r + g + b;
        assert!(
            new_sum > orig_sum,
            "Expected brighter, got sum={new_sum} vs {orig_sum}"
        );
    }

    // --- Vignette tests ---

    #[test]
    fn vignette_zero_amount_is_identity() {
        let (r, g, b) = super::apply_vignette(0.8, 0.5, 0.3, 0.0, super::VignetteShape::Elliptical, 0, 0, 100, 100);
        assert!((r - 0.8).abs() < 1e-6);
        assert!((g - 0.5).abs() < 1e-6);
        assert!((b - 0.3).abs() < 1e-6);
    }

    #[test]
    fn vignette_center_pixel_unchanged() {
        // 100x100 image: half_w = 50.0. Pixel (50, 50) → dx = 0, dy = 0 → factor = 1.0 exactly.
        let (r, g, b) = super::apply_vignette(0.8, 0.5, 0.3, -50.0, super::VignetteShape::Elliptical, 50, 50, 100, 100);
        assert!((r - 0.8).abs() < 1e-6, "r: expected 0.8, got {r}");
        assert!((g - 0.5).abs() < 1e-6, "g: expected 0.5, got {g}");
        assert!((b - 0.3).abs() < 1e-6, "b: expected 0.3, got {b}");
    }

    #[test]
    fn vignette_corner_darkened() {
        let (r, _g, _b) = super::apply_vignette(0.8, 0.5, 0.3, -50.0, super::VignetteShape::Elliptical, 0, 0, 100, 100);
        assert!(r < 0.8, "Corner should be darkened, got r={r}");
    }

    #[test]
    fn vignette_corner_brightened() {
        let (r, _g, _b) = super::apply_vignette(0.5, 0.5, 0.5, 50.0, super::VignetteShape::Elliptical, 0, 0, 100, 100);
        assert!(r > 0.5, "Corner should be brightened, got r={r}");
    }

    #[test]
    fn vignette_circular_top_bottom_darker_than_sides() {
        // 3:2 wide image (300x200). Circular radius = max(150, 100) = 150.
        // Left-center (0, 100): dx=150, dy=0 → d²=(150/150)²=1.0 → factor=0 → full effect.
        // Top-center (150, 0): dx=0, dy=100 → d²=(100/150)²=0.444 → factor=(0.556)²=0.309.
        // Left/right edges are further from center than top/bottom in circular mode on a wide image.
        let (r_top, _, _) = super::apply_vignette(0.8, 0.8, 0.8, -100.0, super::VignetteShape::Circular, 150, 0, 300, 200);
        let (r_left, _, _) = super::apply_vignette(0.8, 0.8, 0.8, -100.0, super::VignetteShape::Circular, 0, 100, 300, 200);
        assert!(r_left < r_top, "Circular: left edge ({r_left}) should be darker than top edge ({r_top}) on wide image");
    }

    #[test]
    fn vignette_elliptical_edges_even() {
        // 3:2 aspect ratio image (300x200). Elliptical mode: normalized by half_w and half_h.
        // Top-center (150, 0): d² = (0/150)² + (100/100)² = 1.0
        // Left-center (0, 100): d² = (150/150)² + (0/100)² = 1.0
        // Both should have the same darkening.
        let (r_top, _, _) = super::apply_vignette(0.8, 0.8, 0.8, -50.0, super::VignetteShape::Elliptical, 150, 0, 300, 200);
        let (r_left, _, _) = super::apply_vignette(0.8, 0.8, 0.8, -50.0, super::VignetteShape::Elliptical, 0, 100, 300, 200);
        let (r_bottom, _, _) = super::apply_vignette(0.8, 0.8, 0.8, -50.0, super::VignetteShape::Elliptical, 150, 199, 300, 200);
        let (r_right, _, _) = super::apply_vignette(0.8, 0.8, 0.8, -50.0, super::VignetteShape::Elliptical, 299, 100, 300, 200);
        let eps = 0.02; // small tolerance for edge pixel asymmetry
        assert!((r_top - r_left).abs() < eps, "Top ({r_top}) and left ({r_left}) should be equal");
        assert!((r_top - r_bottom).abs() < eps, "Top ({r_top}) and bottom ({r_bottom}) should be equal");
        assert!((r_top - r_right).abs() < eps, "Top ({r_top}) and right ({r_right}) should be equal");
    }
}
