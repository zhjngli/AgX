use rayon::prelude::*;
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

/// Estimate noise standard deviation using median absolute deviation (MAD).
///
/// sigma = median(|data|) / 0.6745
///
/// This is a robust estimator assuming the finest wavelet level is dominated
/// by Gaussian noise (Donoho & Johnstone, 1994).
fn estimate_sigma(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    let mut abs_vals: Vec<f32> = data.iter().map(|v| v.abs()).collect();
    let mid = abs_vals.len() / 2;
    abs_vals.select_nth_unstable_by(mid, |a, b| a.partial_cmp(b).unwrap());
    let median = abs_vals[mid];
    median / 0.6745
}

/// Apply soft thresholding in-place: sign(x) * max(|x| - threshold, 0).
fn soft_threshold(data: &mut [f32], threshold: f32) {
    for v in data.iter_mut() {
        let abs = v.abs();
        if abs <= threshold {
            *v = 0.0;
        } else {
            *v = v.signum() * (abs - threshold);
        }
    }
}

/// Number of wavelet decomposition levels.
const NUM_LEVELS: usize = 5;

/// B3-spline kernel: [1/16, 1/4, 3/8, 1/4, 1/16]
const B3_KERNEL: [f32; 5] = [1.0 / 16.0, 4.0 / 16.0, 6.0 / 16.0, 4.0 / 16.0, 1.0 / 16.0];

/// Mirror-reflect index at image boundaries.
#[inline]
fn mirror(i: isize, max: usize) -> usize {
    if max <= 1 {
        return 0;
    }
    let period = 2 * max - 2;
    let pos = if i < 0 { (-i) as usize } else { i as usize };
    let m = pos % period;
    if m >= max {
        period - m
    } else {
        m
    }
}

/// Horizontal convolution with B3-spline kernel at given tap spacing (gap = 2^level).
fn convolve_horizontal(input: &[f32], width: usize, height: usize, gap: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; width * height];
    output
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                let mut sum = 0.0;
                for (k, &w_k) in B3_KERNEL.iter().enumerate() {
                    let offset = (k as isize - 2) * gap as isize;
                    let xi = mirror(x as isize + offset, width);
                    sum += w_k * input[y * width + xi];
                }
                *out = sum;
            }
        });
    output
}

/// Vertical convolution with B3-spline kernel at given tap spacing (gap = 2^level).
fn convolve_vertical(input: &[f32], width: usize, height: usize, gap: usize) -> Vec<f32> {
    let mut output = vec![0.0f32; width * height];
    output
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                let mut sum = 0.0;
                for (k, &w_k) in B3_KERNEL.iter().enumerate() {
                    let offset = (k as isize - 2) * gap as isize;
                    let yi = mirror(y as isize + offset, height);
                    sum += w_k * input[yi * width + x];
                }
                *out = sum;
            }
        });
    output
}

/// À trous wavelet decomposition into NUM_LEVELS detail levels + residual.
///
/// At each level k (0–4), the kernel tap spacing is 2^k pixels.
/// detail[k] = approx_prev - approx_current.
/// The residual is the final approximation after all levels.
fn atrous_decompose(input: &[f32], width: usize, height: usize) -> (Vec<Vec<f32>>, Vec<f32>) {
    let n = width * height;
    let mut details: Vec<Vec<f32>> = Vec::with_capacity(NUM_LEVELS);
    let mut approximation = input.to_vec();

    for level in 0..NUM_LEVELS {
        let gap = 1 << level; // 1, 2, 4, 8, 16
        let smoothed = convolve_vertical(
            &convolve_horizontal(&approximation, width, height, gap),
            width,
            height,
            gap,
        );
        let mut detail = vec![0.0f32; n];
        for i in 0..n {
            detail[i] = approximation[i] - smoothed[i];
        }
        details.push(detail);
        approximation = smoothed;
    }

    (details, approximation)
}

/// Per-level threshold scale factors. Coarser levels need higher thresholds
/// to avoid over-smoothing structure.
const LEVEL_SCALE: [f32; NUM_LEVELS] = [1.0, 1.0, 1.2, 1.5, 2.0];

fn denoise_channel(
    channel: &[f32],
    width: usize,
    height: usize,
    strength: f32,
    detail_factor: f32,
    is_luma: bool,
) -> Vec<f32> {
    if strength == 0.0 {
        return channel.to_vec();
    }
    let (mut details, residual) = atrous_decompose(channel, width, height);
    let sigma = estimate_sigma(&details[0]);

    for (level, detail) in details.iter_mut().enumerate() {
        let mut threshold = sigma * LEVEL_SCALE[level] * strength;
        if level == 0 && is_luma {
            threshold *= detail_factor;
        }
        soft_threshold(detail, threshold);
    }

    // Reconstruct: residual + sum(details)
    let mut result = residual;
    for detail in &details {
        for (i, v) in detail.iter().enumerate() {
            result[i] += v;
        }
    }
    result
}

