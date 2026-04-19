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

// --- Tone Curves ---

/// A single tone curve defined by control points.
/// Points are (input, output) pairs in [0.0, 1.0], sorted by input.
/// First point must have x=0.0, last must have x=1.0.
#[cfg_attr(feature = "docgen", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToneCurve {
    /// Control points as (input, output) pairs in [0.0, 1.0], sorted by input.
    pub points: Vec<(f32, f32)>,
}

impl Default for ToneCurve {
    fn default() -> Self {
        Self {
            points: vec![(0.0, 0.0), (1.0, 1.0)],
        }
    }
}

impl ToneCurve {
    /// Return `true` if this is the identity curve (two endpoints, no adjustment).
    pub fn is_identity(&self) -> bool {
        self.points.len() == 2 && self.points[0] == (0.0, 0.0) && self.points[1] == (1.0, 1.0)
    }

    /// Validate control points: at least 2, endpoints at x=0 and x=1,
    /// all values in \[0,1\], strictly increasing x.
    pub fn validate(&self) -> std::result::Result<(), String> {
        let points = &self.points;
        if points.len() < 2 {
            return Err(format!("need at least 2 points, got {}", points.len()));
        }
        if (points[0].0).abs() > 1e-6 {
            return Err(format!("first point x must be 0.0, got {}", points[0].0));
        }
        if (points.last().unwrap().0 - 1.0).abs() > 1e-6 {
            return Err(format!(
                "last point x must be 1.0, got {}",
                points.last().unwrap().0
            ));
        }
        for &(x, y) in points {
            if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
                return Err(format!("point ({x}, {y}) out of range [0, 1]"));
            }
        }
        for i in 1..points.len() {
            if points[i].0 <= points[i - 1].0 {
                return Err(format!(
                    "x values must be strictly increasing: {} >= {}",
                    points[i].0,
                    points[i - 1].0
                ));
            }
        }
        Ok(())
    }
}

/// Parameters for 5-channel tone curves.
#[cfg_attr(feature = "docgen", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ToneCurveParams {
    /// Master RGB curve applied to all channels.
    #[serde(default)]
    pub rgb: ToneCurve,
    /// Luminance-only curve.
    #[serde(default)]
    pub luma: ToneCurve,
    /// Red channel curve.
    #[serde(default)]
    pub red: ToneCurve,
    /// Green channel curve.
    #[serde(default)]
    pub green: ToneCurve,
    /// Blue channel curve.
    #[serde(default)]
    pub blue: ToneCurve,
}

impl ToneCurveParams {
    /// Return `true` if all five curves are identity (no adjustment).
    pub fn is_default(&self) -> bool {
        self.rgb.is_identity()
            && self.luma.is_identity()
            && self.red.is_identity()
            && self.green.is_identity()
            && self.blue.is_identity()
    }
}

/// Build a 256-entry lookup table from a tone curve using
/// Fritsch-Carlson monotone cubic hermite interpolation.
pub(crate) fn build_tone_curve_lut(curve: &ToneCurve) -> [f32; 256] {
    let pts = &curve.points;
    let n = pts.len();
    debug_assert!(n >= 2);

    // Special case: 2 points = linear interpolation
    if n == 2 {
        let mut lut = [0.0_f32; 256];
        let (x0, y0) = pts[0];
        let (x1, y1) = pts[1];
        let dx = x1 - x0;
        for (i, slot) in lut.iter_mut().enumerate() {
            let t = i as f32 / 255.0;
            let frac = if dx.abs() < 1e-9 { 0.0 } else { (t - x0) / dx };
            *slot = (y0 + frac * (y1 - y0)).clamp(0.0, 1.0);
        }
        return lut;
    }

    // Step 1: Compute slopes between adjacent points
    let mut delta = vec![0.0_f32; n - 1];
    for i in 0..n - 1 {
        let dx = pts[i + 1].0 - pts[i].0;
        delta[i] = if dx.abs() < 1e-9 {
            0.0
        } else {
            (pts[i + 1].1 - pts[i].1) / dx
        };
    }

    // Step 2: Compute initial tangents
    let mut m = vec![0.0_f32; n];
    m[0] = delta[0];
    m[n - 1] = delta[n - 2];
    for i in 1..n - 1 {
        m[i] = (delta[i - 1] + delta[i]) / 2.0;
    }

    // Step 3: Fritsch-Carlson monotonicity constraints
    for i in 0..n - 1 {
        if delta[i].abs() < 1e-9 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
        } else {
            let alpha = m[i] / delta[i];
            let beta = m[i + 1] / delta[i];
            let tau = alpha * alpha + beta * beta;
            if tau > 9.0 {
                let t = 3.0 / tau.sqrt();
                m[i] = t * alpha * delta[i];
                m[i + 1] = t * beta * delta[i];
            }
        }
    }

    // Step 4: Evaluate hermite spline at 256 points
    let mut lut = [0.0_f32; 256];
    let mut seg = 0_usize;
    for (i, slot) in lut.iter_mut().enumerate() {
        let x = i as f32 / 255.0;

        // Advance segment
        while seg < n - 2 && x > pts[seg + 1].0 {
            seg += 1;
        }

        let (x0, y0) = pts[seg];
        let (x1, y1) = pts[seg + 1];
        let h = x1 - x0;
        if h.abs() < 1e-9 {
            *slot = y0;
            continue;
        }

        let t = (x - x0) / h;
        let t2 = t * t;
        let t3 = t2 * t;

        // Hermite basis functions
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;

        *slot = (h00 * y0 + h10 * h * m[seg] + h01 * y1 + h11 * h * m[seg + 1]).clamp(0.0, 1.0);
    }

    lut
}

