#![doc = include_str!("color_grading.md")]

use super::{LUMA_B, LUMA_G, LUMA_R};
use serde::{Deserialize, Serialize};

// --- Color Grading (sRGB gamma space) ---

/// A single color wheel with hue, saturation, and luminance.
///
/// Used for shadows, midtones, highlights, and global wheels in color grading.
/// Hue: 0-360 degrees, Saturation: 0-100, Luminance: -100 to +100.
#[cfg_attr(
    any(feature = "docgen", feature = "validate"),
    derive(schemars::JsonSchema)
)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ColorWheel {
    /// Hue angle in degrees (0–360).
    #[serde(default)]
    #[cfg_attr(
        any(feature = "docgen", feature = "validate"),
        schemars(range(min = 0.0, max = 360.0))
    )]
    pub hue: f32,
    /// Saturation amount (0–100, default: 0).
    #[serde(default)]
    #[cfg_attr(
        any(feature = "docgen", feature = "validate"),
        schemars(range(min = 0.0, max = 100.0))
    )]
    pub saturation: f32,
    /// Luminance shift (range: -100 to +100, default: 0).
    #[serde(default)]
    #[cfg_attr(any(feature = "docgen", feature = "validate"), schemars(range(min = -100.0, max = 100.0)))]
    pub luminance: f32,
}

/// 3-way color grading parameters (shadows, midtones, highlights, global + balance).
#[cfg_attr(
    any(feature = "docgen", feature = "validate"),
    derive(schemars::JsonSchema)
)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ColorGradingParams {
    /// Shadow tones color wheel.
    #[serde(default)]
    pub shadows: ColorWheel,
    /// Midtone color wheel.
    #[serde(default)]
    pub midtones: ColorWheel,
    /// Highlight tones color wheel.
    #[serde(default)]
    pub highlights: ColorWheel,
    /// Global color wheel (applied uniformly).
    #[serde(default)]
    pub global: ColorWheel,
    /// Shadow/highlight balance point (range: -100 to +100, default: 0).
    #[serde(default)]
    #[cfg_attr(any(feature = "docgen", feature = "validate"), schemars(range(min = -100.0, max = 100.0)))]
    pub balance: f32,
}

impl ColorGradingParams {
    /// Returns `true` when all fields are at their default (neutral) values.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Precomputed loop-invariant values for color grading.
///
/// Create once per render via [`ColorGradingPrecomputed::new`], then call
/// [`apply_color_grading_pre`] per pixel.
#[derive(Debug, Clone, Copy)]
pub struct ColorGradingPrecomputed {
    shadow_tint: [f32; 3],
    midtone_tint: [f32; 3],
    highlight_tint: [f32; 3],
    global_tint: [f32; 3],
    shadow_lum: f32,
    midtone_lum: f32,
    highlight_lum: f32,
    global_lum: f32,
    balance_factor: f32,
    balance_active: bool,
}

impl ColorGradingPrecomputed {
    fn wheel_to_tint(wheel: &ColorWheel) -> [f32; 3] {
        let hue_rad = wheel.hue * std::f32::consts::PI / 180.0;
        let sat = wheel.saturation / 100.0;
        [
            1.0 + sat * hue_rad.cos(),
            1.0 + sat * (hue_rad - 2.0 * std::f32::consts::PI / 3.0).cos(),
            1.0 + sat * (hue_rad - 4.0 * std::f32::consts::PI / 3.0).cos(),
        ]
    }

