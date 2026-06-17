//! Color-space conversion matrices and transfer curves.
//!
//! # Working space contract
//!
//! The engine's working space is linear Rec.2020. Conversions between stages
//! are inserted exclusively by the CPU executor in `engine::pipeline`, via
//! [`convert_buffer`] (the single conversion primitive — hub-and-spoke through
//! linear Rec.2020). Engine output is always linear Rec.2020.

use crate::engine::ColorSpace;
use rayon::prelude::*;

/// Linear Rec.2020 → linear sRGB.
///
/// Derived from BT.2020 and sRGB primaries plus the D65 white point.
pub const LINEAR_REC2020_TO_LINEAR_SRGB: [[f32; 3]; 3] = [
    [1.660491, -0.587641, -0.072850],
    [-0.124550, 1.132899, -0.008349],
    [-0.018151, -0.100579, 1.11873],
];

/// Linear sRGB → linear Rec.2020 (inverse of the above).
pub const LINEAR_SRGB_TO_LINEAR_REC2020: [[f32; 3]; 3] = [
    [0.627404, 0.329283, 0.043313],
    [0.069097, 0.919541, 0.011362],
    [0.016391, 0.088013, 0.895595],
];

/// Apply the sRGB transfer curve in a sign-preserving way.
///
/// The standard sRGB curve is defined for non-negative inputs. For negative
/// inputs (which can arise from heavy edits in a wide working space), apply
/// the curve to the absolute value and negate the result. Equivalent to:
/// `sign(x) * srgb_curve(abs(x))`.
pub fn srgb_curve_signed(x: f32) -> f32 {
    let sign_factor = if x < 0.0 { -1.0 } else { 1.0 };
    let absx = x.abs();
    let curved = if absx <= 0.0031308 {
        12.92 * absx
    } else {
        1.055 * absx.powf(1.0 / 2.4) - 0.055
    };
    sign_factor * curved
}

/// Inverse of `srgb_curve_signed`. Sign-preserving inverse sRGB curve.
pub fn srgb_curve_signed_inverse(x: f32) -> f32 {
    let sign_factor = if x < 0.0 { -1.0 } else { 1.0 };
    let absx = x.abs();
    let linear = if absx <= 0.04045 {
        absx / 12.92
    } else {
        ((absx + 0.055) / 1.055).powf(2.4)
    };
    sign_factor * linear
}

/// Apply the Adobe RGB (1998) transfer curve, sign-preserving.
///
/// Adobe RGB encodes with a pure gamma of 563/256 (≈ 2.19921875); the encode
/// direction raises to `1/2.19921875`. Sign-preserving for negative inputs that
/// can arise from heavy edits in the wide working space, matching the
/// `srgb_curve_signed` convention: `sign(x) * |x|^(1/2.19921875)`.
pub fn adobe_rgb_curve_signed(x: f32) -> f32 {
    let sign_factor = if x < 0.0 { -1.0 } else { 1.0 };
    // Encode exponent 1/gamma = 256/563, written as the exact rational so the
    // literal is lint-clean (clippy::excessive_precision) and self-documenting.
    sign_factor * x.abs().powf(256.0 / 563.0)
}

/// Linear Display P3 → linear Rec.2020.
///
/// Display P3 uses the DCI-P3 primaries with D65 white point. The Rec.2020
/// gamut contains the P3 gamut, but P3 primaries expressed in Rec.2020
/// coordinates can produce small negative components (e.g., P3 red's blue
/// channel ≈ −0.0012). Downstream encode clips to [0, 1] at the final step.
pub const LINEAR_P3_TO_LINEAR_REC2020: [[f32; 3]; 3] = [
    [0.753833, 0.198597, 0.047570],
    [0.045744, 0.941776, 0.012480],
    [-0.001210, 0.017601, 0.983610],
];