/// Apply noise reduction to a linear RGB buffer.
///
/// Separates into YCbCr, applies à trous wavelet decomposition to each channel,
/// estimates noise via MAD, applies soft thresholding scaled by user parameters,
/// then reconstructs. All three channels are denoised in parallel.
pub fn apply_noise_reduction(
    pixels: &[[f32; 3]],
    width: usize,
    height: usize,
    params: &NoiseReductionParams,
) -> Vec<[f32; 3]> {
    if params.is_neutral() {
        return pixels.to_vec();
    }

    let (y_chan, cb_chan, cr_chan) = rgb_to_ycbcr(pixels);

    // Map user parameters (0–100) to multiplier range (0–3)
    let luma_strength = params.luminance / 100.0 * 3.0;
    let chroma_strength = params.color / 100.0 * 3.0;
    // Inverted: detail=100 → factor=0 (level-0 threshold zeroed, preserving fine detail)
    let detail_factor = 1.0 - params.detail / 100.0;

    // Denoise all three channels in parallel
    let (y_denoised, (cb_denoised, cr_denoised)) = rayon::join(
        || denoise_channel(&y_chan, width, height, luma_strength, detail_factor, true),
        || {
            rayon::join(
                || denoise_channel(&cb_chan, width, height, chroma_strength, detail_factor, false),
                || denoise_channel(&cr_chan, width, height, chroma_strength, detail_factor, false),
            )
        },
    );

    ycbcr_to_rgb(&y_denoised, &cb_denoised, &cr_denoised)
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
    fn estimate_noise_sigma_on_known_noise() {
        let n = 1000;
        let mut data = Vec::with_capacity(n);
        let mut rng: u64 = 42;
        for _ in 0..n {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u = (rng >> 33) as f32 / (1u64 << 31) as f32;
            data.push((u - 0.5) * 0.2);
        }
        let sigma = estimate_sigma(&data);
        assert!(
            sigma > 0.01 && sigma < 0.2,
            "sigma={sigma} out of expected range"
        );
    }

    #[test]
    fn soft_threshold_zero_threshold_is_identity() {
        let mut data = vec![-0.5, -0.1, 0.0, 0.1, 0.5];
        let original = data.clone();
        soft_threshold(&mut data, 0.0);
        for (i, v) in data.iter().enumerate() {
            assert!(
                (v - original[i]).abs() < 1e-7,
                "zero threshold should be identity at index {i}"
            );
        }
    }

    #[test]
    fn soft_threshold_removes_small_coefficients() {
        let mut data = vec![-0.5, -0.1, 0.0, 0.1, 0.5];
        soft_threshold(&mut data, 0.2);
        assert_eq!(data[1], 0.0);
        assert_eq!(data[2], 0.0);
        assert_eq!(data[3], 0.0);
        assert!((data[0] - (-0.3)).abs() < 1e-6);
        assert!((data[4] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn atrous_decompose_and_reconstruct_is_identity() {
        let w = 16;
        let h = 16;
        let input: Vec<f32> = (0..w * h).map(|i| i as f32 / (w * h) as f32).collect();
        let (details, residual) = atrous_decompose(&input, w, h);
        assert_eq!(details.len(), NUM_LEVELS);
        assert_eq!(residual.len(), w * h);
        let mut reconstructed = residual;
        for level in &details {
            for (i, v) in level.iter().enumerate() {
                reconstructed[i] += v;
            }
        }
        for i in 0..input.len() {
            assert!(
                (reconstructed[i] - input[i]).abs() < 1e-5,
                "pixel {i}: expected {}, got {}",
                input[i],
                reconstructed[i]
            );
        }
    }

    #[test]
    fn atrous_boundary_handling_small_image() {
        let w = 4;
        let h = 4;
        let input: Vec<f32> = (0..w * h).map(|i| i as f32 / 16.0).collect();
        let (details, residual) = atrous_decompose(&input, w, h);
        let mut reconstructed = residual;
        for level in &details {
            for (i, v) in level.iter().enumerate() {
                reconstructed[i] += v;
            }
        }
        for i in 0..input.len() {
            assert!(
                (reconstructed[i] - input[i]).abs() < 1e-5,
                "small image pixel {i}: expected {}, got {}",
                input[i],
                reconstructed[i]
            );
        }
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

    #[test]
    fn apply_nr_zero_amount_is_identity() {
        let pixels: Vec<[f32; 3]> = vec![[0.5, 0.3, 0.1]; 16];
        let params = NoiseReductionParams {
            luminance: 0.0,
            color: 0.0,
            detail: 0.0,
        };
        let result = apply_noise_reduction(&pixels, 4, 4, &params);
        for (i, px) in pixels.iter().enumerate() {
            for c in 0..3 {
                assert!(
                    (result[i][c] - px[c]).abs() < 1e-5,
                    "zero NR should be identity: pixel {i} channel {c}"
                );
            }
        }
    }

    #[test]
    fn apply_nr_luminance_reduces_luma_variation() {
        let w = 32;
        let h = 32;
        let mut pixels: Vec<[f32; 3]> = Vec::with_capacity(w * h);
        let mut rng: u64 = 123;
        for i in 0..(w * h) {
            let base = i as f32 / (w * h) as f32 * 0.5 + 0.25;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise = ((rng >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 0.1;
            let v = (base + noise).clamp(0.0, 1.0);
            pixels.push([v, v, v]);
        }
        let params = NoiseReductionParams {
            luminance: 50.0,
            color: 0.0,
            detail: 0.0,
        };
        let result = apply_noise_reduction(&pixels, w, h, &params);
        let luma_before: Vec<f32> = pixels
            .iter()
            .map(|p| LUMA_R * p[0] + LUMA_G * p[1] + LUMA_B * p[2])
            .collect();
        let luma_after: Vec<f32> = result
            .iter()
            .map(|p| LUMA_R * p[0] + LUMA_G * p[1] + LUMA_B * p[2])
            .collect();
        let var = |v: &[f32]| {
            let mean = v.iter().sum::<f32>() / v.len() as f32;
            v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / v.len() as f32
        };
        let var_before = var(&luma_before);
        let var_after = var(&luma_after);
        assert!(
            var_after < var_before,
            "NR should reduce luma variance: before={var_before}, after={var_after}"
        );
    }

    #[test]
    fn apply_nr_color_reduces_chroma_variation() {
        let w = 32;
        let h = 32;
        let mut pixels: Vec<[f32; 3]> = Vec::with_capacity(w * h);
        let mut rng: u64 = 456;
        for _ in 0..(w * h) {
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise_r = ((rng >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 0.15;
            rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
            let noise_b = ((rng >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 0.15;
            pixels.push([
                (0.5 + noise_r).clamp(0.0, 1.0),
                0.5,
                (0.5 + noise_b).clamp(0.0, 1.0),
            ]);
        }
        let params = NoiseReductionParams {
            luminance: 0.0,
            color: 50.0,
            detail: 0.0,
        };
        let result = apply_noise_reduction(&pixels, w, h, &params);
        let cb_before: Vec<f32> = pixels
            .iter()
            .map(|p| {
                let y = LUMA_R * p[0] + LUMA_G * p[1] + LUMA_B * p[2];
                p[2] - y
            })
            .collect();
        let cb_after: Vec<f32> = result
            .iter()
            .map(|p| {
                let y = LUMA_R * p[0] + LUMA_G * p[1] + LUMA_B * p[2];
                p[2] - y
            })
            .collect();
        let var = |v: &[f32]| {
            let mean = v.iter().sum::<f32>() / v.len() as f32;
            v.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / v.len() as f32
        };
        assert!(
            var(&cb_after) < var(&cb_before),
            "NR should reduce chroma variance"
        );
    }

    #[test]
    fn apply_nr_detail_preserves_edges() {
        let w = 32;
        let h = 32;
        let mut pixels: Vec<[f32; 3]> = Vec::with_capacity(w * h);
        let mut rng: u64 = 789;
        for _y in 0..h {
            for x in 0..w {
                let base = if x < 16 { 0.2 } else { 0.8 };
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                let noise = ((rng >> 33) as f32 / (1u64 << 31) as f32 - 0.5) * 0.05;
                let v = (base + noise).clamp(0.0, 1.0);
                pixels.push([v, v, v]);
            }
        }
        let high_detail = NoiseReductionParams {
            luminance: 50.0,
            color: 0.0,
            detail: 100.0,
        };
        let low_detail = NoiseReductionParams {
            luminance: 50.0,
            color: 0.0,
            detail: 0.0,
        };
        let result_hd = apply_noise_reduction(&pixels, w, h, &high_detail);
        let result_ld = apply_noise_reduction(&pixels, w, h, &low_detail);
        let mid = h / 2;
        let edge_hd = (result_hd[mid * w + 16][0] - result_hd[mid * w + 15][0]).abs();
        let edge_ld = (result_ld[mid * w + 16][0] - result_ld[mid * w + 15][0]).abs();
        assert!(
            edge_hd >= edge_ld,
            "High detail should preserve edges better: hd={edge_hd}, ld={edge_ld}"
        );
    }
}
