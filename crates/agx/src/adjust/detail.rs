//! Detail pass: sharpening, clarity, and texture controls built on a multi-scale unsharp mask.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Minimum supported sharpening amount.
pub const SHARPEN_AMOUNT_MIN: f32 = 0.0;
/// Maximum supported sharpening amount.
pub const SHARPEN_AMOUNT_MAX: f32 = 100.0;
/// Minimum supported sharpening radius.
pub const SHARPEN_RADIUS_MIN: f32 = 0.5;
/// Maximum supported sharpening radius.
pub const SHARPEN_RADIUS_MAX: f32 = 3.0;
/// Minimum supported sharpening threshold.
pub const SHARPEN_THRESHOLD_MIN: f32 = 0.0;
/// Maximum supported sharpening threshold.
pub const SHARPEN_THRESHOLD_MAX: f32 = 100.0;
/// Minimum supported sharpening masking value.
pub const SHARPEN_MASKING_MIN: f32 = 0.0;
/// Maximum supported sharpening masking value.
pub const SHARPEN_MASKING_MAX: f32 = 100.0;
/// Minimum supported detail slider value for clarity and texture.
pub const DETAIL_SLIDER_MIN: f32 = -100.0;
/// Maximum supported detail slider value for clarity and texture.
pub const DETAIL_SLIDER_MAX: f32 = 100.0;
/// Default sharpening radius.
pub const SHARPEN_RADIUS_DEFAULT: f32 = 1.0;
/// Default sharpening threshold.
pub const SHARPEN_THRESHOLD_DEFAULT: f32 = 25.0;

fn default_sharpening_radius() -> f32 {
    SHARPEN_RADIUS_DEFAULT
}
fn default_sharpening_threshold() -> f32 {
    SHARPEN_THRESHOLD_DEFAULT
}

/// Sharpening parameters controlling unsharp-mask strength, radius, threshold, and edge masking.
#[cfg_attr(feature = "docgen", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharpeningParams {
    /// Sharpening intensity, 0–100. Default 0 (off).
    #[serde(default)]
    #[cfg_attr(feature = "docgen", schemars(range(min = 0.0, max = 100.0)))]
    pub amount: f32,
    /// Gaussian blur radius in pixels. Larger radius sharpens coarser detail. Default 1.0.
    #[serde(default = "default_sharpening_radius")]
    #[cfg_attr(feature = "docgen", schemars(range(min = 0.5, max = 3.0)))]
    pub radius: f32,
    /// Minimum luminance delta (0–255 scale) below which sharpening is suppressed. Default 25.
    #[serde(default = "default_sharpening_threshold")]
    #[cfg_attr(feature = "docgen", schemars(range(min = 0.0, max = 100.0)))]
    pub threshold: f32,
    /// Edge-aware masking strength, 0–100. Higher restricts sharpening to strong edges. Default 0.
    #[serde(default)]
    #[cfg_attr(feature = "docgen", schemars(range(min = 0.0, max = 100.0)))]
    pub masking: f32,
}

impl Default for SharpeningParams {
    fn default() -> Self {
        Self {
            amount: 0.0,
            radius: default_sharpening_radius(),
            threshold: default_sharpening_threshold(),
            masking: 0.0,
        }
    }
}

/// Combined detail parameters: sharpening, clarity, and texture.
///
/// Each control targets a different spatial frequency range via unsharp mask sigma.
#[cfg_attr(feature = "docgen", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailParams {
    /// Sharpening sub-parameters (amount, radius, threshold, masking).
    #[serde(default)]
    pub sharpening: SharpeningParams,
    /// Mid-frequency local contrast, -100 to +100. Positive adds punch, negative softens.
    #[serde(default)]
    #[cfg_attr(feature = "docgen", schemars(range(min = -100.0, max = 100.0)))]
    pub clarity: f32,
    /// Fine-frequency detail, -100 to +100. Positive enhances fine texture, negative smooths.
    #[serde(default)]
    #[cfg_attr(feature = "docgen", schemars(range(min = -100.0, max = 100.0)))]
    pub texture: f32,
}

