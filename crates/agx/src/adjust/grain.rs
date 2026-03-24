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
/// - `seed`: optional fixed seed for deterministic grain (None = random each render)
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
    /// Optional fixed seed for deterministic grain. When None, the engine
    /// generates a random seed each render. Set in e2e test presets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
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
            seed: None,
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

/// Internal configuration for each grain type.
#[derive(Debug, Clone, Copy)]
struct GrainTypeConfig {
    /// Number of octaves (2 or 3)
    octaves: u32,
    /// Relative weight of each octave (up to 3 entries)
    octave_weights: [f32; 3],
    /// Contrast multiplier applied to raw noise
    contrast: f32,
    /// Luminance falloff exponent — higher = stronger midtone bias
    luma_falloff: f32,
}

impl GrainTypeConfig {
    fn from_type(grain_type: GrainType) -> Self {
        match grain_type {
            GrainType::Fine => Self {
                octaves: 2,
                octave_weights: [0.3, 0.7, 0.0],
                contrast: 0.6,
                luma_falloff: 3.0,
            },
            GrainType::Silver => Self {
                octaves: 3,
                octave_weights: [0.5, 0.35, 0.15],
                contrast: 1.0,
                luma_falloff: 2.0,
            },
            GrainType::Soft => Self {
                octaves: 2,
                octave_weights: [0.7, 0.3, 0.0],
                contrast: 0.7,
                luma_falloff: 3.0,
            },
            GrainType::Cubic => Self {
                octaves: 3,
                octave_weights: [0.4, 0.3, 0.3],
                contrast: 1.3,
                luma_falloff: 1.5,
            },
            GrainType::Tabular => Self {
                octaves: 2,
                octave_weights: [0.6, 0.4, 0.0],
                contrast: 0.8,
                luma_falloff: 2.0,
            },
            GrainType::Harsh => Self {
                octaves: 3,
                octave_weights: [0.3, 0.3, 0.4],
                contrast: 1.5,
                luma_falloff: 1.0,
            },
        }
    }
}

/// Multi-octave simplex noise with size-dependent frequency scaling.
///
/// `size` is the user's 0–100 parameter. Lower size = higher frequency = finer grain.
/// Returns a noise value (not yet scaled by amount or luminance weight).
fn multi_octave_noise(
    x: f32,
    y: f32,
    perm: &[u8; 512],
    config: &GrainTypeConfig,
    size: f32,
) -> f32 {
    // Map size 0–100 to frequency: size=0 → freq=0.1 (very fine), size=100 → freq=0.002 (very coarse)
    let freq = 0.1 * (0.02f32).powf(size / 100.0);

    let mut value = 0.0f32;
    let mut freq_mult = 1.0f32;
    for i in 0..config.octaves {
        let weight = config.octave_weights[i as usize];
        value += weight * simplex_noise_2d(x * freq * freq_mult, y * freq * freq_mult, perm);
        freq_mult *= 2.0;
    }

    value * config.contrast
}

/// Precomputed grain state — create once per render, then call
/// [`apply_grain_pixel`] per pixel.
#[derive(Debug, Clone)]
pub struct GrainPrecomputed {
    perm: [u8; 512],
    perm_r: [u8; 512],
    perm_g: [u8; 512],
    perm_b: [u8; 512],
    config: GrainTypeConfig,
    strength: f32,
    size: f32,
    chroma_blend: f32,
    res_scale: f32,
}

impl GrainPrecomputed {
    pub fn new(params: &GrainParams, seed: u64, width: u32, height: u32) -> Self {
        let config = GrainTypeConfig::from_type(params.grain_type);
        let reference_dim = 3000.0f32;
        let dim = width.max(height) as f32;
        let res_scale = reference_dim / dim;
        Self {
            perm: build_permutation_table(seed),
            perm_r: build_permutation_table(seed.wrapping_add(1)),
            perm_g: build_permutation_table(seed.wrapping_add(2)),
            perm_b: build_permutation_table(seed.wrapping_add(3)),
            config,
            strength: params.amount / 100.0 * 0.15,
            size: params.size,
            chroma_blend: params.chromatic / 100.0,
            res_scale,
        }
    }
}

/// Compute luminance-aware weight.
#[inline]
fn luminance_weight(luma: f32, falloff: f32) -> f32 {
    let base = (4.0 * luma.clamp(0.0, 1.0) * (1.0 - luma.clamp(0.0, 1.0))).clamp(0.0, 1.0);
    base.powf(falloff)
}

