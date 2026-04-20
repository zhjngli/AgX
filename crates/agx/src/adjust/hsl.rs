#![doc = include_str!("hsl.md")]

use palette::{Hsl, IntoColor, Srgb};

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