/// Linear BT.2020 → linear Rec.2020. Identity matrix (BT.2020 primaries
/// == Rec.2020 primaries).
///
/// Defined explicitly so call sites that dispatch over a source-primaries
/// enum (e.g. `decode/heic.rs`) can apply *one* "X → Rec.2020" matrix per
/// pixel without a special-case branch for the no-op. Removing this constant
/// in a future "simplify" pass would silently re-introduce the special case
/// and break the symmetry — the `bt2020_to_rec2020_is_identity` unit test
/// guards against that drift.
pub const LINEAR_BT2020_TO_LINEAR_REC2020: [[f32; 3]; 3] =
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Linear Rec.2020 → linear Display P3 (DCI-P3 primaries, D65). Inverse of
/// `LINEAR_P3_TO_LINEAR_REC2020`. Derived from primaries and pinned against
/// lcms2 by `color_space::icc_crosscheck_tests` (run under `--features icc`).
pub const LINEAR_REC2020_TO_LINEAR_P3: [[f32; 3]; 3] = [
    [1.343578, -0.282180, -0.061399],
    [-0.065297, 1.075788, -0.010490],
    [0.002822, -0.019598, 1.016777],
];

/// Linear Rec.2020 → linear Adobe RGB (1998) (Adobe primaries, D65). Derived
/// from primaries and pinned against lcms2 by `color_space::icc_crosscheck_tests`
/// (run under `--features icc`).
pub const LINEAR_REC2020_TO_LINEAR_ADOBE_RGB: [[f32; 3]; 3] = [
    [1.151978, -0.097503, -0.054475],
    [-0.124550, 1.1329, -0.008349],
    [-0.022530, -0.049807, 1.072337],
];

/// Apply a 3×3 matrix to every pixel of a `[[f32; 3]]` buffer in place.
///
/// Pixel layout: `buf[i] = [r, g, b]`. The matrix is applied as
/// `out = m * v` (row-major).
pub fn apply_matrix_3x3(buf: &mut [[f32; 3]], m: &[[f32; 3]; 3]) {
    for px in buf.iter_mut() {
        let r = px[0];
        let g = px[1];
        let b = px[2];
        px[0] = m[0][0] * r + m[0][1] * g + m[0][2] * b;
        px[1] = m[1][0] * r + m[1][1] * g + m[1][2] * b;
        px[2] = m[2][0] * r + m[2][1] * g + m[2][2] * b;
    }
}

/// Convert a pixel buffer in place from one color space to another, routed
/// through the linear Rec.2020 hub. `from == to` is a true no-op.
///
/// This is the single source of truth for pipeline color-space conversions:
/// each space defines only its to-hub / from-hub hops, so adding a new space
/// later (OKLab, a named log curve) means adding two hops here and nothing else.
pub fn convert_buffer(buf: &mut [[f32; 3]], from: ColorSpace, to: ColorSpace) {
    if from == to {
        return;
    }
    to_hub(buf, from);
    from_hub(buf, to);
}

/// Apply a scalar transfer-curve to every channel of every pixel. Takes a `fn` pointer
/// rather than a closure so callers can pass named functions (e.g. `srgb_curve_signed`)
/// directly, avoiding closure indirection and keeping call sites readable.
fn apply_curve(buf: &mut [[f32; 3]], f: fn(f32) -> f32) {
    buf.par_iter_mut().for_each(|p| {
        p[0] = f(p[0]);
        p[1] = f(p[1]);
        p[2] = f(p[2]);
    });
}

/// Bring a buffer from `space` into the linear Rec.2020 hub.
fn to_hub(buf: &mut [[f32; 3]], space: ColorSpace) {
    match space {
        ColorSpace::LinearRec2020 => {}
        ColorSpace::GammaRec2020 => apply_curve(buf, srgb_curve_signed_inverse),
        ColorSpace::LinearSrgb => apply_matrix_3x3(buf, &LINEAR_SRGB_TO_LINEAR_REC2020),
        ColorSpace::SrgbGamma => {
            apply_curve(buf, srgb_curve_signed_inverse);
            apply_matrix_3x3(buf, &LINEAR_SRGB_TO_LINEAR_REC2020);
        }
    }
}

