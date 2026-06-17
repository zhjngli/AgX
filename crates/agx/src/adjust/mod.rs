//! Pure-function adjustment math: per-pixel and per-image algorithms (exposure, white balance, basic tone, HSL, color grading, tone curves, vignette, detail, dehaze, denoise, grain).

use palette::{LinSrgb, Srgb};
use rayon::prelude::*;

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

/// Basic tone sliders: contrast, highlights, shadows, whites, blacks (gamma Rec.2020 working space).
pub mod basic_tone;
pub use basic_tone::{apply_blacks, apply_contrast, apply_highlights, apply_shadows, apply_whites};

/// HSL (hue / saturation / luminance) per-color-band adjustments.
pub mod hsl;
pub use hsl::{apply_hsl, cosine_weight, hue_distance, WeightFn};

/// Three-way (shadows / midtones / highlights) color grading (gamma Rec.2020 working space).
pub mod color_grading;
pub use color_grading::{
    apply_color_grading_pre, ColorGradingParams, ColorGradingPrecomputed, ColorWheel,
};

/// Tone curves (master RGB, per-channel, and luminance; Fritsch-Carlson LUTs).
pub mod tone_curves;
pub(crate) use tone_curves::build_tone_curve_lut;
pub use tone_curves::{apply_tone_curves_pre, ToneCurve, ToneCurveParams, ToneCurvePrecomputed};

/// Vignette: position-dependent per-pixel lightness multiplier.
pub mod vignette;
pub use vignette::{
    apply_vignette, apply_vignette_buffer, apply_vignette_pre, VignettePrecomputed, VignetteShape,
};

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

/// Convert a linear sRGB f32 pixel to sRGB-gamma. Pre-migration this was
/// the engine's working-space transfer; post-migration the engine uses
/// `crate::color_space::srgb_curve_signed`. This helper remains for the
/// encode-side matrix-then-curve fused pass and as the inner sRGB step
/// of the LUT-wrap bracket — both call sites are intentionally working
/// in sRGB primaries, not Rec.2020.
pub fn linear_to_srgb(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let srgb: Srgb<f32> = LinSrgb::new(r, g, b).into_encoding();
    (srgb.red, srgb.green, srgb.blue)
}

/// Convert an sRGB-gamma f32 pixel to linear sRGB. Pre-migration this
/// was the engine's working-space transfer; post-migration the engine
/// uses `crate::color_space::srgb_curve_signed_inverse`. This helper
/// remains for the encode-side fused pass and as the inner sRGB step
/// of the LUT-wrap bracket — both call sites are intentionally working
/// in sRGB primaries, not Rec.2020.
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

// --- Per-pixel adjustments (gamma Rec.2020 working space) ---

/// All per-pixel parameters needed for the gamma Rec.2020 working-space adjustment pass.
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
}

/// Apply all per-pixel adjustments to a gamma Rec.2020 buffer in-place.
///
/// Processes contrast, highlights, shadows, whites, blacks, tone curves,
/// HSL, color grading, and LUT in that order. Operates in the gamma Rec.2020
/// working space.
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

            *pixel = [sr, sg, sb];
        }
    });
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
        };
        apply_per_pixel_adjustments(&mut buf, &pp);
        // Positive contrast should push values above 0.5 higher
        assert!(
            buf[0][0] > 0.8,
            "contrast should increase value above midpoint"
        );
    }
}
