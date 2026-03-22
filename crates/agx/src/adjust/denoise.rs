use serde::{Deserialize, Serialize};

use super::{LUMA_B, LUMA_G, LUMA_R};

/// Noise reduction parameters.
///
/// - `luminance`: 0–100, strength of luminance denoising
/// - `color`: 0–100, strength of chroma denoising
/// - `detail`: 0–100, finest-scale protection (higher = more detail kept)
///
/// When all three are zero, the noise reduction pass is skipped entirely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoiseReductionParams {
    #[serde(default)]
    pub luminance: f32,
    #[serde(default)]
    pub color: f32,
    #[serde(default)]
    pub detail: f32,
}

impl Default for NoiseReductionParams {
    fn default() -> Self {
        Self {
            luminance: 0.0,
            color: 0.0,
            detail: 0.0,
        }
    }
}

impl NoiseReductionParams {
    /// Returns true when no noise reduction effect would be applied.
    pub fn is_neutral(&self) -> bool {
        self.luminance == 0.0 && self.color == 0.0 && self.detail == 0.0
    }
}

/// Split RGB buffer into Y, Cb, Cr channels.
///
/// Y  = LUMA_R * R + LUMA_G * G + LUMA_B * B
/// Cb = B - Y
/// Cr = R - Y
fn rgb_to_ycbcr(pixels: &[[f32; 3]]) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let n = pixels.len();
    let mut y = Vec::with_capacity(n);
    let mut cb = Vec::with_capacity(n);
    let mut cr = Vec::with_capacity(n);
    for px in pixels {
        let luma = LUMA_R * px[0] + LUMA_G * px[1] + LUMA_B * px[2];
        y.push(luma);
        cb.push(px[2] - luma);
        cr.push(px[0] - luma);
    }
    (y, cb, cr)
}

/// Reconstruct RGB from Y, Cb, Cr channels. Clamps to [0, 1].
///
/// R = Y + Cr
/// B = Y + Cb
/// G = (Y - LUMA_R * R - LUMA_B * B) / LUMA_G
fn ycbcr_to_rgb(y: &[f32], cb: &[f32], cr: &[f32]) -> Vec<[f32; 3]> {
    let n = y.len();
    let mut pixels = Vec::with_capacity(n);
    for i in 0..n {
        let r_raw = y[i] + cr[i];
        let b_raw = y[i] + cb[i];
        let g_raw = (y[i] - LUMA_R * r_raw - LUMA_B * b_raw) / LUMA_G;
        pixels.push([
            r_raw.clamp(0.0, 1.0),
            g_raw.clamp(0.0, 1.0),
            b_raw.clamp(0.0, 1.0),
        ]);
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params_are_neutral() {
        let p = NoiseReductionParams::default();
        assert!(p.is_neutral());
        assert_eq!(p.luminance, 0.0);
        assert_eq!(p.color, 0.0);
        assert_eq!(p.detail, 0.0);
    }

    #[test]
    fn non_zero_is_not_neutral() {
        let p = NoiseReductionParams {
            luminance: 10.0,
            color: 0.0,
            detail: 0.0,
        };
        assert!(!p.is_neutral());
        let p2 = NoiseReductionParams {
            luminance: 0.0,
            color: 5.0,
            detail: 0.0,
        };
        assert!(!p2.is_neutral());
        let p3 = NoiseReductionParams {
            luminance: 0.0,
            color: 0.0,
            detail: 50.0,
        };
        assert!(!p3.is_neutral());
    }

    #[test]
    fn ycbcr_roundtrip() {
        let pixels: Vec<[f32; 3]> = vec![
            [0.5, 0.3, 0.1],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.8, 0.2, 0.6],
        ];
        let (y, cb, cr) = rgb_to_ycbcr(&pixels);
        let result = ycbcr_to_rgb(&y, &cb, &cr);
        for (i, px) in pixels.iter().enumerate() {
            for c in 0..3 {
                assert!(
                    (result[i][c] - px[c]).abs() < 1e-6,
                    "pixel {i} channel {c}: expected {}, got {}",
                    px[c],
                    result[i][c]
                );
            }
        }
    }
}