/// Look up a value in a precomputed 256-entry LUT with linear interpolation.
#[inline(always)]
pub(crate) fn lut_lookup(lut: &[f32; 256], value: f32) -> f32 {
    let idx = value * 255.0;
    let idx = idx.clamp(0.0, 255.0);
    let lo = idx.floor() as usize;
    let hi = (lo + 1).min(255);
    let frac = idx - lo as f32;
    lut[lo] + frac * (lut[hi] - lut[lo])
}

/// Precomputed tone curve LUTs for fast per-pixel application.
#[derive(Clone)]
pub struct ToneCurvePrecomputed {
    rgb: Option<[f32; 256]>,
    luma: Option<[f32; 256]>,
    red: Option<[f32; 256]>,
    green: Option<[f32; 256]>,
    blue: Option<[f32; 256]>,
}

impl ToneCurvePrecomputed {
    /// Precompute 256-entry LUTs for each non-identity curve.
    pub fn new(params: &ToneCurveParams) -> Self {
        Self {
            rgb: (!params.rgb.is_identity()).then(|| build_tone_curve_lut(&params.rgb)),
            luma: (!params.luma.is_identity()).then(|| build_tone_curve_lut(&params.luma)),
            red: (!params.red.is_identity()).then(|| build_tone_curve_lut(&params.red)),
            green: (!params.green.is_identity()).then(|| build_tone_curve_lut(&params.green)),
            blue: (!params.blue.is_identity()).then(|| build_tone_curve_lut(&params.blue)),
        }
    }
}

