//! Pure-function adjustment math: per-pixel, dehaze, denoise, detail, grain, color grading, tone curves, vignette.

use palette::{LinSrgb, Srgb};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Detail pass: sharpening, clarity, and texture.
pub mod detail;
pub use detail::{DetailParams, SharpeningParams};

/// Dehaze adjustment.
pub mod dehaze;
pub use dehaze::DehazeParams;

/// Noise reduction.
pub mod denoise;
pub use denoise::NoiseReductionParams;

pub mod grain;
pub use grain::{GrainParams, GrainType};

/// Exposure adjustment (linear space).
pub mod exposure;
pub use exposure::{apply_exposure, exposure_factor};

/// White balance (temperature + tint) adjustment (linear space).
pub mod white_balance;
pub use white_balance::apply_white_balance;

/// Basic tone sliders: contrast, highlights, shadows, whites, blacks (sRGB gamma space).
pub mod basic_tone;
pub use basic_tone::{apply_blacks, apply_contrast, apply_highlights, apply_shadows, apply_whites};

/// HSL (hue / saturation / luminance) per-color-band adjustments.
pub mod hsl;
pub use hsl::{apply_hsl, cosine_weight, hue_distance, WeightFn};

/// Three-way (shadows / midtones / highlights) color grading (sRGB gamma space).
pub mod color_grading;
pub use color_grading::{
    apply_color_grading_pre, ColorGradingParams, ColorGradingPrecomputed, ColorWheel,
};

/// Tone curves (master RGB, per-channel, and luminance; Fritsch-Carlson LUTs).
pub mod tone_curves;
pub(crate) use tone_curves::build_tone_curve_lut;
pub use tone_curves::{apply_tone_curves_pre, ToneCurve, ToneCurveParams, ToneCurvePrecomputed};

// --- Luminance coefficients (Rec. 709) ---

pub(crate) const LUMA_R: f32 = 0.2126;
pub(crate) const LUMA_G: f32 = 0.7152;
pub(crate) const LUMA_B: f32 = 0.0722;

// --- Channel helpers ---

/// Apply a per-channel adjustment function to all three channels.
#[inline(always)]
pub fn apply_per_channel(r: f32, g: f32, b: f32, f: impl Fn(f32) -> f32) -> (f32, f32, f32) {
    (f(r), f(g), f(b))
}

/// Hermite smoothstep: 0 at edge0, 1 at edge1, smooth cubic transition.
#[inline]
pub(crate) fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

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

/// Apply white balance and exposure to a linear buffer in-place.
///
/// Each pixel gets WB channel multipliers (normalized to preserve brightness)
/// followed by exposure (multiply by 2^stops). Operates in linear space.
pub fn apply_white_balance_exposure_buffer(
    buf: &mut [[f32; 3]],
    temperature: f32,
    tint: f32,
    exposure: f32,
) {
    let factor = exposure_factor(exposure);
    for pixel in buf.iter_mut() {
        let (r, g, b) = apply_white_balance(pixel[0], pixel[1], pixel[2], temperature, tint);
        let (r, g, b) = apply_per_channel(r, g, b, |v| apply_exposure(v, factor));
        *pixel = [r, g, b];
    }
}

// --- Per-pixel adjustments (sRGB gamma space) ---