/// Take a buffer from the linear Rec.2020 hub into `space`.
fn from_hub(buf: &mut [[f32; 3]], space: ColorSpace) {
    match space {
        ColorSpace::LinearRec2020 => {}
        ColorSpace::GammaRec2020 => apply_curve(buf, srgb_curve_signed),
        ColorSpace::LinearSrgb => apply_matrix_3x3(buf, &LINEAR_REC2020_TO_LINEAR_SRGB),
        ColorSpace::SrgbGamma => {
            apply_matrix_3x3(buf, &LINEAR_REC2020_TO_LINEAR_SRGB);
            apply_curve(buf, srgb_curve_signed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ColorSpace;

    #[test]
    fn convert_buffer_identity_is_noop() {
        let mut buf = vec![[0.2_f32, 0.5, 0.8], [-0.1, 1.2, 0.0]];
        let before = buf.clone();
        convert_buffer(&mut buf, ColorSpace::GammaRec2020, ColorSpace::GammaRec2020);
        assert_eq!(
            buf, before,
            "same-space conversion must not touch the buffer"
        );
    }

    #[test]
    fn convert_buffer_round_trips() {
        let original = vec![[0.2_f32, 0.5, 0.8], [0.05, 0.9, 0.3]];
        for space in [
            ColorSpace::GammaRec2020,
            ColorSpace::LinearSrgb,
            ColorSpace::SrgbGamma,
        ] {
            let mut buf = original.clone();
            convert_buffer(&mut buf, ColorSpace::LinearRec2020, space);
            convert_buffer(&mut buf, space, ColorSpace::LinearRec2020);
            for (i, px) in buf.iter().enumerate() {
                for c in 0..3 {
                    assert!(
                        (px[c] - original[i][c]).abs() < 1e-5,
                        "round trip via {space:?} drifted at [{i}][{c}]"
                    );
                }
            }
        }
    }

    #[test]
    fn convert_buffer_gamma_to_srgbgamma_matches_legacy_bracket_math() {
        // The Gamma Rec.2020 -> sRGB-gamma hop must equal the pre-sample bracket math:
        // inverse sRGB curve (decode gamma), Rec.2020 -> sRGB matrix, sRGB curve (re-encode).
        let px = [0.4_f32, 0.6, 0.2];
        let mut buf = vec![px];
        convert_buffer(&mut buf, ColorSpace::GammaRec2020, ColorSpace::SrgbGamma);

        let lin_rec = [
            srgb_curve_signed_inverse(px[0]),
            srgb_curve_signed_inverse(px[1]),
            srgb_curve_signed_inverse(px[2]),
        ];
        let m = &LINEAR_REC2020_TO_LINEAR_SRGB;
        let lin_srgb = [
            m[0][0] * lin_rec[0] + m[0][1] * lin_rec[1] + m[0][2] * lin_rec[2],
            m[1][0] * lin_rec[0] + m[1][1] * lin_rec[1] + m[1][2] * lin_rec[2],
            m[2][0] * lin_rec[0] + m[2][1] * lin_rec[1] + m[2][2] * lin_rec[2],
        ];
        let expected = [
            srgb_curve_signed(lin_srgb[0]),
            srgb_curve_signed(lin_srgb[1]),
            srgb_curve_signed(lin_srgb[2]),
        ];
        for c in 0..3 {
            assert!(
                (buf[0][c] - expected[c]).abs() < 1e-6,
                "channel {c} mismatch"
            );
        }
    }

    /// Multiplying a matrix by its inverse should produce identity within
    /// float epsilon.
    #[test]
    fn rec2020_srgb_round_trip_is_identity() {
        let m = LINEAR_REC2020_TO_LINEAR_SRGB;
        let m_inv = LINEAR_SRGB_TO_LINEAR_REC2020;

        for v in &[
            [1.0_f32, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.5, 0.5, 0.5],
            [0.18, 0.18, 0.18],
        ] {
            let mid = [
                m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
                m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
                m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
            ];
            let out = [
                m_inv[0][0] * mid[0] + m_inv[0][1] * mid[1] + m_inv[0][2] * mid[2],
                m_inv[1][0] * mid[0] + m_inv[1][1] * mid[1] + m_inv[1][2] * mid[2],
                m_inv[2][0] * mid[0] + m_inv[2][1] * mid[1] + m_inv[2][2] * mid[2],
            ];

            for c in 0..3 {
                assert!(
                    (out[c] - v[c]).abs() < 1e-4,
                    "round-trip mismatch at channel {}: in={} out={}",
                    c,
                    v[c],
                    out[c]
                );
            }
        }
    }

    #[test]
    fn apply_matrix_3x3_round_trips_through_inverse() {
        let mut buf = vec![[1.0_f32, 0.5, 0.2], [0.0, 1.2, -0.1]];
        apply_matrix_3x3(&mut buf, &LINEAR_REC2020_TO_LINEAR_SRGB);
        apply_matrix_3x3(&mut buf, &LINEAR_SRGB_TO_LINEAR_REC2020);
        assert!((buf[0][0] - 1.0).abs() < 1e-4);
        assert!((buf[0][1] - 0.5).abs() < 1e-4);
        assert!((buf[0][2] - 0.2).abs() < 1e-4);
        assert!((buf[1][0] - 0.0).abs() < 1e-4);
        assert!((buf[1][1] - 1.2).abs() < 1e-4);
        assert!((buf[1][2] - (-0.1)).abs() < 1e-4);
    }

    #[test]
    fn srgb_curve_signed_handles_negatives_by_sign_extension() {
        let positive = srgb_curve_signed(0.5);
        let negative = srgb_curve_signed(-0.5);
        assert!(positive > 0.0);
        assert!(negative < 0.0);
        assert!((positive + negative).abs() < 1e-6, "curve must be odd");
    }

    #[test]
    fn srgb_curve_signed_round_trip() {
        for v in &[-1.5_f32, -0.5, 0.0, 0.18, 0.5, 1.0, 1.5] {
            let gamma = srgb_curve_signed(*v);
            let back = srgb_curve_signed_inverse(gamma);
            assert!(
                (back - v).abs() < 1e-5,
                "round-trip drift at v={}: got {}",
                v,
                back
            );
        }
    }

    #[test]
    fn p3_red_maps_into_rec2020() {
        let m = LINEAR_P3_TO_LINEAR_REC2020;
        let p3_red = [1.0_f32, 0.0, 0.0];
        let rec2020 = [
            m[0][0] * p3_red[0] + m[0][1] * p3_red[1] + m[0][2] * p3_red[2],
            m[1][0] * p3_red[0] + m[1][1] * p3_red[1] + m[1][2] * p3_red[2],
            m[2][0] * p3_red[0] + m[2][1] * p3_red[1] + m[2][2] * p3_red[2],
        ];
        assert!(rec2020[0] > 0.0 && rec2020[0] < 1.0);
        assert!(rec2020[0].is_finite() && rec2020[1].is_finite() && rec2020[2].is_finite());
    }

    #[test]
    fn srgb_curve_signed_at_threshold_round_trips() {
        let at_threshold = srgb_curve_signed(0.0031308);
        let back = srgb_curve_signed_inverse(at_threshold);
        assert!((back - 0.0031308).abs() < 1e-6);
    }

    #[test]
    fn bt2020_to_rec2020_is_identity() {
        let m = LINEAR_BT2020_TO_LINEAR_REC2020;
        for v in &[
            [1.0_f32, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.5, 0.5, 0.5],
            [-0.1, 1.2, 0.3],
        ] {
            let out = [
                m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
                m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
                m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
            ];
            assert!((out[0] - v[0]).abs() < 1e-9);
            assert!((out[1] - v[1]).abs() < 1e-9);
            assert!((out[2] - v[2]).abs() < 1e-9);
        }
    }

    #[test]
    fn display_p3_red_survives_as_wider_gamut_in_rec2020() {
        // P3 pure red, mapped to linear Rec.2020, should stay in-gamut and
        // produce a wider primary than sRGB pure red (whose Rec.2020 mapping
        // squashes to ~(0.627, 0.069, 0.016)). Specifically, P3 red mapped to
        // Rec.2020 has a notably larger R component than sRGB red mapped to
        // Rec.2020, because P3 is between sRGB and Rec.2020 in gamut size.
        let p3_red = [1.0_f32, 0.0, 0.0];
        let m = LINEAR_P3_TO_LINEAR_REC2020;
        let rec2020 = [
            m[0][0] * p3_red[0] + m[0][1] * p3_red[1] + m[0][2] * p3_red[2],
            m[1][0] * p3_red[0] + m[1][1] * p3_red[1] + m[1][2] * p3_red[2],
            m[2][0] * p3_red[0] + m[2][1] * p3_red[1] + m[2][2] * p3_red[2],
        ];

        // Ground truth from the matrix: column 0 is (~0.7538, ~0.0457, ~-0.00121).
        assert!(
            rec2020[0] > 0.7,
            "P3 red R component too small in Rec.2020: {}",
            rec2020[0]
        );
        assert!(
            rec2020[1] > 0.0,
            "P3 red G component should be positive in Rec.2020"
        );
        assert!(
            rec2020[2].abs() < 0.05,
            "P3 red B should be near zero (small negative OK)"
        );

        // Sanity: this Rec.2020 representation is *brighter on R* than the equivalent
        // sRGB-red mapped to Rec.2020, which is the whole point of preserving P3.
        let m_srgb = LINEAR_SRGB_TO_LINEAR_REC2020;
        let srgb_red_in_rec2020 = m_srgb[0][0]; // 0.627404
        assert!(
            rec2020[0] > srgb_red_in_rec2020,
            "P3 red ({}) should map to a *wider* R in Rec.2020 than sRGB red ({}); \
             that's the entire reason we route P3 directly rather than squashing.",
            rec2020[0],
            srgb_red_in_rec2020,
        );
    }

    #[test]
    fn rec2020_to_p3_and_adobe_preserve_white() {
        // Each row must sum to ~1.0 so neutral (equal-RGB) values stay neutral.
        for m in [
            &LINEAR_REC2020_TO_LINEAR_P3,
            &LINEAR_REC2020_TO_LINEAR_ADOBE_RGB,
        ] {
            for row in m.iter() {
                let sum = row[0] + row[1] + row[2];
                assert!((sum - 1.0).abs() < 1e-3, "row sum {sum} should be ~1.0");
            }
        }
    }

    #[test]
    fn rec2020_p3_round_trip_is_identity() {
        // LINEAR_REC2020_TO_LINEAR_P3 must invert the existing P3 -> Rec.2020 matrix.
        let fwd = LINEAR_P3_TO_LINEAR_REC2020;
        let inv = LINEAR_REC2020_TO_LINEAR_P3;
        for v in &[
            [1.0_f32, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.4, 0.6, 0.2],
        ] {
            let mid = [
                fwd[0][0] * v[0] + fwd[0][1] * v[1] + fwd[0][2] * v[2],
                fwd[1][0] * v[0] + fwd[1][1] * v[1] + fwd[1][2] * v[2],
                fwd[2][0] * v[0] + fwd[2][1] * v[1] + fwd[2][2] * v[2],
            ];
            let out = [
                inv[0][0] * mid[0] + inv[0][1] * mid[1] + inv[0][2] * mid[2],
                inv[1][0] * mid[0] + inv[1][1] * mid[1] + inv[1][2] * mid[2],
                inv[2][0] * mid[0] + inv[2][1] * mid[1] + inv[2][2] * mid[2],
            ];
            for c in 0..3 {
                assert!((out[c] - v[c]).abs() < 1e-4, "round-trip drift at {c}");
            }
        }
    }

    #[test]
    fn adobe_rgb_curve_signed_is_odd_and_round_trips() {
        let pos = adobe_rgb_curve_signed(0.5);
        let neg = adobe_rgb_curve_signed(-0.5);
        assert!((pos + neg).abs() < 1e-6, "curve must be odd");
        let decoded = pos.powf(563.0 / 256.0); // inverse gamma (563/256)
        assert!((decoded - 0.5).abs() < 1e-4, "round-trip drift: {decoded}");
    }
}

/// Cross-check the hand-baked Rec.2020 → target matrices against lcms2. Gated on
/// `icc` because lcms2 is only available behind that feature. Run with:
/// `cargo test -p agx-photo --features icc --lib color_space::icc_crosscheck_tests`.
#[cfg(all(test, feature = "icc"))]
mod icc_crosscheck_tests {
    use super::*;
    use lcms2::{CIExyY, CIExyYTRIPLE, Intent, PixelFormat, Profile, ToneCurve, Transform};

    const D65: CIExyY = CIExyY {
        x: 0.3127,
        y: 0.3290,
        Y: 1.0,
    };

    fn linear_profile(r: (f64, f64), g: (f64, f64), b: (f64, f64)) -> Profile {
        let primaries = CIExyYTRIPLE {
            Red: CIExyY {
                x: r.0,
                y: r.1,
                Y: 1.0,
            },
            Green: CIExyY {
                x: g.0,
                y: g.1,
                Y: 1.0,
            },
            Blue: CIExyY {
                x: b.0,
                y: b.1,
                Y: 1.0,
            },
        };
        let linear = ToneCurve::new(1.0);
        Profile::new_rgb(&D65, &primaries, &[&linear, &linear, &linear])
            .expect("build linear profile")
    }

    fn assert_matrix_matches_lcms2(target: Profile, m: &[[f32; 3]; 3]) {
        // Linear Rec.2020 source so the transform is the pure primary matrix.
        let src = linear_profile((0.708, 0.292), (0.170, 0.797), (0.131, 0.046));
        let t = Transform::new(
            &src,
            PixelFormat::RGB_FLT,
            &target,
            PixelFormat::RGB_FLT,
            Intent::RelativeColorimetric,
        )
        .expect("build transform");

        for color in [
            [0.5_f32, 0.2, 0.1],
            [0.1, 0.6, 0.3],
            [0.9, 0.8, 0.2],
            [0.3, 0.3, 0.3],
        ] {
            let mut buf = [color];
            t.transform_in_place(&mut buf[..]);
            let lcms = buf[0];
            let ours = [
                m[0][0] * color[0] + m[0][1] * color[1] + m[0][2] * color[2],
                m[1][0] * color[0] + m[1][1] * color[1] + m[1][2] * color[2],
                m[2][0] * color[0] + m[2][1] * color[1] + m[2][2] * color[2],
            ];
            for c in 0..3 {
                // Our f32 matrix multiply vs lcms2's f64 internals: 2e-3 headroom
                // covers the rounding difference for a pure 3x3 primary conversion.
                assert!(
                    (lcms[c] - ours[c]).abs() < 2e-3,
                    "channel {c}: lcms2 {} vs ours {}",
                    lcms[c],
                    ours[c]
                );
            }
        }
    }

    #[test]
    fn rec2020_to_p3_matrix_matches_lcms2() {
        let p3 = linear_profile((0.680, 0.320), (0.265, 0.690), (0.150, 0.060));
        assert_matrix_matches_lcms2(p3, &LINEAR_REC2020_TO_LINEAR_P3);
    }

    #[test]
    fn rec2020_to_adobe_matrix_matches_lcms2() {
        let adobe = linear_profile((0.640, 0.330), (0.210, 0.710), (0.150, 0.060));
        assert_matrix_matches_lcms2(adobe, &LINEAR_REC2020_TO_LINEAR_ADOBE_RGB);
    }
}