/// Apply tone curves to a pixel. Order: RGB master -> per-channel -> luminance.
#[inline]
pub fn apply_tone_curves_pre(
    mut r: f32,
    mut g: f32,
    mut b: f32,
    pre: &ToneCurvePrecomputed,
) -> (f32, f32, f32) {
    // Step 1: RGB master curve
    if let Some(ref lut) = pre.rgb {
        r = lut_lookup(lut, r);
        g = lut_lookup(lut, g);
        b = lut_lookup(lut, b);
    }

    // Step 2: Per-channel curves
    if let Some(ref lut) = pre.red {
        r = lut_lookup(lut, r);
    }
    if let Some(ref lut) = pre.green {
        g = lut_lookup(lut, g);
    }
    if let Some(ref lut) = pre.blue {
        b = lut_lookup(lut, b);
    }

    // Step 3: Luminance curve
    if let Some(ref lut) = pre.luma {
        let l = LUMA_R * r + LUMA_G * g + LUMA_B * b;
        let l_new = lut_lookup(lut, l);
        if l > 1e-6 {
            let scale = l_new / l;
            r = (r * scale).clamp(0.0, 1.0);
            g = (g * scale).clamp(0.0, 1.0);
            b = (b * scale).clamp(0.0, 1.0);
        } else {
            // Near-zero luminance: set uniform gray at mapped value
            r = l_new;
            g = l_new;
            b = l_new;
        }
    }

    (r, g, b)
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
    fn tone_curve_default_is_identity() {
        let tc = ToneCurve::default();
        assert_eq!(tc.points, vec![(0.0, 0.0), (1.0, 1.0)]);
    }

    #[test]
    fn tone_curve_params_default_is_identity() {
        let params = ToneCurveParams::default();
        assert!(params.is_default());
    }

    #[test]
    fn tone_curve_params_non_default_detected() {
        let mut params = ToneCurveParams::default();
        params.rgb.points = vec![(0.0, 0.0), (0.5, 0.6), (1.0, 1.0)];
        assert!(!params.is_default());
    }

    #[test]
    fn tone_curve_lut_identity_is_diagonal() {
        let curve = ToneCurve::default();
        let lut = build_tone_curve_lut(&curve);
        for (i, &v) in lut.iter().enumerate() {
            let expected = i as f32 / 255.0;
            assert!(
                (v - expected).abs() < 1e-5,
                "LUT[{i}] = {v}, expected {expected}"
            );
        }
    }

    #[test]
    fn tone_curve_lut_endpoints_match() {
        let curve = ToneCurve {
            points: vec![(0.0, 0.2), (0.5, 0.6), (1.0, 0.8)],
        };
        let lut = build_tone_curve_lut(&curve);
        assert!(
            (lut[0] - 0.2).abs() < 1e-5,
            "LUT[0] should match first point y"
        );
        assert!(
            (lut[255] - 0.8).abs() < 1e-5,
            "LUT[255] should match last point y"
        );
    }

    #[test]
    fn tone_curve_lut_monotonic() {
        let curve = ToneCurve {
            points: vec![(0.0, 0.0), (0.25, 0.15), (0.75, 0.85), (1.0, 1.0)],
        };
        let lut = build_tone_curve_lut(&curve);
        for i in 1..256 {
            assert!(
                lut[i] >= lut[i - 1],
                "LUT must be monotonic: lut[{}]={} < lut[{}]={}",
                i,
                lut[i],
                i - 1,
                lut[i - 1]
            );
        }
    }

    #[test]
    fn tone_curve_lut_two_points_linear() {
        let curve = ToneCurve {
            points: vec![(0.0, 0.3), (1.0, 0.7)],
        };
        let lut = build_tone_curve_lut(&curve);
        for (i, &v) in lut.iter().enumerate() {
            let t = i as f32 / 255.0;
            let expected = 0.3 + 0.4 * t;
            assert!(
                (v - expected).abs() < 1e-4,
                "LUT[{i}] = {v}, expected {expected}"
            );
        }
    }

    #[test]
    fn tone_curve_apply_identity_no_change() {
        let params = ToneCurveParams::default();
        let pre = ToneCurvePrecomputed::new(&params);
        let (r, g, b) = apply_tone_curves_pre(0.5, 0.3, 0.7, &pre);
        assert!((r - 0.5).abs() < 1e-4);
        assert!((g - 0.3).abs() < 1e-4);
        assert!((b - 0.7).abs() < 1e-4);
    }

    #[test]
    fn tone_curve_rgb_master_shifts_all_channels() {
        let mut params = ToneCurveParams::default();
        params.rgb.points = vec![(0.0, 0.2), (1.0, 0.8)];
        let pre = ToneCurvePrecomputed::new(&params);
        let (r, g, b) = apply_tone_curves_pre(0.0, 0.5, 1.0, &pre);
        assert!((r - 0.2).abs() < 0.02, "black should map to ~0.2, got {r}");
        assert!((b - 0.8).abs() < 0.02, "white should map to ~0.8, got {b}");
        assert!((g - 0.5).abs() < 0.05, "mid should map to ~0.5, got {g}");
    }

    #[test]
    fn tone_curve_per_channel_only_affects_that_channel() {
        let mut params = ToneCurveParams::default();
        params.red.points = vec![(0.0, 0.0), (1.0, 0.5)];
        let pre = ToneCurvePrecomputed::new(&params);
        let (r, g, b) = apply_tone_curves_pre(1.0, 1.0, 1.0, &pre);
        assert!(
            (r - 0.5).abs() < 0.02,
            "red should be compressed to ~0.5, got {r}"
        );
        assert!((g - 1.0).abs() < 0.02, "green should be unchanged, got {g}");
        assert!((b - 1.0).abs() < 0.02, "blue should be unchanged, got {b}");
    }

    #[test]
    fn tone_curve_luma_preserves_color_ratios() {
        let mut params = ToneCurveParams::default();
        params.luma.points = vec![(0.0, 0.0), (1.0, 0.5)];
        let pre = ToneCurvePrecomputed::new(&params);
        let (r, g, _b) = apply_tone_curves_pre(0.8, 0.4, 0.2, &pre);
        let ratio_before = 0.8 / 0.4;
        let ratio_after = r / g;
        assert!(
            (ratio_after - ratio_before).abs() < 0.1,
            "color ratios should be preserved: before={ratio_before}, after={ratio_after}"
        );
    }

    #[test]
    fn tone_curve_luma_near_zero_fallback() {
        let mut params = ToneCurveParams::default();
        params.luma.points = vec![(0.0, 0.3), (1.0, 1.0)];
        let pre = ToneCurvePrecomputed::new(&params);
        let (r, g, b) = apply_tone_curves_pre(0.0, 0.0, 0.0, &pre);
        assert!((r - 0.3).abs() < 0.02, "r should be ~0.3, got {r}");
        assert!((g - 0.3).abs() < 0.02, "g should be ~0.3, got {g}");
        assert!((b - 0.3).abs() < 0.02, "b should be ~0.3, got {b}");
    }

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