impl Default for DetailParams {
    fn default() -> Self {
        Self {
            sharpening: SharpeningParams::default(),
            clarity: 0.0,
            texture: 0.0,
        }
    }
}

impl DetailParams {
    /// Returns true when no detail effect would be applied.
    ///
    /// Only checks the "active" fields (sharpening amount, clarity, texture).
    /// Sharpening radius/threshold/masking are irrelevant when amount is 0.
    pub fn is_neutral(&self) -> bool {
        self.sharpening.amount == 0.0 && self.clarity == 0.0 && self.texture == 0.0
    }
}

// --- Gaussian blur (separable 2-pass) ---

pub(crate) fn build_gaussian_kernel(sigma: f32) -> Vec<f32> {
    let half = (3.0 * sigma).ceil() as usize;
    let size = 2 * half + 1;
    let mut kernel = Vec::with_capacity(size);
    let denom = 2.0 * sigma * sigma;
    for i in 0..size {
        let x = i as f32 - half as f32;
        kernel.push((-x * x / denom).exp());
    }
    let sum: f32 = kernel.iter().sum();
    for w in &mut kernel {
        *w /= sum;
    }
    kernel
}

pub(crate) fn gaussian_blur(input: &[f32], width: usize, height: usize, sigma: f32) -> Vec<f32> {
    let kernel = build_gaussian_kernel(sigma);
    let half = kernel.len() / 2;

    // Horizontal pass: each row reads from `input`, writes to its own row in `temp`.
    let mut temp = vec![0.0f32; width * height];
    temp.par_chunks_mut(width).enumerate().for_each(|(y, row)| {
        for (x, out) in row.iter_mut().enumerate() {
            let mut sum = 0.0f32;
            for (ki, &kw) in kernel.iter().enumerate() {
                let sx = (x as isize + ki as isize - half as isize)
                    .max(0)
                    .min(width as isize - 1) as usize;
                sum += input[y * width + sx] * kw;
            }
            *out = sum;
        }
    });

    // Vertical pass: each row reads from `temp` (immutable), writes to its own row in `output`.
    let mut output = vec![0.0f32; width * height];
    output
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row)| {
            for (x, out) in row.iter_mut().enumerate() {
                let mut sum = 0.0f32;
                for (ki, &kw) in kernel.iter().enumerate() {
                    let sy = (y as isize + ki as isize - half as isize)
                        .max(0)
                        .min(height as isize - 1) as usize;
                    sum += temp[sy * width + x] * kw;
                }
                *out = sum;
            }
        });

    output
}

// --- Luminance extraction and unsharp mask ---

fn extract_luminance(buf: &[[f32; 3]], width: usize, height: usize) -> Vec<f32> {
    debug_assert_eq!(buf.len(), width * height);
    buf.iter()
        .map(|px| super::LUMA_R * px[0] + super::LUMA_G * px[1] + super::LUMA_B * px[2])
        .collect()
}

fn apply_unsharp_mask(
    buf: &[[f32; 3]],
    width: usize,
    height: usize,
    sigma: f32,
    amount: f32,
) -> Vec<[f32; 3]> {
    if amount == 0.0 {
        return buf.to_vec();
    }
    let strength = amount / 100.0;
    let luminance = extract_luminance(buf, width, height);
    let blurred = gaussian_blur(&luminance, width, height, sigma);
    buf.iter()
        .enumerate()
        .map(|(i, px)| {
            let high_freq = luminance[i] - blurred[i];
            let delta = strength * high_freq;
            [
                (px[0] + delta).clamp(0.0, 1.0),
                (px[1] + delta).clamp(0.0, 1.0),
                (px[2] + delta).clamp(0.0, 1.0),
            ]
        })
        .collect()
}

// --- Sharpening with threshold and masking ---

const EDGE_SCALE: f32 = 4.0;

fn compute_edge_map(luminance: &[f32], width: usize, height: usize) -> Vec<f32> {
    let mut edge_map = vec![0.0f32; width * height];
    for y in 0..height {
        for x in 0..width {
            let xp = (x + 1).min(width - 1);
            let xm = x.saturating_sub(1);
            let yp = (y + 1).min(height - 1);
            let ym = y.saturating_sub(1);
            let dx = luminance[y * width + xp] - luminance[y * width + xm];
            let dy = luminance[yp * width + x] - luminance[ym * width + x];
            edge_map[y * width + x] = (dx * dx + dy * dy).sqrt() * EDGE_SCALE;
        }
    }
    edge_map
}