    /// Precompute tints and luminance values from the given parameters.
    pub fn new(params: &ColorGradingParams) -> Self {
        Self {
            shadow_tint: Self::wheel_to_tint(&params.shadows),
            midtone_tint: Self::wheel_to_tint(&params.midtones),
            highlight_tint: Self::wheel_to_tint(&params.highlights),
            global_tint: Self::wheel_to_tint(&params.global),
            shadow_lum: params.shadows.luminance / 100.0,
            midtone_lum: params.midtones.luminance / 100.0,
            highlight_lum: params.highlights.luminance / 100.0,
            global_lum: params.global.luminance / 100.0,
            balance_factor: 2.0_f32.powf(-params.balance / 100.0),
            balance_active: params.balance != 0.0,
        }
    }
}

/// Apply 3-way color grading using precomputed invariants (hot path).
///
/// Operates in sRGB gamma space. Uses Rec. 709 luminance coefficients on
/// gamma-encoded values as a perceptual approximation for weight computation.
#[inline]
pub fn apply_color_grading_pre(
    r: f32,
    g: f32,
    b: f32,
    pre: &ColorGradingPrecomputed,
) -> (f32, f32, f32) {
    // Pixel luminance (Rec. 709 on gamma-encoded values)
    let lum = LUMA_R * r + LUMA_G * g + LUMA_B * b;

    // Balance remapping (skip powf when balance is neutral)
    let lum_adj = if pre.balance_active {
        // Luma may be negative or > 1 for wide-gamut inputs. Clamp here is
        // domain-safety: powf with negative base produces NaN.
        lum.clamp(0.0, 1.0).powf(pre.balance_factor)
    } else {
        // Luma used as a weight for shadow/midtone/highlight contribution.
        // Clamp here is domain-safety: weights would invert with negative luma
        // or sum incorrectly above 1.
        lum.clamp(0.0, 1.0)
    };

    // 3-way weights (always sum to 1.0)
    let w_shadow = (1.0 - lum_adj) * (1.0 - lum_adj);
    let w_highlight = lum_adj * lum_adj;
    let w_midtone = 1.0 - w_shadow - w_highlight;

    // Weighted blend of regional tints
    let regional_r = pre.shadow_tint[0] * w_shadow
        + pre.midtone_tint[0] * w_midtone
        + pre.highlight_tint[0] * w_highlight;
    let regional_g = pre.shadow_tint[1] * w_shadow
        + pre.midtone_tint[1] * w_midtone
        + pre.highlight_tint[1] * w_highlight;
    let regional_b = pre.shadow_tint[2] * w_shadow
        + pre.midtone_tint[2] * w_midtone
        + pre.highlight_tint[2] * w_highlight;

    // Apply global tint on top
    let combined_r = regional_r * pre.global_tint[0];
    let combined_g = regional_g * pre.global_tint[1];
    let combined_b = regional_b * pre.global_tint[2];

    // Multiply pixel by combined tint; output unclamped — wide-gamut headroom
    // survives this stage, final clamp is at encode.
    let mut out_r = r * combined_r;
    let mut out_g = g * combined_g;
    let mut out_b = b * combined_b;

    // Luminance shifts (weighted additive, pre-divided by 100); unclamped.
    let adjustment = pre.shadow_lum * w_shadow
        + pre.midtone_lum * w_midtone
        + pre.highlight_lum * w_highlight
        + pre.global_lum;
    out_r += adjustment;
    out_g += adjustment;
    out_b += adjustment;

    (out_r, out_g, out_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_grading_default_is_identity() {
        let params = ColorGradingParams::default();
        assert!(params.is_default());
        assert_eq!(params.balance, 0.0);
        assert_eq!(params.shadows.hue, 0.0);
        assert_eq!(params.shadows.saturation, 0.0);
        assert_eq!(params.shadows.luminance, 0.0);
    }

    #[test]
    fn color_grading_default_no_change() {
        let params = ColorGradingParams::default();
        let pre = ColorGradingPrecomputed::new(&params);
        let (r, g, b) = apply_color_grading_pre(0.5, 0.5, 0.5, &pre);
        assert!((r - 0.5).abs() < 1e-6);
        assert!((g - 0.5).abs() < 1e-6);
        assert!((b - 0.5).abs() < 1e-6);
    }

    #[test]
    fn color_grading_shadow_teal_shifts_dark_pixels() {
        let mut params = ColorGradingParams::default();
        params.shadows.hue = 180.0; // cyan/teal
        params.shadows.saturation = 50.0;
        let pre = ColorGradingPrecomputed::new(&params);

        // Dark pixel (shadows region)
        let (r, g, b) = apply_color_grading_pre(0.1, 0.1, 0.1, &pre);
        assert!(g > r, "green should exceed red for teal tint on dark pixel");
        assert!(b > r, "blue should exceed red for teal tint on dark pixel");

        // Bright pixel (highlights region) — should be mostly unaffected
        let (r2, g2, b2) = apply_color_grading_pre(0.9, 0.9, 0.9, &pre);
        let shift = ((r2 - 0.9).abs() + (g2 - 0.9).abs() + (b2 - 0.9).abs()) / 3.0;
        assert!(
            shift < 0.05,
            "bright pixel should be mostly unaffected by shadow tint"
        );
    }

    #[test]
    fn color_grading_highlight_orange_shifts_bright_pixels() {
        let mut params = ColorGradingParams::default();
        params.highlights.hue = 30.0; // orange
        params.highlights.saturation = 50.0;
        let pre = ColorGradingPrecomputed::new(&params);

        // Bright pixel
        let (r, g, b) = apply_color_grading_pre(0.9, 0.9, 0.9, &pre);
        assert!(
            r > g,
            "red should exceed green for orange tint on bright pixel"
        );
        assert!(
            g > b,
            "green should exceed blue for orange tint on bright pixel"
        );

        // Dark pixel — should be mostly unaffected
        let (r2, g2, b2) = apply_color_grading_pre(0.1, 0.1, 0.1, &pre);
        let shift = ((r2 - 0.1).abs() + (g2 - 0.1).abs() + (b2 - 0.1).abs()) / 3.0;
        assert!(
            shift < 0.05,
            "dark pixel should be mostly unaffected by highlight tint"
        );
    }

    #[test]
    fn color_grading_midtone_affects_mid_pixels() {
        let mut params = ColorGradingParams::default();
        params.midtones.hue = 120.0; // green
        params.midtones.saturation = 50.0;
        let pre = ColorGradingPrecomputed::new(&params);

        // Mid-luminance pixel — should show green tint
        let (r, g, b) = apply_color_grading_pre(0.5, 0.5, 0.5, &pre);
        assert!(
            g > r,
            "green should exceed red for green midtone tint on mid pixel"
        );
        assert!(
            g > b,
            "green should exceed blue for green midtone tint on mid pixel"
        );

        // Dark pixel — should be mostly unaffected
        let (r2, g2, _) = apply_color_grading_pre(0.05, 0.05, 0.05, &pre);
        let shift_dark = (g2 - r2).abs();
        // Bright pixel — should be mostly unaffected
        let (r3, g3, _) = apply_color_grading_pre(0.95, 0.95, 0.95, &pre);
        let shift_bright = (g3 - r3).abs();
        // Mid pixel shift should be larger than extremes
        let shift_mid = (g - r).abs();
        assert!(
            shift_mid > shift_dark,
            "midtone tint should affect mid pixels more than dark"
        );
        assert!(
            shift_mid > shift_bright,
            "midtone tint should affect mid pixels more than bright"
        );
    }

    #[test]
    fn color_grading_global_tint_affects_all() {
        let mut params = ColorGradingParams::default();
        params.global.hue = 0.0; // red
        params.global.saturation = 50.0;
        let pre = ColorGradingPrecomputed::new(&params);

        let (r1, _, b1) = apply_color_grading_pre(0.2, 0.2, 0.2, &pre);
        assert!(r1 > b1, "global red tint on dark pixel");

        let (r2, _, b2) = apply_color_grading_pre(0.5, 0.5, 0.5, &pre);
        assert!(r2 > b2, "global red tint on mid pixel");

        let (r3, _, b3) = apply_color_grading_pre(0.8, 0.8, 0.8, &pre);
        assert!(r3 > b3, "global red tint on bright pixel");
    }

    #[test]
    fn color_grading_saturation_zero_no_color_effect() {
        let mut params = ColorGradingParams::default();
        params.shadows.hue = 200.0;
        params.shadows.saturation = 0.0;
        let pre = ColorGradingPrecomputed::new(&params);
        let (r, g, b) = apply_color_grading_pre(0.1, 0.1, 0.1, &pre);
        assert!((r - 0.1).abs() < 1e-6);
        assert!((g - 0.1).abs() < 1e-6);
        assert!((b - 0.1).abs() < 1e-6);
    }

    #[test]
    fn color_grading_luminance_weight_sum() {
        for i in 0..=100 {
            let lum = i as f32 / 100.0;
            let w_shadow = (1.0 - lum) * (1.0 - lum);
            let w_highlight = lum * lum;
            let w_midtone = 1.0 - w_shadow - w_highlight;
            let sum = w_shadow + w_midtone + w_highlight;
            assert!(
                (sum - 1.0).abs() < 1e-6,
                "weights must sum to 1.0, got {} at lum={}",
                sum,
                lum
            );
        }
    }

    #[test]
    fn color_grading_preserves_out_of_range_values() {
        // OOG input (r=1.3) with a non-trivial highlight tint should not be
        // clamped at output. Wide-gamut headroom must survive the tint multiply
        // and luminance shift stages.
        let mut params = ColorGradingParams::default();
        params.highlights.hue = 30.0;
        params.highlights.saturation = 20.0;
        params.highlights.luminance = 10.0;
        let pre = ColorGradingPrecomputed::new(&params);
        let (r, _g, _b) = apply_color_grading_pre(1.3, 0.9, 0.8, &pre);
        assert!(r > 1.0, "color_grading clamped OOG value: r={r}");
        assert!(r.is_finite());
    }

    #[test]
    fn color_grading_balance_shifts_weights() {
        let mut params_neg = ColorGradingParams::default();
        params_neg.shadows.hue = 200.0;
        params_neg.shadows.saturation = 50.0;
        params_neg.balance = -50.0;

        let mut params_pos = ColorGradingParams::default();
        params_pos.shadows.hue = 200.0;
        params_pos.shadows.saturation = 50.0;
        params_pos.balance = 50.0;

        let pre_neg = ColorGradingPrecomputed::new(&params_neg);
        let pre_pos = ColorGradingPrecomputed::new(&params_pos);

        let (_, g_neg, _) = apply_color_grading_pre(0.5, 0.5, 0.5, &pre_neg);
        let (_, g_pos, _) = apply_color_grading_pre(0.5, 0.5, 0.5, &pre_pos);
        assert!(
            g_neg > g_pos,
            "negative balance should increase shadow influence on midtones"
        );
    }
}
