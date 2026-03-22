use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// Dehaze adjustment parameters. Amount range: -100 to +100. Positive removes haze,
/// negative adds haze/fog. When amount is 0, the dehaze pass is skipped entirely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DehazeParams {
    #[serde(default)]
    pub amount: f32,
}

impl Default for DehazeParams {
    fn default() -> Self {
        Self { amount: 0.0 }
    }
}

impl DehazeParams {
    /// Returns true when no dehaze effect would be applied.
    pub fn is_neutral(&self) -> bool {
        self.amount == 0.0
    }
}

const PATCH_SIZE: usize = 15;
const AIRLIGHT_PERCENTILE: f64 = 0.001;

/// O(n) centered sliding window minimum using a monotonic deque.
///
/// For each position `j` in `data`, computes the minimum value within a
/// symmetric window `[j - half, j + half]` (clamped to array bounds).
fn min_filter_1d(data: &[f32], window_size: usize) -> Vec<f32> {
    let n = data.len();
    if n == 0 {
        return Vec::new();
    }
    let half = window_size / 2;
    let mut result = vec![0.0_f32; n];
    let mut deque: VecDeque<usize> = VecDeque::new();

    for right in 0..(n + half) {
        // Add data[right] to deque if in bounds
        if right < n {
            while let Some(&back) = deque.back() {
                if data[back] >= data[right] {
                    deque.pop_back();
                } else {
                    break;
                }
            }
            deque.push_back(right);
        }

        // Output position: j = right - half
        let j = right as isize - half as isize;
        if j >= 0 && (j as usize) < n {
            let j = j as usize;
            let left = j.saturating_sub(half);
            while let Some(&front) = deque.front() {
                if front < left {
                    deque.pop_front();
                } else {
                    break;
                }
            }
            result[j] = data[deque[0]];
        }
    }

    result
}

/// Compute the dark channel of an RGB buffer.
///
/// For each pixel, computes the minimum value across all three RGB channels
/// within a local patch of `PATCH_SIZE` x `PATCH_SIZE` pixels.
/// Uses a separable 2D min filter (horizontal then vertical) for O(w*h) complexity.
fn dark_channel(buf: &[[f32; 3]], width: usize, height: usize) -> Vec<f32> {
    let n = width * height;

    // Step 1: Per-pixel minimum across RGB channels
    let mut pixel_min = vec![0.0_f32; n];
    for i in 0..n {
        let [r, g, b] = buf[i];
        pixel_min[i] = r.min(g).min(b);
    }

    // Step 2: Horizontal min filter (per row)
    let mut h_filtered = vec![0.0_f32; n];
    for y in 0..height {
        let row_start = y * width;
        let row = &pixel_min[row_start..row_start + width];
        let filtered = min_filter_1d(row, PATCH_SIZE);
        h_filtered[row_start..row_start + width].copy_from_slice(&filtered);
    }

    // Step 3: Vertical min filter (per column)
    let mut result = vec![0.0_f32; n];
    let mut col_buf = vec![0.0_f32; height];
    for x in 0..width {
        for y in 0..height {
            col_buf[y] = h_filtered[y * width + x];
        }
        let filtered = min_filter_1d(&col_buf, PATCH_SIZE);
        for y in 0..height {
            result[y * width + x] = filtered[y];
        }
    }

    result
}

/// Estimate the atmospheric light color from the image and its dark channel.
///
/// Selects the top 0.1% brightest pixels in the dark channel (haziest regions),
/// then picks the pixel with the highest RGB intensity among those.
fn estimate_airlight(buf: &[[f32; 3]], dark_ch: &[f32]) -> [f32; 3] {
    let n = buf.len();
    if n == 0 {
        return [1.0, 1.0, 1.0];
    }

    // Number of top pixels to consider (at least 1)
    let top_count = ((n as f64 * AIRLIGHT_PERCENTILE).ceil() as usize).max(1);

    // Sort indices by dark channel value (descending)
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_unstable_by(|&a, &b| dark_ch[b].partial_cmp(&dark_ch[a]).unwrap());

    // Among top dark channel pixels, find the one with highest intensity
    let mut best_idx = indices[0];
    let mut best_intensity = 0.0_f32;
    for &idx in indices.iter().take(top_count) {
        let [r, g, b] = buf[idx];
        let intensity = r + g + b;
        if intensity > best_intensity {
            best_intensity = intensity;
            best_idx = idx;
        }
    }

    buf[best_idx]
}

