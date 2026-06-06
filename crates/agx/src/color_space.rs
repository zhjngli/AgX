//! Color-space conversion matrices and transfer curves.
//!
//! Working space contract: linear Rec.2020 between decode and the engine,
//! gamma-encoded Rec.2020 between stages 5 and 8. See
//! `docs/plans/2026-05-16-wide-working-space-design.md`.

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

/// Sample a 3D LUT (sRGB-gamma authored) from a gamma-encoded Rec.2020 pixel.
///
/// Existing `.cube` LUTs are authored in sRGB-gamma space. The engine working
/// space for stages 5–8 is gamma-encoded Rec.2020, so the LUT call is bracketed:
/// gamma Rec.2020 → linear Rec.2020 → linear sRGB → gamma sRGB → LUT →
/// gamma sRGB → linear sRGB → linear Rec.2020 → gamma Rec.2020.
pub fn wrap_lut_lookup<F>(r: f32, g: f32, b: f32, sample: F) -> (f32, f32, f32)
where
    F: FnOnce(f32, f32, f32) -> (f32, f32, f32),
{
    let r_lin_r = srgb_curve_signed_inverse(r);
    let g_lin_r = srgb_curve_signed_inverse(g);
    let b_lin_r = srgb_curve_signed_inverse(b);

    let m = &LINEAR_REC2020_TO_LINEAR_SRGB;
    let r_lin_s = m[0][0] * r_lin_r + m[0][1] * g_lin_r + m[0][2] * b_lin_r;
    let g_lin_s = m[1][0] * r_lin_r + m[1][1] * g_lin_r + m[1][2] * b_lin_r;
    let b_lin_s = m[2][0] * r_lin_r + m[2][1] * g_lin_r + m[2][2] * b_lin_r;

    let r_g_s = srgb_curve_signed(r_lin_s);
    let g_g_s = srgb_curve_signed(g_lin_s);
    let b_g_s = srgb_curve_signed(b_lin_s);

    let (r_out, g_out, b_out) = sample(r_g_s, g_g_s, b_g_s);

    let r_lin_s2 = srgb_curve_signed_inverse(r_out);
    let g_lin_s2 = srgb_curve_signed_inverse(g_out);
    let b_lin_s2 = srgb_curve_signed_inverse(b_out);

    let m_inv = &LINEAR_SRGB_TO_LINEAR_REC2020;
    let r_lin_r2 = m_inv[0][0] * r_lin_s2 + m_inv[0][1] * g_lin_s2 + m_inv[0][2] * b_lin_s2;
    let g_lin_r2 = m_inv[1][0] * r_lin_s2 + m_inv[1][1] * g_lin_s2 + m_inv[1][2] * b_lin_s2;
    let b_lin_r2 = m_inv[2][0] * r_lin_s2 + m_inv[2][1] * g_lin_s2 + m_inv[2][2] * b_lin_s2;

    (
        srgb_curve_signed(r_lin_r2),
        srgb_curve_signed(g_lin_r2),
        srgb_curve_signed(b_lin_r2),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn wrap_lut_lookup_identity_lut_round_trips() {
        let identity = |r: f32, g: f32, b: f32| (r, g, b);
        for (r, g, b) in &[
            (0.2_f32, 0.5, 0.8),
            (0.0, 0.0, 0.0),
            (1.0, 1.0, 1.0),
            (0.7, 0.2, 0.4),
        ] {
            let (or, og, ob) = wrap_lut_lookup(*r, *g, *b, identity);
            assert!((or - r).abs() < 1e-3, "r drift: in={r} out={or}");
            assert!((og - g).abs() < 1e-3, "g drift: in={g} out={og}");
            assert!((ob - b).abs() < 1e-3, "b drift: in={b} out={ob}");
        }
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

    #[test]
    fn wrap_lut_lookup_constant_non_white_lut_matches_hand_computed_round_trip() {
        // A LUT that always returns a gamut-asymmetric gamma-sRGB triple. The
        // post-LUT bracket converts that to gamma Rec.2020: a swapped matrix
        // would break this assertion (unlike a constant-white case, which is
        // invariant under both matrices because their rows sum to ~1.0).
        let constant = |_: f32, _: f32, _: f32| (0.7_f32, 0.3, 0.5);

        let (or, og, ob) = wrap_lut_lookup(0.1, 0.2, 0.3, constant);

        let r_lin_s = srgb_curve_signed_inverse(0.7);
        let g_lin_s = srgb_curve_signed_inverse(0.3);
        let b_lin_s = srgb_curve_signed_inverse(0.5);

        let m = &LINEAR_SRGB_TO_LINEAR_REC2020;
        let r_lin_r = m[0][0] * r_lin_s + m[0][1] * g_lin_s + m[0][2] * b_lin_s;
        let g_lin_r = m[1][0] * r_lin_s + m[1][1] * g_lin_s + m[1][2] * b_lin_s;
        let b_lin_r = m[2][0] * r_lin_s + m[2][1] * g_lin_s + m[2][2] * b_lin_s;

        let expected = [
            srgb_curve_signed(r_lin_r),
            srgb_curve_signed(g_lin_r),
            srgb_curve_signed(b_lin_r),
        ];

        assert!(
            (or - expected[0]).abs() < 1e-6,
            "r: got {or}, expected {}",
            expected[0]
        );
        assert!(
            (og - expected[1]).abs() < 1e-6,
            "g: got {og}, expected {}",
            expected[1]
        );
        assert!(
            (ob - expected[2]).abs() < 1e-6,
            "b: got {ob}, expected {}",
            expected[2]
        );
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