/// All per-pixel parameters needed for the sRGB gamma-space adjustment pass.
///
/// The `lut_fn` closure abstracts over the LUT lookup so that `adjust`
/// does not depend on the `lut` module (architecture rule).
pub struct PerPixelParams<'a> {
    /// Contrast adjustment (range: -100 to +100, default: 0).
    pub contrast: f32,
    /// Highlight recovery / boost (range: -100 to +100, default: 0).
    pub highlights: f32,
    /// Shadow lift / deepen (range: -100 to +100, default: 0).
    pub shadows: f32,
    /// White point adjustment (range: -100 to +100, default: 0).
    pub whites: f32,
    /// Black point adjustment (range: -100 to +100, default: 0).
    pub blacks: f32,
    /// Precomputed tone curve lookup, if active.
    pub tone_curve_pre: Option<&'a ToneCurvePrecomputed>,
    /// Whether any HSL channel has a non-zero shift.
    pub hsl_active: bool,
    /// Per-channel hue shifts indexed by color channel.
    pub hue_shifts: [f32; 8],
    /// Per-channel saturation shifts indexed by color channel.
    pub sat_shifts: [f32; 8],
    /// Per-channel luminance shifts indexed by color channel.
    pub lum_shifts: [f32; 8],
    /// Precomputed color grading data, if active.
    pub color_grading_pre: Option<ColorGradingPrecomputed>,
    /// Optional LUT lookup closure (abstracts over the `lut` module).
    #[allow(clippy::type_complexity)]
    pub lut_fn: Option<&'a (dyn Fn(f32, f32, f32) -> (f32, f32, f32) + Sync + 'a)>,
}

/// Apply all per-pixel adjustments to an sRGB gamma buffer in-place.
///
/// Processes contrast, highlights, shadows, whites, blacks, tone curves,
/// HSL, color grading, and LUT in that order. Operates in sRGB gamma space.
pub fn apply_per_pixel_adjustments(buf: &mut [[f32; 3]], pp: &PerPixelParams) {
    buf.par_chunks_mut(1024).for_each(|chunk| {
        for pixel in chunk.iter_mut() {
            let [mut sr, mut sg, mut sb] = *pixel;

            if pp.contrast != 0.0 {
                (sr, sg, sb) = apply_per_channel(sr, sg, sb, |v| apply_contrast(v, pp.contrast));
            }
            if pp.highlights != 0.0 {
                (sr, sg, sb) =
                    apply_per_channel(sr, sg, sb, |v| apply_highlights(v, pp.highlights));
            }
            if pp.shadows != 0.0 {
                (sr, sg, sb) = apply_per_channel(sr, sg, sb, |v| apply_shadows(v, pp.shadows));
            }
            if pp.whites != 0.0 {
                (sr, sg, sb) = apply_per_channel(sr, sg, sb, |v| apply_whites(v, pp.whites));
            }
            if pp.blacks != 0.0 {
                (sr, sg, sb) = apply_per_channel(sr, sg, sb, |v| apply_blacks(v, pp.blacks));
            }
            if let Some(pre) = pp.tone_curve_pre {
                let (tr, tg, tb) = apply_tone_curves_pre(sr, sg, sb, pre);
                sr = tr;
                sg = tg;
                sb = tb;
            }
            if pp.hsl_active {
                let (hr, hg, hb) = apply_hsl(
                    sr,
                    sg,
                    sb,
                    &pp.hue_shifts,
                    &pp.sat_shifts,
                    &pp.lum_shifts,
                    cosine_weight,
                );
                sr = hr;
                sg = hg;
                sb = hb;
            }
            if let Some(ref pre) = pp.color_grading_pre {
                let (cr, cg, cb) = apply_color_grading_pre(sr, sg, sb, pre);
                sr = cr;
                sg = cg;
                sb = cb;
            }
            if let Some(lut_fn) = pp.lut_fn {
                let (lr, lg, lb) = lut_fn(sr, sg, sb);
                sr = lr;
                sg = lg;
                sb = lb;
            }

            *pixel = [sr, sg, sb];
        }
    });
}

// --- Vignette (sRGB gamma space, position-dependent) ---

/// Vignette falloff geometry.
#[cfg_attr(feature = "docgen", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VignetteShape {
    /// Elliptical falloff matching the image aspect ratio (default).
    #[default]
    Elliptical,
    /// Circular falloff centered on the image.
    Circular,
}

impl std::fmt::Display for VignetteShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Elliptical => write!(f, "elliptical"),
            Self::Circular => write!(f, "circular"),
        }
    }
}

