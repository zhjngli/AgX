use serde::{Deserialize, Serialize};

use super::{LUMA_B, LUMA_G, LUMA_R};

/// Grain type controlling the internal character of the noise.
///
/// Each type maps to a combination of octave weights, contrast curve,
/// and luminance falloff strength. Matches Capture One's grain type model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrainType {
    Fine,
    #[default]
    Silver,
    Soft,
    Cubic,
    Tabular,
    Harsh,
}

/// Film grain simulation parameters.
///
/// - `grain_type`: selects the grain character (default: Silver)
/// - `amount`: 0–100, intensity of grain effect (0 = no grain)
/// - `size`: 0–100, fine to coarse grain (default: 50)
/// - `chromatic`: 0–100, strength of per-channel color variation (0 = luminance-only)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrainParams {
    #[serde(default)]
    pub grain_type: GrainType,
    #[serde(default)]
    pub amount: f32,
    #[serde(default = "default_size")]
    pub size: f32,
    #[serde(default)]
    pub chromatic: f32,
}

fn default_size() -> f32 {
    50.0
}

impl Default for GrainParams {
    fn default() -> Self {
        Self {
            grain_type: GrainType::default(),
            amount: 0.0,
            size: 50.0,
            chromatic: 0.0,
        }
    }
}

impl GrainParams {
    /// Returns true when no grain effect would be applied.
    pub fn is_neutral(&self) -> bool {
        self.amount == 0.0
    }
}

/// Standard 2D simplex noise gradient table (12 directions).
const GRAD2: [[f32; 2]; 12] = [
    [1.0, 1.0], [-1.0, 1.0], [1.0, -1.0], [-1.0, -1.0],
    [1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0],
    [1.0, 1.0], [-1.0, 1.0], [1.0, -1.0], [-1.0, -1.0],
];

/// Skewing factor for 2D simplex grid: (sqrt(3) - 1) / 2.
const F2: f32 = 0.366_025_4;
/// Unskewing factor: (3 - sqrt(3)) / 6.
const G2: f32 = 0.211_324_87;

/// Build a seeded 256-entry permutation table for simplex noise.
fn build_permutation_table(seed: u64) -> [u8; 512] {
    let mut perm = [0u8; 256];
    for i in 0..256 {
        perm[i] = i as u8;
    }
    // Fisher-Yates shuffle with simple LCG seeded PRNG
    let mut rng = seed;
    for i in (1..256).rev() {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let j = (rng >> 33) as usize % (i + 1);
        perm.swap(i, j);
    }
    // Double the table to avoid index wrapping
    let mut table = [0u8; 512];
    for i in 0..512 {
        table[i] = perm[i & 255];
    }
    table
}

/// 2D simplex noise, returns value in approximately [-1, 1].
fn simplex_noise_2d(x: f32, y: f32, perm: &[u8; 512]) -> f32 {
    let s = (x + y) * F2;
    let i = (x + s).floor() as i32;
    let j = (y + s).floor() as i32;

    let t = (i + j) as f32 * G2;
    let x0 = x - (i as f32 - t);
    let y0 = y - (j as f32 - t);

    let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

    let x1 = x0 - i1 as f32 + G2;
    let y1 = y0 - j1 as f32 + G2;
    let x2 = x0 - 1.0 + 2.0 * G2;
    let y2 = y0 - 1.0 + 2.0 * G2;

    let ii = (i & 255) as usize;
    let jj = (j & 255) as usize;

    let mut n = 0.0f32;

    let t0 = 0.5 - x0 * x0 - y0 * y0;
    if t0 > 0.0 {
        let t0 = t0 * t0;
        let gi = perm[ii + perm[jj] as usize] as usize % 12;
        n += t0 * t0 * (GRAD2[gi][0] * x0 + GRAD2[gi][1] * y0);
    }

    let t1 = 0.5 - x1 * x1 - y1 * y1;
    if t1 > 0.0 {
        let t1 = t1 * t1;
        let gi = perm[ii + i1 + perm[jj + j1] as usize] as usize % 12;
        n += t1 * t1 * (GRAD2[gi][0] * x1 + GRAD2[gi][1] * y1);
    }

    let t2 = 0.5 - x2 * x2 - y2 * y2;
    if t2 > 0.0 {
        let t2 = t2 * t2;
        let gi = perm[ii + 1 + perm[jj + 1] as usize] as usize % 12;
        n += t2 * t2 * (GRAD2[gi][0] * x2 + GRAD2[gi][1] * y2);
    }

    // Scale to approximately [-1, 1]
    70.0 * n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grain_params_default_is_neutral() {
        let p = GrainParams::default();
        assert!(p.is_neutral());
    }

    #[test]
    fn grain_params_nonzero_amount_not_neutral() {
        let p = GrainParams {
            amount: 50.0,
            ..Default::default()
        };
        assert!(!p.is_neutral());
    }

    #[test]
    fn grain_type_default_is_silver() {
        assert_eq!(GrainType::default(), GrainType::Silver);
    }

    #[test]
    fn simplex_noise_is_deterministic() {
        let perm = build_permutation_table(42);
        let a = simplex_noise_2d(1.0, 2.0, &perm);
        let b = simplex_noise_2d(1.0, 2.0, &perm);
        assert_eq!(a, b);
    }

    #[test]
    fn simplex_noise_in_range() {
        let perm = build_permutation_table(42);
        for i in 0..100 {
            for j in 0..100 {
                let v = simplex_noise_2d(i as f32 * 0.1, j as f32 * 0.1, &perm);
                assert!(
                    (-1.0..=1.0).contains(&v),
                    "noise at ({}, {}) = {} out of range",
                    i, j, v
                );
            }
        }
    }

    #[test]
    fn simplex_noise_spatial_coherence() {
        let perm = build_permutation_table(42);
        let a = simplex_noise_2d(1.0, 1.0, &perm);
        let b = simplex_noise_2d(1.01, 1.01, &perm);
        let diff = (a - b).abs();
        assert!(
            diff < 0.1,
            "nearby points should have similar noise: a={a}, b={b}, diff={diff}"
        );
    }

    #[test]
    fn different_seeds_produce_different_noise() {
        let perm_a = build_permutation_table(42);
        let perm_b = build_permutation_table(99);
        let mut same = 0;
        let total = 100;
        for i in 0..total {
            let a = simplex_noise_2d(i as f32 * 0.5, 0.0, &perm_a);
            let b = simplex_noise_2d(i as f32 * 0.5, 0.0, &perm_b);
            if (a - b).abs() < 1e-6 {
                same += 1;
            }
        }
        assert!(
            same < total / 2,
            "different seeds should produce mostly different values, got {same}/{total} same"
        );
    }
}