/// Apply grain to a single pixel. Output is clamped to [0.0, 1.0].
pub fn apply_grain_pixel(
    r: f32,
    g: f32,
    b: f32,
    x: u32,
    y: u32,
    pre: &GrainPrecomputed,
) -> (f32, f32, f32) {
    if pre.strength == 0.0 {
        return (r, g, b);
    }

    let xf = x as f32 * pre.res_scale;
    let yf = y as f32 * pre.res_scale;

    let luma = LUMA_R * r + LUMA_G * g + LUMA_B * b;
    let luma_w = luminance_weight(luma, pre.config.luma_falloff);
    let scale = pre.strength * luma_w;

    if pre.chroma_blend == 0.0 {
        let noise = multi_octave_noise(xf, yf, &pre.perm, &pre.config, pre.size);
        let shift = noise * scale;
        (
            (r + shift).clamp(0.0, 1.0),
            (g + shift).clamp(0.0, 1.0),
            (b + shift).clamp(0.0, 1.0),
        )
    } else {
        let shared = multi_octave_noise(xf, yf, &pre.perm, &pre.config, pre.size);
        let nr = multi_octave_noise(xf, yf, &pre.perm_r, &pre.config, pre.size);
        let ng = multi_octave_noise(xf, yf, &pre.perm_g, &pre.config, pre.size);
        let nb = multi_octave_noise(xf, yf, &pre.perm_b, &pre.config, pre.size);

        let blend = pre.chroma_blend;
        let shift_r = (shared * (1.0 - blend) + nr * blend) * scale;
        let shift_g = (shared * (1.0 - blend) + ng * blend) * scale;
        let shift_b = (shared * (1.0 - blend) + nb * blend) * scale;

        (
            (r + shift_r).clamp(0.0, 1.0),
            (g + shift_g).clamp(0.0, 1.0),
            (b + shift_b).clamp(0.0, 1.0),
        )
    }
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

    #[test]
    fn grain_types_produce_different_output() {
        let types = [
            GrainType::Fine, GrainType::Silver, GrainType::Soft,
            GrainType::Cubic, GrainType::Tabular, GrainType::Harsh,
        ];
        let perm = build_permutation_table(42);
        let mut variances = Vec::new();
        for gt in &types {
            let config = GrainTypeConfig::from_type(*gt);
            let mut sum_sq = 0.0;
            let n = 400;
            for i in 0..20 {
                for j in 0..20 {
                    let v = multi_octave_noise(
                        i as f32 * 0.1, j as f32 * 0.1, &perm, &config, 50.0,
                    );
                    sum_sq += v * v;
                }
            }
            variances.push(sum_sq / n as f32);
        }
        let min = variances.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = variances.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            max > min * 1.1,
            "grain types should produce different variances: {variances:?}"
        );
    }

    #[test]
    fn size_affects_frequency() {
        let perm = build_permutation_table(42);
        let config = GrainTypeConfig::from_type(GrainType::Silver);
        let mut delta_small = 0.0f32;
        for i in 0..99 {
            let a = multi_octave_noise(i as f32, 0.0, &perm, &config, 10.0);
            let b = multi_octave_noise((i + 1) as f32, 0.0, &perm, &config, 10.0);
            delta_small += (a - b).abs();
        }
        let mut delta_large = 0.0f32;
        for i in 0..99 {
            let a = multi_octave_noise(i as f32, 0.0, &perm, &config, 90.0);
            let b = multi_octave_noise((i + 1) as f32, 0.0, &perm, &config, 90.0);
            delta_large += (a - b).abs();
        }
        assert!(
            delta_small > delta_large,
            "fine grain should vary more between adjacent pixels: small={delta_small}, large={delta_large}"
        );
    }

    #[test]
    fn apply_grain_pixel_identity_when_amount_zero() {
        let params = GrainParams::default();
        let pre = GrainPrecomputed::new(&params, 42, 100, 100);
        let (r, g, b) = apply_grain_pixel(0.5, 0.3, 0.1, 10, 10, &pre);
        assert_eq!(r, 0.5);
        assert_eq!(g, 0.3);
        assert_eq!(b, 0.1);
    }

    #[test]
    fn apply_grain_pixel_modifies_output() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 50.0,
            size: 50.0,
            chromatic: 0.0,
            seed: None,
        };
        let pre = GrainPrecomputed::new(&params, 42, 100, 100);
        let (r, g, b) = apply_grain_pixel(0.5, 0.5, 0.5, 10, 10, &pre);
        let changed = (r - 0.5).abs() > 1e-6
            || (g - 0.5).abs() > 1e-6
            || (b - 0.5).abs() > 1e-6;
        assert!(changed, "grain should modify pixel values: got ({r}, {g}, {b})");
    }

    #[test]
    fn apply_grain_pixel_luminance_only_shifts_channels_equally() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 50.0,
            size: 50.0,
            chromatic: 0.0,
            seed: None,
        };
        let pre = GrainPrecomputed::new(&params, 42, 100, 100);
        let (r, g, b) = apply_grain_pixel(0.5, 0.5, 0.5, 10, 10, &pre);
        let dr = r - 0.5;
        let dg = g - 0.5;
        let db = b - 0.5;
        assert!(
            (dr - dg).abs() < 1e-6 && (dg - db).abs() < 1e-6,
            "luminance-only grain should shift all channels equally: dr={dr}, dg={dg}, db={db}"
        );
    }

    #[test]
    fn apply_grain_pixel_chromatic_shifts_channels_differently() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 50.0,
            size: 50.0,
            chromatic: 100.0,
            seed: None,
        };
        let pre = GrainPrecomputed::new(&params, 42, 100, 100);
        let mut found_diff = false;
        for x in 0..20u32 {
            for y in 0..20u32 {
                let (r, g, b) = apply_grain_pixel(0.5, 0.5, 0.5, x, y, &pre);
                let dr = r - 0.5;
                let dg = g - 0.5;
                let db = b - 0.5;
                if (dr - dg).abs() > 1e-4 || (dg - db).abs() > 1e-4 {
                    found_diff = true;
                    break;
                }
            }
            if found_diff { break; }
        }
        assert!(found_diff, "chromatic grain should produce different per-channel shifts");
    }

    #[test]
    fn apply_grain_pixel_luminance_aware_falloff() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 80.0,
            size: 50.0,
            chromatic: 0.0,
            seed: None,
        };
        let pre = GrainPrecomputed::new(&params, 42, 200, 200);
        let measure = |lum: f32| -> f32 {
            let mut total = 0.0f32;
            let n = 100;
            for x in 0..n {
                let (r, _g, _b) = apply_grain_pixel(lum, lum, lum, x, 50, &pre);
                total += (r - lum).abs();
            }
            total / n as f32
        };
        let mid = measure(0.5);
        let dark = measure(0.02);
        let bright = measure(0.98);
        assert!(
            mid > dark,
            "midtones should have more grain than shadows: mid={mid}, dark={dark}"
        );
        assert!(
            mid > bright,
            "midtones should have more grain than highlights: mid={mid}, bright={bright}"
        );
    }

    #[test]
    fn apply_grain_pixel_resolution_independent() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 50.0,
            size: 50.0,
            chromatic: 0.0,
            seed: None,
        };
        let pre_small = GrainPrecomputed::new(&params, 42, 1000, 1000);
        let pre_large = GrainPrecomputed::new(&params, 42, 3000, 3000);
        let (r1, _, _) = apply_grain_pixel(0.5, 0.5, 0.5, 500, 500, &pre_small);
        let (r2, _, _) = apply_grain_pixel(0.5, 0.5, 0.5, 1500, 1500, &pre_large);
        let shift1 = (r1 - 0.5).abs();
        let shift2 = (r2 - 0.5).abs();
        assert!(
            shift1 > 0.0 && shift2 > 0.0,
            "both images should have grain: shift1={shift1}, shift2={shift2}"
        );
    }

    #[test]
    fn apply_grain_pixel_output_clamped() {
        let params = GrainParams {
            grain_type: GrainType::Harsh,
            amount: 100.0,
            size: 50.0,
            chromatic: 100.0,
            seed: None,
        };
        let pre = GrainPrecomputed::new(&params, 42, 100, 100);
        for x in 0..50u32 {
            for y in 0..50u32 {
                let (r, g, b) = apply_grain_pixel(0.01, 0.01, 0.01, x, y, &pre);
                assert!((0.0..=1.0).contains(&r) && (0.0..=1.0).contains(&g) && (0.0..=1.0).contains(&b),
                    "output must be clamped: ({r}, {g}, {b})");
                let (r, g, b) = apply_grain_pixel(0.99, 0.99, 0.99, x, y, &pre);
                assert!((0.0..=1.0).contains(&r) && (0.0..=1.0).contains(&g) && (0.0..=1.0).contains(&b),
                    "output must be clamped: ({r}, {g}, {b})");
            }
        }
    }
}