impl std::str::FromStr for VignetteShape {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "elliptical" => Ok(Self::Elliptical),
            "circular" => Ok(Self::Circular),
            _ => Err(format!(
                "invalid vignette shape '{s}'. Use: elliptical or circular"
            )),
        }
    }
}

/// Precomputed loop-invariant values for vignette rendering.
///
/// Create once per render via [`VignettePrecomputed::new`], then call
/// [`apply_vignette_pre`] per pixel. This avoids recomputing `half_w`,
/// `half_h`, `strength`, and per-axis reciprocals on every pixel.
#[derive(Debug, Clone, Copy)]
pub struct VignettePrecomputed {
    half_w: f32,
    half_h: f32,
    inv_x: f32,
    inv_y: f32,
    strength: f32,
}

impl VignettePrecomputed {
    /// Precompute vignette geometry from amount, shape, and image dimensions.
    pub fn new(amount: f32, shape: VignetteShape, w: u32, h: u32) -> Self {
        let half_w = w as f32 / 2.0;
        let half_h = h as f32 / 2.0;
        let (inv_x, inv_y) = match shape {
            VignetteShape::Elliptical => (1.0 / half_w, 1.0 / half_h),
            VignetteShape::Circular => {
                let inv_r = 1.0 / half_w.max(half_h);
                (inv_r, inv_r)
            }
        };
        Self {
            half_w,
            half_h,
            inv_x,
            inv_y,
            strength: amount / 100.0,
        }
    }
}

/// Apply creative vignette using precomputed invariants (hot path).
///
/// Call [`VignettePrecomputed::new`] once, then this function per pixel.
pub fn apply_vignette_pre(
    r: f32,
    g: f32,
    b: f32,
    pre: &VignettePrecomputed,
    x: u32,
    y: u32,
) -> (f32, f32, f32) {
    let dx = (x as f32 - pre.half_w) * pre.inv_x;
    let dy = (y as f32 - pre.half_h) * pre.inv_y;
    let d_sq = dx * dx + dy * dy;

    let base = (1.0 - d_sq).clamp(0.0, 1.0);
    let factor = base * base;
    let multiplier = 1.0 + pre.strength * (1.0 - factor);

    (
        (r * multiplier).clamp(0.0, 1.0),
        (g * multiplier).clamp(0.0, 1.0),
        (b * multiplier).clamp(0.0, 1.0),
    )
}