const GUIDED_FILTER_RADIUS: usize = 40;
const GUIDED_FILTER_EPSILON: f32 = 0.001;

/// O(n) box filter using running sums.
/// Computes the mean of each window of size (2*radius+1) centered at each position.
fn box_filter_1d(data: &[f32], radius: usize) -> Vec<f32> {
    let n = data.len();
    if n == 0 {
        return Vec::new();
    }
    let mut prefix = vec![0.0_f32; n + 1];
    for i in 0..n {
        prefix[i + 1] = prefix[i] + data[i];
    }
    let mut result = vec![0.0_f32; n];
    for i in 0..n {
        let left = if i >= radius { i - radius } else { 0 };
        let right = (i + radius).min(n - 1);
        let count = (right - left + 1) as f32;
        result[i] = (prefix[right + 1] - prefix[left]) / count;
    }
    result
}

/// 2D box filter (separable: horizontal then vertical).
fn box_filter_2d(data: &[f32], width: usize, height: usize, radius: usize) -> Vec<f32> {
    let n = width * height;
    // Horizontal pass
    let mut h_filtered = vec![0.0_f32; n];
    for y in 0..height {
        let row_start = y * width;
        let row = &data[row_start..row_start + width];
        let filtered = box_filter_1d(row, radius);
        h_filtered[row_start..row_start + width].copy_from_slice(&filtered);
    }
    // Vertical pass
    let mut result = vec![0.0_f32; n];
    let mut col = vec![0.0_f32; height];
    for x in 0..width {
        for y in 0..height {
            col[y] = h_filtered[y * width + x];
        }
        let filtered = box_filter_1d(&col, radius);
        for y in 0..height {
            result[y * width + x] = filtered[y];
        }
    }
    result
}

/// Guided filter: edge-aware smoothing of `input` using `guide` as reference.
///
/// Implements He et al. 2010. Guide is grayscale (single channel), input is the
/// raw transmission map. Uses box filters for O(n) complexity.
fn guided_filter(guide: &[f32], input: &[f32], width: usize, height: usize) -> Vec<f32> {
    let r = GUIDED_FILTER_RADIUS;
    let eps = GUIDED_FILTER_EPSILON;
    let n = width * height;

    let mean_g = box_filter_2d(guide, width, height, r);
    let mean_p = box_filter_2d(input, width, height, r);

    let mut gp = vec![0.0_f32; n];
    let mut gg = vec![0.0_f32; n];
    for i in 0..n {
        gp[i] = guide[i] * input[i];
        gg[i] = guide[i] * guide[i];
    }
    let mean_gp = box_filter_2d(&gp, width, height, r);
    let mean_gg = box_filter_2d(&gg, width, height, r);

    let mut a = vec![0.0_f32; n];
    let mut b = vec![0.0_f32; n];
    for i in 0..n {
        let cov_gp = mean_gp[i] - mean_g[i] * mean_p[i];
        let var_g = mean_gg[i] - mean_g[i] * mean_g[i];
        a[i] = cov_gp / (var_g + eps);
        b[i] = mean_p[i] - a[i] * mean_g[i];
    }

    let mean_a = box_filter_2d(&a, width, height, r);
    let mean_b = box_filter_2d(&b, width, height, r);
    let mut result = vec![0.0_f32; n];
    for i in 0..n {
        result[i] = mean_a[i] * guide[i] + mean_b[i];
    }
    result
}

const T_MIN: f32 = 0.1;
const LUMA_R: f32 = 0.2126;
const LUMA_G: f32 = 0.7152;
const LUMA_B: f32 = 0.0722;

