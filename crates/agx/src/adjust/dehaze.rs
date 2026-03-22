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
}