/// Apply creative vignette to an sRGB gamma pixel (convenience wrapper).
///
/// Darkens (negative amount) or brightens (positive amount) edges based on
/// distance from center. Amount range: -100 to +100. 0 = no effect.
///
/// For batch pixel processing, prefer [`VignettePrecomputed`] + [`apply_vignette_pre`].
#[allow(clippy::too_many_arguments)]
pub fn apply_vignette(
    r: f32,
    g: f32,
    b: f32,
    amount: f32,
    shape: VignetteShape,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> (f32, f32, f32) {
    if amount == 0.0 {
        return (r, g, b);
    }
    apply_vignette_pre(
        r,
        g,
        b,
        &VignettePrecomputed::new(amount, shape, w, h),
        x,
        y,
    )
}

/// Apply vignette to an sRGB gamma buffer in-place using precomputed invariants.
pub fn apply_vignette_buffer(
    buf: &mut [[f32; 3]],
    width: u32,
    height: u32,
    pre: &VignettePrecomputed,
) {
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let [r, g, b] = buf[idx];
            let (r, g, b) = apply_vignette_pre(r, g, b, pre, x, y);
            buf[idx] = [r, g, b];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Color space roundtrip ---

    #[test]
    fn linear_srgb_roundtrip() {
        let (sr, sg, sb) = linear_to_srgb(0.5, 0.3, 0.1);
        let (lr, lg, lb) = srgb_to_linear(sr, sg, sb);
        assert!((lr - 0.5).abs() < 1e-5);
        assert!((lg - 0.3).abs() < 1e-5);
        assert!((lb - 0.1).abs() < 1e-5);
    }

    // --- Vignette tests ---

    #[test]
    fn vignette_zero_amount_is_identity() {
        let (r, g, b) = super::apply_vignette(
            0.8,
            0.5,
            0.3,
            0.0,
            super::VignetteShape::Elliptical,
            0,
            0,
            100,
            100,
        );
        assert!((r - 0.8).abs() < 1e-6);
        assert!((g - 0.5).abs() < 1e-6);
        assert!((b - 0.3).abs() < 1e-6);
    }

    #[test]
    fn vignette_center_pixel_unchanged() {
        // 100x100 image: half_w = 50.0. Pixel (50, 50) → dx = 0, dy = 0 → factor = 1.0 exactly.
        let (r, g, b) = super::apply_vignette(
            0.8,
            0.5,
            0.3,
            -50.0,
            super::VignetteShape::Elliptical,
            50,
            50,
            100,
            100,
        );
        assert!((r - 0.8).abs() < 1e-6, "r: expected 0.8, got {r}");
        assert!((g - 0.5).abs() < 1e-6, "g: expected 0.5, got {g}");
        assert!((b - 0.3).abs() < 1e-6, "b: expected 0.3, got {b}");
    }

    #[test]
    fn vignette_corner_darkened() {
        let (r, _g, _b) = super::apply_vignette(
            0.8,
            0.5,
            0.3,
            -50.0,
            super::VignetteShape::Elliptical,
            0,
            0,
            100,
            100,
        );
        assert!(r < 0.8, "Corner should be darkened, got r={r}");
    }

    #[test]
    fn vignette_corner_brightened() {
        let (r, _g, _b) = super::apply_vignette(
            0.5,
            0.5,
            0.5,
            50.0,
            super::VignetteShape::Elliptical,
            0,
            0,
            100,
            100,
        );
        assert!(r > 0.5, "Corner should be brightened, got r={r}");
    }

    #[test]
    fn vignette_circular_top_bottom_darker_than_sides() {
        // 3:2 wide image (300x200). Circular radius = max(150, 100) = 150.
        // Left-center (0, 100): dx=150, dy=0 → d²=(150/150)²=1.0 → factor=0 → full effect.
        // Top-center (150, 0): dx=0, dy=100 → d²=(100/150)²=0.444 → factor=(0.556)²=0.309.
        // Left/right edges are further from center than top/bottom in circular mode on a wide image.
        let (r_top, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -100.0,
            super::VignetteShape::Circular,
            150,
            0,
            300,
            200,
        );
        let (r_left, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -100.0,
            super::VignetteShape::Circular,
            0,
            100,
            300,
            200,
        );
        assert!(
            r_left < r_top,
            "Circular: left edge ({r_left}) should be darker than top edge ({r_top}) on wide image"
        );
    }

    #[test]
    fn vignette_elliptical_edges_even() {
        // 3:2 aspect ratio image (300x200). Elliptical mode: normalized by half_w and half_h.
        // Top-center (150, 0): d² = (0/150)² + (100/100)² = 1.0
        // Left-center (0, 100): d² = (150/150)² + (0/100)² = 1.0
        // Both should have the same darkening.
        let (r_top, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -50.0,
            super::VignetteShape::Elliptical,
            150,
            0,
            300,
            200,
        );
        let (r_left, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -50.0,
            super::VignetteShape::Elliptical,
            0,
            100,
            300,
            200,
        );
        let (r_bottom, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -50.0,
            super::VignetteShape::Elliptical,
            150,
            199,
            300,
            200,
        );
        let (r_right, _, _) = super::apply_vignette(
            0.8,
            0.8,
            0.8,
            -50.0,
            super::VignetteShape::Elliptical,
            299,
            100,
            300,
            200,
        );
        let eps = 0.02; // small tolerance for edge pixel asymmetry
        assert!(
            (r_top - r_left).abs() < eps,
            "Top ({r_top}) and left ({r_left}) should be equal"
        );
        assert!(
            (r_top - r_bottom).abs() < eps,
            "Top ({r_top}) and bottom ({r_bottom}) should be equal"
        );
        assert!(
            (r_top - r_right).abs() < eps,
            "Top ({r_top}) and right ({r_right}) should be equal"
        );
    }

    // --- Tone Curve tests ---

    #[test]
    fn white_balance_exposure_buffer_identity() {
        let mut buf = vec![[0.5, 0.3, 0.1], [0.25, 0.25, 0.25]];
        let original = buf.clone();
        apply_white_balance_exposure_buffer(&mut buf, 0.0, 0.0, 0.0);
        for i in 0..buf.len() {
            for c in 0..3 {
                assert!(
                    (buf[i][c] - original[i][c]).abs() < 1e-6,
                    "pixel[{}][{}] changed with neutral params",
                    i,
                    c
                );
            }
        }
    }

    #[test]
    fn white_balance_exposure_buffer_applies_exposure() {
        let mut buf = vec![[0.25, 0.25, 0.25]];
        apply_white_balance_exposure_buffer(&mut buf, 0.0, 0.0, 1.0);
        for (c, &v) in buf[0].iter().enumerate() {
            assert!((v - 0.5).abs() < 1e-5, "channel {c}: expected 0.5, got {v}");
        }
    }

    #[test]
    fn white_balance_exposure_buffer_applies_wb() {
        let mut buf = vec![[0.5, 0.5, 0.5]];
        apply_white_balance_exposure_buffer(&mut buf, 50.0, 0.0, 0.0);
        assert!(buf[0][0] > buf[0][2], "warm WB should make red > blue");
    }

    // --- Per-pixel adjustments tests ---

    #[test]
    fn per_pixel_adjustments_neutral_is_identity() {
        let mut buf = vec![[0.7, 0.5, 0.3]]; // values already in sRGB gamma
        let original = buf.clone();
        let pp = PerPixelParams {
            contrast: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            tone_curve_pre: None,
            hsl_active: false,
            hue_shifts: [0.0; 8],
            sat_shifts: [0.0; 8],
            lum_shifts: [0.0; 8],
            color_grading_pre: None,
            lut_fn: None,
        };
        apply_per_pixel_adjustments(&mut buf, &pp);
        for c in 0..3 {
            assert!(
                (buf[0][c] - original[0][c]).abs() < 1e-6,
                "channel {} changed with neutral params",
                c
            );
        }
    }

    #[test]
    fn per_pixel_adjustments_applies_contrast() {
        let mut buf = vec![[0.8, 0.8, 0.8]]; // above midpoint in sRGB
        let pp = PerPixelParams {
            contrast: 50.0,
            highlights: 0.0,
            shadows: 0.0,
            whites: 0.0,
            blacks: 0.0,
            tone_curve_pre: None,
            hsl_active: false,
            hue_shifts: [0.0; 8],
            sat_shifts: [0.0; 8],
            lum_shifts: [0.0; 8],
            color_grading_pre: None,
            lut_fn: None,
        };
        apply_per_pixel_adjustments(&mut buf, &pp);
        // Positive contrast should push values above 0.5 higher
        assert!(
            buf[0][0] > 0.8,
            "contrast should increase value above midpoint"
        );
    }

    #[test]
    fn vignette_buffer_darkens_corners() {
        let w = 4u32;
        let h = 4u32;
        let mut buf: Vec<[f32; 3]> = vec![[0.5, 0.5, 0.5]; (w * h) as usize];
        let pre = VignettePrecomputed::new(-50.0, VignetteShape::Elliptical, w, h);
        apply_vignette_buffer(&mut buf, w, h, &pre);
        // Center pixel should be unchanged (or close)
        let center = buf[(w + 1) as usize];
        // Corner pixel should be darker
        let corner = buf[0];
        assert!(
            corner[0] < center[0],
            "corner ({}) should be darker than center ({})",
            corner[0],
            center[0]
        );
    }
}