/// Apply dehaze adjustment to a linear RGB buffer.
///
/// The buffer contains pixels in linear sRGB space (after white balance and exposure).
/// Positive amount removes haze, negative amount adds haze/fog.
/// Returns a new buffer of the same size.
pub fn apply_dehaze(
    buf: &[[f32; 3]],
    width: usize,
    height: usize,
    params: &DehazeParams,
) -> Vec<[f32; 3]> {
    if params.is_neutral() {
        return buf.to_vec();
    }

    let n = width * height;
    let amount = params.amount;

    // Step 1: Dark channel of the original image
    let dc = dark_channel(buf, width, height);

    // Step 2: Atmospheric light estimation
    let a = estimate_airlight(buf, &dc);

    if amount < 0.0 {
        // Negative amount: add haze by blending toward airlight
        let strength = (-amount / 100.0).min(1.0);
        let mut result = vec![[0.0_f32; 3]; n];
        for i in 0..n {
            for c in 0..3 {
                result[i][c] = (buf[i][c] * (1.0 - strength) + a[c] * strength).clamp(0.0, 1.0);
            }
        }
        return result;
    }

    // Positive amount: remove haze
    let omega = (amount / 100.0).min(1.0);

    // Step 3: Normalize image by airlight and compute dark channel of normalized
    let a_safe = [a[0].max(0.01), a[1].max(0.01), a[2].max(0.01)];
    let mut normalized = vec![[0.0_f32; 3]; n];
    for i in 0..n {
        normalized[i] = [
            buf[i][0] / a_safe[0],
            buf[i][1] / a_safe[1],
            buf[i][2] / a_safe[2],
        ];
    }
    let dc_norm = dark_channel(&normalized, width, height);

    // Raw transmission map
    let mut t_raw = vec![0.0_f32; n];
    for i in 0..n {
        t_raw[i] = 1.0 - omega * dc_norm[i];
    }

    // Step 4: Guided filter refinement
    let mut guide = vec![0.0_f32; n];
    for i in 0..n {
        let [r, g, b] = buf[i];
        guide[i] = LUMA_R * r + LUMA_G * g + LUMA_B * b;
    }
    let t_refined = guided_filter(&guide, &t_raw, width, height);

    // Step 5: Scene recovery
    let mut result = vec![[0.0_f32; 3]; n];
    for i in 0..n {
        let t = t_refined[i].max(T_MIN);
        for c in 0..3 {
            let recovered = (buf[i][c] - a[c]) / t + a[c];
            result[i][c] = recovered.clamp(0.0, 1.0);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params_are_neutral() {
        let p = DehazeParams::default();
        assert_eq!(p.amount, 0.0);
        assert!(p.is_neutral());
    }

    #[test]
    fn non_zero_amount_is_not_neutral() {
        let p = DehazeParams { amount: 50.0 };
        assert!(!p.is_neutral());
    }

    #[test]
    fn negative_amount_is_not_neutral() {
        let p = DehazeParams { amount: -30.0 };
        assert!(!p.is_neutral());
    }

    #[test]
    fn dark_channel_uniform_buffer() {
        let buf = vec![[0.5_f32, 0.5, 0.5]; 4];
        let dc = dark_channel(&buf, 2, 2);
        for &v in &dc {
            assert!((v - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn dark_channel_picks_min_rgb() {
        let buf = vec![[0.8, 0.3, 0.6]; 1];
        let dc = dark_channel(&buf, 1, 1);
        assert!((dc[0] - 0.3).abs() < 1e-6);
    }

    #[test]
    fn dark_channel_spreads_minimum_across_patch() {
        let mut buf = vec![[0.9, 0.9, 0.9]; 9]; // 3x3
        buf[4] = [0.1, 0.1, 0.1]; // center pixel is dark
        let dc = dark_channel(&buf, 3, 3);
        // With PATCH_SIZE=15, the entire 3x3 image is within one patch
        for &v in &dc {
            assert!((v - 0.1).abs() < 1e-6, "Expected 0.1, got {v}");
        }
    }

    #[test]
    fn airlight_selects_brightest_in_haziest_region() {
        let mut buf = vec![[0.1, 0.1, 0.1]; 16]; // 4x4 clear
        buf[0] = [0.9, 0.85, 0.8]; // bright hazy pixel (high min channel)
        let dc = dark_channel(&buf, 4, 4);
        let a = estimate_airlight(&buf, &dc);
        assert!(a[0] > 0.5, "Expected bright airlight R, got {}", a[0]);
    }

    #[test]
    fn guided_filter_uniform_input_is_identity() {
        let guide = vec![0.5_f32; 9];
        let input = vec![0.7_f32; 9];
        let result = guided_filter(&guide, &input, 3, 3);
        for &v in &result {
            assert!((v - 0.7).abs() < 1e-4, "Expected ~0.7, got {v}");
        }
    }

    #[test]
    fn guided_filter_preserves_step_edge() {
        let width = 20;
        let height = 1;
        let mut guide = vec![0.0_f32; width];
        let mut input = vec![0.0_f32; width];
        for i in width / 2..width {
            guide[i] = 1.0;
            input[i] = 1.0;
        }
        let result = guided_filter(&guide, &input, width, height);
        assert!(result[0] < 0.3, "Left should be dark, got {}", result[0]);
        assert!(
            result[width - 1] > 0.7,
            "Right should be bright, got {}",
            result[width - 1]
        );
    }

    #[test]
    fn apply_dehaze_zero_amount_is_identity() {
        let buf = vec![[0.5, 0.3, 0.7]; 4];
        let params = DehazeParams { amount: 0.0 };
        let result = apply_dehaze(&buf, 2, 2, &params);
        for (i, px) in result.iter().enumerate() {
            for c in 0..3 {
                assert!(
                    (px[c] - buf[i][c]).abs() < 1e-6,
                    "Pixel {i} channel {c}: expected {}, got {}",
                    buf[i][c],
                    px[c]
                );
            }
        }
    }

    #[test]
    fn apply_dehaze_positive_changes_output() {
        // Non-uniform image simulating a hazy scene with variation
        let mut buf = Vec::with_capacity(100);
        for i in 0..100 {
            let base = 0.5 + 0.3 * (i as f32 / 100.0);
            buf.push([base, base * 0.9, base * 0.85]);
        }
        let params = DehazeParams { amount: 50.0 };
        let result = apply_dehaze(&buf, 10, 10, &params);
        let differs = result
            .iter()
            .zip(buf.iter())
            .any(|(r, b)| (r[0] - b[0]).abs() > 1e-4);
        assert!(differs, "Dehaze should change hazy image");
    }

    #[test]
    fn apply_dehaze_negative_adds_haze() {
        // Non-uniform image: negative dehaze blends toward estimated airlight
        let mut buf = Vec::with_capacity(100);
        for i in 0..100 {
            let t = i as f32 / 100.0;
            buf.push([0.2 + 0.5 * t, 0.3 + 0.3 * t, 0.1 + 0.4 * t]);
        }
        let params = DehazeParams { amount: -30.0 };
        let result = apply_dehaze(&buf, 10, 10, &params);
        let differs = result
            .iter()
            .zip(buf.iter())
            .any(|(r, b)| (r[0] - b[0]).abs() > 1e-4);
        assert!(differs, "Negative dehaze should add haze");
    }

    #[test]
    fn apply_dehaze_output_clamped_to_0_1() {
        let buf = vec![[0.95, 0.95, 0.95]; 100]; // very bright, 10x10
        let params = DehazeParams { amount: 100.0 };
        let result = apply_dehaze(&buf, 10, 10, &params);
        for px in &result {
            for c in 0..3 {
                assert!(
                    px[c] >= 0.0 && px[c] <= 1.0,
                    "Output {:.4} out of [0,1]",
                    px[c]
                );
            }
        }
    }

    #[test]
    fn apply_dehaze_t_min_prevents_extreme_values() {
        let buf = vec![[0.8, 0.8, 0.8]; 100]; // 10x10 very hazy
        let params = DehazeParams { amount: 100.0 };
        let result = apply_dehaze(&buf, 10, 10, &params);
        for px in &result {
            for c in 0..3 {
                assert!(
                    px[c] >= 0.0 && px[c] <= 1.0,
                    "T_MIN should prevent extreme values, got {:.4}",
                    px[c]
                );
            }
        }
    }
}
