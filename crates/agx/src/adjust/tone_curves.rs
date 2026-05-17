#![doc = include_str!("tone_curves.md")]

use super::{LUMA_B, LUMA_G, LUMA_R};
use serde::{Deserialize, Serialize};

// --- Tone Curves ---

/// A single tone curve defined by control points.
/// Points are (input, output) pairs in [0.0, 1.0], sorted by input.
/// First point must have x=0.0, last must have x=1.0.
#[cfg_attr(
    any(feature = "docgen", feature = "validate"),
    derive(schemars::JsonSchema)
)]
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
#[cfg_attr(
    any(feature = "docgen", feature = "validate"),
    derive(schemars::JsonSchema)
)]
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

    // Special case: 2 points = linear interpolation.
    // Out-of-[0, 1] curve domain: piecewise interpolation linearly
    // extrapolates from the nearest segment endpoint. Wide-gamut headroom
    // is preserved; the final clamp is at encode.
    if n == 2 {
        let mut lut = [0.0_f32; 256];
        let (x0, y0) = pts[0];
        let (x1, y1) = pts[1];
        let dx = x1 - x0;
        for (i, slot) in lut.iter_mut().enumerate() {
            let t = i as f32 / 255.0;
            let frac = if dx.abs() < 1e-9 { 0.0 } else { (t - x0) / dx };
            *slot = y0 + frac * (y1 - y0);
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

        *slot = h00 * y0 + h10 * h * m[seg] + h01 * y1 + h11 * h * m[seg + 1];
    }

    lut
}

/// Look up a value in a precomputed 256-entry LUT with linear interpolation.
#[inline(always)]
pub(crate) fn lut_lookup(lut: &[f32; 256], value: f32) -> f32 {
    let idx = value * 255.0;
    // Domain-safety: clamp index to valid array bounds. OOG inputs are handled
    // by returning the endpoint LUT value (nearest-boundary lookup, not
    // extrapolation). The LUT values themselves may be OOG if the curve is
    // non-identity; those propagate unclamped to the output.
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
            r *= scale;
            g *= scale;
            b *= scale;
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
    fn tone_curve_identity_passes_through_oog_input() {
        // Identity curve (all LUTs are None): OOG input passes through unchanged
        // via the identity fast path — no LUT lookup occurs.
        // Input 1.2 should return 1.2, not clamped to 1.0.
        let params = ToneCurveParams::default();
        let pre = ToneCurvePrecomputed::new(&params);
        let (r, _g, _b) = apply_tone_curves_pre(1.2, 0.5, 0.5, &pre);
        assert!(r > 1.0, "tone curve clamped OOG: {}", r);
        assert!(r.is_finite());
    }

    #[test]
    fn lut_lookup_clamps_index_returns_endpoint_value() {
        // Build a non-identity LUT and feed it OOG values.
        // Verify: the function does not panic; OOG inputs get the endpoint LUT
        // value (nearest-boundary clamping of the index, not extrapolation).
        // The LUT value at the endpoint itself propagates unclamped as output.
        let curve = ToneCurve {
            points: vec![(0.0, 0.2), (0.5, 0.6), (1.0, 0.8)],
        };
        let lut = build_tone_curve_lut(&curve);

        // Above-range input should return the same value as input=1.0
        let out_oog_high = lut_lookup(&lut, 1.2);
        let out_endpoint_high = lut_lookup(&lut, 1.0);
        assert_eq!(
            out_oog_high, out_endpoint_high,
            "OOG high value should map to endpoint LUT value"
        );

        // Below-range input should return the same value as input=0.0
        let out_oog_low = lut_lookup(&lut, -0.3);
        let out_endpoint_low = lut_lookup(&lut, 0.0);
        assert_eq!(
            out_oog_low, out_endpoint_low,
            "negative input should map to zero endpoint LUT value"
        );
    }

    #[test]
    fn tone_curve_oog_input_finite_with_active_luma_curve() {
        // Non-identity luma curve: OOG input must produce finite output.
        // lut_lookup clamps the index (domain-safety) so the luma lookup returns
        // a bounded value, then the scale factor is applied unclamped.
        let mut params = ToneCurveParams::default();
        params.luma.points = vec![(0.0, 0.0), (0.5, 0.4), (1.0, 0.9)];
        let pre = ToneCurvePrecomputed::new(&params);
        let (r, g, b) = apply_tone_curves_pre(1.2, 0.8, 0.7, &pre);
        assert!(r.is_finite(), "r not finite: {}", r);
        assert!(g.is_finite(), "g not finite: {}", g);
        assert!(b.is_finite(), "b not finite: {}", b);
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
}
