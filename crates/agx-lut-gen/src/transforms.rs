/// Convert RGB to HSL.
///
/// Input: r, g, b in [0, 1].
/// Output: (h, s, l) where h is in [0, 360), s and l in [0, 1].
pub fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) * 0.5;

    if (max - min).abs() < 1e-10 {
        return (0.0, 0.0, l);
    }

    let d = max - min;

    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < 1e-10 {
        let mut h = (g - b) / d;
        if g < b {
            h += 6.0;
        }
        h
    } else if (max - g).abs() < 1e-10 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    let h = h * 60.0;
    (h, s, l)
}

/// Convert HSL to RGB.
///
/// Input: h in [0, 360), s and l in [0, 1].
/// Output: (r, g, b) each in [0, 1].
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < 1e-10 {
        return (l, l, l);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let h_norm = h / 360.0;

    let r = hue_to_rgb(p, q, h_norm + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h_norm);
    let b = hue_to_rgb(p, q, h_norm - 1.0 / 3.0);

    (r, g, b)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> f32 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 0.5 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

/// S-curve contrast adjustment.
///
/// Darkens values below `midpoint` and brightens values above it.
/// `strength` controls the intensity: 0 = identity, higher = more contrast.
/// Uses a gain-based sigmoid that passes through (0,0), (midpoint, midpoint), and (1,1).
pub fn s_curve(x: f32, strength: f32, midpoint: f32) -> f32 {
    if strength.abs() < 1e-10 {
        return x.clamp(0.0, 1.0);
    }

    // Use a power-based S-curve applied separately to each half.
    // For the lower half [0, midpoint], remap to [0,1], apply power, remap back.
    // For the upper half [midpoint, 1], mirror the same.
    // The power is derived from strength: higher strength = more extreme curve.
    let gamma = 1.0 + strength * 3.0; // strength 0.5 -> gamma 2.5

    let result = if x <= midpoint {
        if midpoint.abs() < 1e-10 {
            0.0
        } else {
            let t = x / midpoint;
            // Apply power curve: t^gamma pushes values toward 0 (darkens shadows)
            midpoint * t.powf(gamma)
        }
    } else if (1.0 - midpoint).abs() < 1e-10 {
        1.0
    } else {
        let t = (x - midpoint) / (1.0 - midpoint);
        // Mirror: 1 - (1-t)^gamma brightens highlights
        midpoint + (1.0 - midpoint) * (1.0 - (1.0 - t).powf(gamma))
    };

    result.clamp(0.0, 1.0)
}

/// Film-like soft highlight rolloff.
///
/// `shoulder` controls how aggressively highlights are compressed.
/// 0 = identity, higher = more compression.
/// Shadows pass through mostly unchanged while highlights are softly compressed.
pub fn film_shoulder(x: f32, shoulder: f32) -> f32 {
    if shoulder.abs() < 1e-10 {
        return x.clamp(0.0, 1.0);
    }
    let x = x.clamp(0.0, 1.0);

    // Variable-exponent power curve: x^(1 + shoulder * x^2)
    // At x near 0: exponent ~1 (identity, shadows preserved)
    // At x near 1: exponent ~(1+shoulder) (highlights compressed)
    // Passes through (0,0) and (1,1).
    let exponent = 1.0 + shoulder * x * x;
    let result = x.powf(exponent);
    result.clamp(0.0, 1.0)
}

/// Lift/Gamma/Gain color grading.
///
/// - `lift`: adds to shadows (shifts black point up)
/// - `gamma`: midtone power adjustment (applied as 1/gamma exponent)
/// - `gain`: highlight multiplier
///
/// The formula: `gain * (x + lift * (1 - x))^(1/gamma)`
/// At x=0: output = (gain * lift^(1/gamma)), so lift raises blacks.
/// At x=1: output = gain, so gain scales the white point.
pub fn lift_gamma_gain(x: f32, lift: f32, gamma: f32, gain: f32) -> f32 {
    // Standard LGG formula used in color grading:
    // result = gain * ((1 - lift) * x + lift) ^ (1/gamma)
    // This ensures: at x=0 -> gain * lift^(1/gamma), at x=1 -> gain * 1.0 = gain
    let lifted = (1.0 - lift) * x + lift;
    let lifted = lifted.max(0.0); // prevent negative base for pow
    let gamma_exp = if gamma.abs() < 1e-10 {
        1.0
    } else {
        1.0 / gamma
    };
    let result = gain * lifted.powf(gamma_exp);
    result.clamp(0.0, 1.0)
}

/// Scale saturation by operating in HSL space.
///
/// `scale` = 1.0 is identity, < 1.0 desaturates, > 1.0 saturates.
pub fn scale_saturation(r: f32, g: f32, b: f32, scale: f32) -> (f32, f32, f32) {
    let (h, s, l) = rgb_to_hsl(r, g, b);
    let new_s = (s * scale).clamp(0.0, 1.0);
    let (ro, go, bo) = hsl_to_rgb(h, new_s, l);
    (ro.clamp(0.0, 1.0), go.clamp(0.0, 1.0), bo.clamp(0.0, 1.0))
}

/// Rotate hue within a specific luminance band.
///
/// `rotation` is in degrees, `lum_center` and `lum_width` define the band.
/// The effect fades out smoothly outside the luminance band.
pub fn hue_rotate_in_lum_range(
    r: f32,
    g: f32,
    b: f32,
    rotation: f32,
    lum_center: f32,
    lum_width: f32,
) -> (f32, f32, f32) {
    let (h, s, l) = rgb_to_hsl(r, g, b);

    // Compute blend factor based on how close luminance is to the band center
    let dist = (l - lum_center).abs();
    let half_width = lum_width * 0.5;
    let blend = if dist >= half_width {
        0.0
    } else {
        // Smooth falloff using cosine interpolation
        let t = dist / half_width;
        0.5 * (1.0 + (t * std::f32::consts::PI).cos())
    };

    let new_h = (h + rotation * blend) % 360.0;
    let new_h = if new_h < 0.0 { new_h + 360.0 } else { new_h };

    let (ro, go, bo) = hsl_to_rgb(new_h, s, l);
    (ro.clamp(0.0, 1.0), go.clamp(0.0, 1.0), bo.clamp(0.0, 1.0))
}

/// Mix a fraction of one channel into another.
///
/// `src` and `dst` are channel indices: 0=R, 1=G, 2=B.
/// `amount` is the fraction of the source channel to blend into the destination.
pub fn crossfeed(r: f32, g: f32, b: f32, src: usize, dst: usize, amount: f32) -> (f32, f32, f32) {
    let channels = [r, g, b];
    let src_val = channels[src];

    let mut out = [r, g, b];
    out[dst] = (out[dst] + src_val * amount).clamp(0.0, 1.0);

    (out[0], out[1], out[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() < tol
    }

    // --- RGB <-> HSL round-trip ---

    #[test]
    fn hsl_roundtrip_red() {
        let (h, s, l) = rgb_to_hsl(1.0, 0.0, 0.0);
        assert!(approx_eq(h, 0.0, 1.0), "h={}", h);
        assert!(approx_eq(s, 1.0, 0.01), "s={}", s);
        assert!(approx_eq(l, 0.5, 0.01), "l={}", l);
        let (r, g, b) = hsl_to_rgb(h, s, l);
        assert!(approx_eq(r, 1.0, 0.01));
        assert!(approx_eq(g, 0.0, 0.01));
        assert!(approx_eq(b, 0.0, 0.01));
    }

    #[test]
    fn hsl_roundtrip_green() {
        let (h, s, l) = rgb_to_hsl(0.0, 1.0, 0.0);
        assert!(approx_eq(h, 120.0, 1.0), "h={}", h);
        let (r, g, b) = hsl_to_rgb(h, s, l);
        assert!(approx_eq(r, 0.0, 0.01));
        assert!(approx_eq(g, 1.0, 0.01));
        assert!(approx_eq(b, 0.0, 0.01));
    }

    #[test]
    fn hsl_roundtrip_blue() {
        let (h, s, l) = rgb_to_hsl(0.0, 0.0, 1.0);
        assert!(approx_eq(h, 240.0, 1.0), "h={}", h);
        let (r, g, b) = hsl_to_rgb(h, s, l);
        assert!(approx_eq(r, 0.0, 0.01));
        assert!(approx_eq(g, 0.0, 0.01));
        assert!(approx_eq(b, 1.0, 0.01));
    }

    #[test]
    fn hsl_roundtrip_gray() {
        let (h, s, l) = rgb_to_hsl(0.5, 0.5, 0.5);
        assert!(approx_eq(s, 0.0, 0.01));
        assert!(approx_eq(l, 0.5, 0.01));
        let (r, g, b) = hsl_to_rgb(h, s, l);
        assert!(approx_eq(r, 0.5, 0.01));
        assert!(approx_eq(g, 0.5, 0.01));
        assert!(approx_eq(b, 0.5, 0.01));
    }

    #[test]
    fn hsl_black_and_white() {
        let (_, _, l) = rgb_to_hsl(0.0, 0.0, 0.0);
        assert!(approx_eq(l, 0.0, 0.01));
        let (_, _, l) = rgb_to_hsl(1.0, 1.0, 1.0);
        assert!(approx_eq(l, 1.0, 0.01));
    }

    // --- S-curve ---

    #[test]
    fn s_curve_identity_at_zero_strength() {
        for &x in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            let result = s_curve(x, 0.0, 0.5);
            assert!(approx_eq(result, x, 0.01), "x={}, result={}", x, result);
        }
    }

    #[test]
    fn s_curve_passes_through_endpoints_and_midpoint() {
        assert!(approx_eq(s_curve(0.0, 0.5, 0.5), 0.0, 0.01));
        assert!(approx_eq(s_curve(0.5, 0.5, 0.5), 0.5, 0.01));
        assert!(approx_eq(s_curve(1.0, 0.5, 0.5), 1.0, 0.01));
    }

    #[test]
    fn s_curve_darkens_shadows_brightens_highlights() {
        // The key S-curve property
        let shadow = s_curve(0.25, 0.5, 0.5);
        assert!(shadow < 0.25, "shadow: {} should be < 0.25", shadow);

        let highlight = s_curve(0.75, 0.5, 0.5);
        assert!(
            highlight > 0.75,
            "highlight: {} should be > 0.75",
            highlight
        );
    }

    // --- Film shoulder ---

    #[test]
    fn film_shoulder_identity_at_zero() {
        for &x in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            let result = film_shoulder(x, 0.0);
            assert!(approx_eq(result, x, 0.01), "x={}, result={}", x, result);
        }
    }

    #[test]
    fn film_shoulder_compresses_highlights() {
        // Reinhard-like compression: output at 0.8 should be noticeably below 0.8
        // with a strong shoulder value
        let result = film_shoulder(0.8, 1.0);
        assert!(result < 0.8, "shoulder should compress: {}", result);
        assert!(result > 0.3, "shouldn't compress too much: {}", result);

        // Even with a mild shoulder, midtones should be slightly compressed
        let mild = film_shoulder(0.5, 0.3);
        assert!(mild < 0.5, "mild shoulder should still compress: {}", mild);
    }

    #[test]
    fn film_shoulder_preserves_endpoints() {
        assert!(approx_eq(film_shoulder(0.0, 0.5), 0.0, 0.01));
        assert!(approx_eq(film_shoulder(1.0, 0.5), 1.0, 0.01));
    }

    // --- Lift/Gamma/Gain ---

    #[test]
    fn lgg_identity() {
        for &x in &[0.0, 0.25, 0.5, 0.75, 1.0] {
            let result = lift_gamma_gain(x, 0.0, 1.0, 1.0);
            assert!(approx_eq(result, x, 0.01), "x={}, result={}", x, result);
        }
    }

    #[test]
    fn lgg_lift_raises_blacks() {
        let result = lift_gamma_gain(0.0, 0.05, 1.0, 1.0);
        assert!(result > 0.04, "lift should raise blacks: {}", result);
    }

    #[test]
    fn lgg_gain_scales_whites() {
        let result = lift_gamma_gain(1.0, 0.0, 1.0, 0.9);
        assert!(
            approx_eq(result, 0.9, 0.01),
            "gain should scale: {}",
            result
        );
    }

    // --- Saturation ---

    #[test]
    fn saturation_identity() {
        let (r, g, b) = scale_saturation(0.8, 0.2, 0.4, 1.0);
        assert!(approx_eq(r, 0.8, 0.01));
        assert!(approx_eq(g, 0.2, 0.01));
        assert!(approx_eq(b, 0.4, 0.01));
    }

    #[test]
    fn saturation_zero_gives_gray() {
        let (r, g, b) = scale_saturation(1.0, 0.0, 0.0, 0.0);
        // Pure red at s=0 should be gray at the same lightness
        assert!(approx_eq(r, g, 0.01), "r={} g={}", r, g);
        assert!(approx_eq(g, b, 0.01), "g={} b={}", g, b);
    }

    // --- Hue rotation in lum range ---

    #[test]
    fn hue_rotate_no_effect_outside_band() {
        // White (l=1.0) should be unaffected by a rotation centered at l=0.2
        let (r, g, b) = hue_rotate_in_lum_range(1.0, 1.0, 1.0, 90.0, 0.2, 0.2);
        assert!(approx_eq(r, 1.0, 0.01));
        assert!(approx_eq(g, 1.0, 0.01));
        assert!(approx_eq(b, 1.0, 0.01));
    }

    #[test]
    fn hue_rotate_affects_target_lum() {
        // Pure red (l=0.5), rotate 120 degrees at lum_center=0.5
        let (r, g, b) = hue_rotate_in_lum_range(1.0, 0.0, 0.0, 120.0, 0.5, 1.0);
        // Should shift toward green
        assert!(g > r, "should shift toward green: r={} g={} b={}", r, g, b);
    }

    // --- Crossfeed ---

    #[test]
    fn crossfeed_identity_at_zero() {
        let (r, g, b) = crossfeed(0.5, 0.3, 0.7, 0, 1, 0.0);
        assert!(approx_eq(r, 0.5, 0.01));
        assert!(approx_eq(g, 0.3, 0.01));
        assert!(approx_eq(b, 0.7, 0.01));
    }

    #[test]
    fn crossfeed_adds_fraction() {
        let (r, g, b) = crossfeed(0.5, 0.3, 0.7, 0, 1, 0.1);
        assert!(approx_eq(r, 0.5, 0.01)); // source unchanged
        assert!(approx_eq(g, 0.35, 0.01)); // 0.3 + 0.5*0.1
        assert!(approx_eq(b, 0.7, 0.01)); // other channel unchanged
    }
}
