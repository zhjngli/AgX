//! Color-space conversion matrices and transfer curves.
//!
//! Working space contract: linear Rec.2020 between decode and the engine,
//! gamma-encoded Rec.2020 between stages 5 and 8. See
//! `docs/plans/2026-05-16-wide-working-space-design.md`.

/// Linear Rec.2020 → linear sRGB.
///
/// Derived from BT.2020 and sRGB primaries plus the D65 white point.
pub const LINEAR_REC2020_TO_LINEAR_SRGB: [[f32; 3]; 3] = [
    [ 1.660491, -0.587641, -0.072850],
    [-0.124550,  1.132899, -0.008349],
    [-0.018151, -0.100579,  1.118730],
];

/// Linear sRGB → linear Rec.2020 (inverse of the above).
pub const LINEAR_SRGB_TO_LINEAR_REC2020: [[f32; 3]; 3] = [
    [0.627404, 0.329283, 0.043313],
    [0.069097, 0.919541, 0.011362],
    [0.016391, 0.088013, 0.895595],
];

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
                    c, v[c], out[c]
                );
            }
        }
    }
}