use super::smoothstep;

fn apply_sharpening(
    buf: &[[f32; 3]],
    width: usize,
    height: usize,
    params: &SharpeningParams,
) -> Vec<[f32; 3]> {
    if params.amount == 0.0 {
        return buf.to_vec();
    }
    let strength = params.amount / 100.0;
    let sigma = params.radius.max(0.1);
    let threshold = params.threshold / 255.0;
    let luminance = extract_luminance(buf, width, height);
    let blurred = gaussian_blur(&luminance, width, height, sigma);
    let edge_map = if params.masking > 0.0 {
        Some(compute_edge_map(&luminance, width, height))
    } else {
        None
    };
    buf.iter()
        .enumerate()
        .map(|(i, px)| {
            let high_freq = luminance[i] - blurred[i];
            if high_freq.abs() < threshold {
                return *px;
            }
            let mask = if let Some(ref em) = edge_map {
                let masking_norm = params.masking / 100.0;
                smoothstep(0.0, masking_norm, em[i])
            } else {
                1.0
            };
            let delta = strength * high_freq * mask;
            [
                (px[0] + delta).clamp(0.0, 1.0),
                (px[1] + delta).clamp(0.0, 1.0),
                (px[2] + delta).clamp(0.0, 1.0),
            ]
        })
        .collect()
}

// --- Detail pass orchestrator ---

const TEXTURE_SIGMA: f32 = 3.0;
const CLARITY_SIGMA: f32 = 20.0;

/// Apply the full detail pass (texture, clarity, sharpening) to a pixel buffer.
///
/// This is the primary public interface for the detail module. The engine calls
/// this after tone mapping and color grading, operating on linear-light RGB pixels
/// in \[0, 1\].
pub fn apply_detail_pass(
    buf: &[[f32; 3]],
    width: usize,
    height: usize,
    params: &DetailParams,
) -> Vec<[f32; 3]> {
    if params.is_neutral() {
        return buf.to_vec();
    }
    let mut current = buf.to_vec();
    if params.texture != 0.0 {
        current = apply_unsharp_mask(&current, width, height, TEXTURE_SIGMA, params.texture);
    }
    if params.clarity != 0.0 {
        current = apply_unsharp_mask(&current, width, height, CLARITY_SIGMA, params.clarity);
    }
    if params.sharpening.amount != 0.0 {
        current = apply_sharpening(&current, width, height, &params.sharpening);
    }
    current
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    // --- Task 1: SharpeningParams and DetailParams ---

    #[test]
    fn default_detail_params_is_neutral() {
        let p = DetailParams::default();
        assert!(p.is_neutral());
    }

    #[test]
    fn is_neutral_ignores_radius_threshold_masking() {
        let p = DetailParams {
            sharpening: SharpeningParams {
                amount: 0.0,
                radius: 2.5,
                threshold: 10.0,
                masking: 50.0,
            },
            clarity: 0.0,
            texture: 0.0,
        };
        assert!(p.is_neutral());
    }

    #[test]
    fn is_neutral_false_when_sharpening_active() {
        let p = DetailParams {
            sharpening: SharpeningParams {
                amount: 50.0,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!p.is_neutral());
    }

    #[test]
    fn is_neutral_false_when_clarity_active() {
        let p = DetailParams {
            clarity: 30.0,
            ..Default::default()
        };
        assert!(!p.is_neutral());
    }

    #[test]
    fn is_neutral_false_when_texture_active() {
        let p = DetailParams {
            texture: -20.0,
            ..Default::default()
        };
        assert!(!p.is_neutral());
    }

    #[test]
    fn sharpening_default_values() {
        let s = SharpeningParams::default();
        assert_eq!(s.amount, 0.0);
        assert_eq!(s.radius, 1.0);
        assert_eq!(s.threshold, 25.0);
        assert_eq!(s.masking, 0.0);
    }

    // --- Task 2: Gaussian blur ---

    #[test]
    fn gaussian_kernel_sums_to_one() {
        for &sigma in &[0.5f32, 1.0, 2.0, 5.0] {
            let kernel = build_gaussian_kernel(sigma);
            let sum: f32 = kernel.iter().sum();
            assert!((sum - 1.0).abs() < 1e-5, "sigma={sigma} sum={sum}");
        }
    }

    #[test]
    fn gaussian_kernel_radius_3_wider_than_1() {
        let k1 = build_gaussian_kernel(1.0);
        let k3 = build_gaussian_kernel(3.0);
        assert!(k3.len() > k1.len());
    }

    #[test]
    fn gaussian_blur_uniform_is_identity() {
        let width = 8;
        let height = 8;
        let input = vec![0.5f32; width * height];
        let output = gaussian_blur(&input, width, height, 1.5);
        for &v in &output {
            assert!((v - 0.5).abs() < 1e-5, "expected 0.5 got {v}");
        }
    }

    #[test]
    fn separable_blur_matches_naive_2d() {
        // Build a small non-uniform image and verify separable == naive 2D convolution.
        let width = 5;
        let height = 5;
        let input: Vec<f32> = (0..width * height).map(|i| (i as f32) / 25.0).collect();
        let sigma = 1.0;
        let kernel = build_gaussian_kernel(sigma);
        let half = kernel.len() / 2;

        // Naive 2D
        let mut naive = vec![0.0f32; width * height];
        for y in 0..height {
            for x in 0..width {
                let mut sum = 0.0f32;
                for (ky, &kwy) in kernel.iter().enumerate() {
                    for (kx, &kwx) in kernel.iter().enumerate() {
                        let sx = (x as isize + kx as isize - half as isize)
                            .max(0)
                            .min(width as isize - 1) as usize;
                        let sy = (y as isize + ky as isize - half as isize)
                            .max(0)
                            .min(height as isize - 1) as usize;
                        sum += input[sy * width + sx] * kwy * kwx;
                    }
                }
                naive[y * width + x] = sum;
            }
        }

        let separable = gaussian_blur(&input, width, height, sigma);
        for (i, (&n, &s)) in naive.iter().zip(separable.iter()).enumerate() {
            assert!((n - s).abs() < 1e-5, "pixel {i}: naive={n} separable={s}");
        }
    }

    #[test]
    fn gaussian_blur_smooths_impulse() {
        let width = 9;
        let height = 9;
        let mut input = vec![0.0f32; width * height];
        // Place a spike in the center
        input[4 * width + 4] = 1.0;
        let output = gaussian_blur(&input, width, height, 1.0);
        // Center should be less than 1.0 (energy spread)
        assert!(output[4 * width + 4] < 1.0);
        // Neighbors should be non-zero
        assert!(output[4 * width + 5] > 0.0);
        assert!(output[5 * width + 4] > 0.0);
    }

    // --- Task 3: Luminance extraction and unsharp mask ---

    #[test]
    fn extract_luminance_grayscale() {
        // For a gray pixel (v, v, v), luminance == v since R+G+B coefficients sum to 1.
        let pixels: Vec<[f32; 3]> = vec![[0.5, 0.5, 0.5], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]];
        let luma = extract_luminance(&pixels, 3, 1);
        assert!((luma[0] - 0.5).abs() < 1e-5);
        assert!((luma[1] - 0.0).abs() < 1e-5);
        assert!((luma[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn unsharp_mask_zero_strength_is_identity() {
        let pixels: Vec<[f32; 3]> = vec![[0.2, 0.4, 0.6]; 16];
        let result = apply_unsharp_mask(&pixels, 4, 4, 1.0, 0.0);
        assert_eq!(result, pixels);
    }

    fn variance_of_channel(buf: &[[f32; 3]], ch: usize) -> f32 {
        let n = buf.len() as f32;
        let mean: f32 = buf.iter().map(|px| px[ch]).sum::<f32>() / n;
        buf.iter().map(|px| (px[ch] - mean).powi(2)).sum::<f32>() / n
    }

    #[test]
    fn negative_clarity_smooths() {
        // A checkerboard pattern has high variance; applying negative clarity should reduce variance.
        let width = 8;
        let height = 8;
        let pixels: Vec<[f32; 3]> = (0..width * height)
            .map(|i| {
                let v = if (i / width + i % width) % 2 == 0 {
                    0.8f32
                } else {
                    0.2f32
                };
                [v, v, v]
            })
            .collect();
        let before = variance_of_channel(&pixels, 0);
        let result = apply_unsharp_mask(&pixels, width, height, CLARITY_SIGMA, -50.0);
        let after = variance_of_channel(&result, 0);
        assert!(
            after < before,
            "expected variance to decrease: before={before} after={after}"
        );
    }

    // --- Task 4: Sharpening with threshold and masking ---

    #[test]
    fn edge_map_uniform_is_zero() {
        let luma = vec![0.5f32; 16];
        let edge_map = compute_edge_map(&luma, 4, 4);
        for &v in &edge_map {
            assert!(
                v.abs() < 1e-5,
                "expected zero edge in uniform image, got {v}"
            );
        }
    }

    #[test]
    fn edge_map_detects_sharp_edge() {
        // Left half = 0.0, right half = 1.0 → strong horizontal gradient.
        let width = 8;
        let height = 4;
        let luma: Vec<f32> = (0..width * height)
            .map(|i| if i % width < width / 2 { 0.0 } else { 1.0 })
            .collect();
        let edge_map = compute_edge_map(&luma, width, height);
        // Pixels at the boundary (column 3 and 4) should have high edge response.
        let boundary_edge = edge_map[1 * width + 3]; // row 1, col 3
        assert!(
            boundary_edge > 0.5,
            "expected large edge at boundary, got {boundary_edge}"
        );
    }

    #[test]
    fn apply_sharpening_zero_amount_is_identity() {
        let pixels: Vec<[f32; 3]> = vec![[0.3, 0.5, 0.7]; 16];
        let params = SharpeningParams {
            amount: 0.0,
            ..Default::default()
        };
        let result = apply_sharpening(&pixels, 4, 4, &params);
        assert_eq!(result, pixels);
    }

    #[test]
    fn sharpening_increases_edge_contrast() {
        // Create a soft edge (blurred step) then sharpen it — the sharpened version
        // should have higher variance than the input.
        let width = 16;
        let height = 4;
        // Soft ramp from 0 to 1 across width
        let pixels: Vec<[f32; 3]> = (0..width * height)
            .map(|i| {
                let x = (i % width) as f32 / (width - 1) as f32;
                [x, x, x]
            })
            .collect();
        let params = SharpeningParams {
            amount: 100.0,
            radius: 1.0,
            threshold: 0.0,
            masking: 0.0,
        };
        let result = apply_sharpening(&pixels, width, height, &params);
        let before = variance_of_channel(&pixels, 0);
        let after = variance_of_channel(&result, 0);
        assert!(
            after > before,
            "expected sharpening to increase variance: before={before} after={after}"
        );
    }

    // --- Task 5: apply_detail_pass orchestrator ---

    #[test]
    fn apply_detail_pass_default_is_identity() {
        let pixels: Vec<[f32; 3]> = vec![[0.2, 0.5, 0.8]; 16];
        let params = DetailParams::default();
        let result = apply_detail_pass(&pixels, 4, 4, &params);
        assert_eq!(result, pixels);
    }

    #[test]
    fn apply_detail_pass_all_active_changes_output() {
        let width = 16;
        let height = 16;
        let pixels: Vec<[f32; 3]> = (0..width * height)
            .map(|i| {
                let x = (i % width) as f32 / (width - 1) as f32;
                [x, x * 0.8, x * 0.6]
            })
            .collect();
        let params = DetailParams {
            sharpening: SharpeningParams {
                amount: 50.0,
                radius: 1.0,
                threshold: 0.0,
                masking: 0.0,
            },
            clarity: 30.0,
            texture: 20.0,
        };
        let result = apply_detail_pass(&pixels, width, height, &params);
        assert_ne!(result, pixels, "expected detail pass to change pixels");
    }
}
