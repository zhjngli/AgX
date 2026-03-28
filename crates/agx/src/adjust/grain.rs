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

impl std::fmt::Display for GrainType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fine => write!(f, "fine"),
            Self::Silver => write!(f, "silver"),
            Self::Soft => write!(f, "soft"),
            Self::Cubic => write!(f, "cubic"),
            Self::Tabular => write!(f, "tabular"),
            Self::Harsh => write!(f, "harsh"),
        }
    }
}

impl std::str::FromStr for GrainType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "fine" => Ok(Self::Fine),
            "silver" => Ok(Self::Silver),
            "soft" => Ok(Self::Soft),
            "cubic" => Ok(Self::Cubic),
            "tabular" => Ok(Self::Tabular),
            "harsh" => Ok(Self::Harsh),
            _ => Err(format!(
                "invalid grain type '{s}'. Use: fine, silver, soft, cubic, tabular, or harsh"
            )),
        }
    }
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
    [1.0, 1.0],
    [-1.0, 1.0],
    [1.0, -1.0],
    [-1.0, -1.0],
    [1.0, 0.0],
    [-1.0, 0.0],
    [0.0, 1.0],
    [0.0, -1.0],
    [1.0, 1.0],
    [-1.0, 1.0],
    [1.0, -1.0],
    [-1.0, -1.0],
];

/// Skewing factor for 2D simplex grid: (sqrt(3) - 1) / 2.
const F2: f32 = 0.366_025_4;
/// Unskewing factor: (3 - sqrt(3)) / 6.
const G2: f32 = 0.211_324_87;

/// Build a seeded 256-entry permutation table for simplex noise.
fn build_permutation_table(seed: u64) -> [u8; 512] {
    let mut perm = [0u8; 256];
    for (i, val) in perm.iter_mut().enumerate() {
        *val = i as u8;
    }
    // Fisher-Yates shuffle with simple LCG seeded PRNG
    let mut rng = seed;
    for i in (1..256).rev() {
        rng = rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
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

/// Multi-octave simplex noise with precomputed per-octave frequencies.
///
/// Returns a noise value (not yet scaled by amount or luminance weight).
fn multi_octave_noise(
    x: f32,
    y: f32,
    perm: &[u8; 512],
    config: &GrainTypeConfig,
    octave_freqs: &[f32; 3],
) -> f32 {
    let mut value = 0.0f32;
    for i in 0..config.octaves {
        let freq = octave_freqs[i as usize];
        let weight = config.octave_weights[i as usize];
        value += weight * simplex_noise_2d(x * freq, y * freq, perm);
    }

    value * config.contrast
}

/// Fixed base frequency for noise generation. Size is controlled by post-blur, not frequency.
const GRAIN_BASE_FREQ: f32 = 0.08;

/// Minimum blur sigma below which the per-pixel fast path is used.
const GRAIN_BLUR_SIGMA_THRESHOLD: f32 = 0.5;

/// Maximum blur sigma at size=100.
const GRAIN_MAX_SIGMA: f32 = 2.5;

/// Compute blur sigma from the size parameter (0-100).
/// Non-linear curve: low sizes stay sharp, higher sizes spread more.
fn grain_sigma(size: f32) -> f32 {
    let t = (size / 100.0).clamp(0.0, 1.0);
    t.powf(1.5) * GRAIN_MAX_SIGMA
}

/// Generate a single-channel noise buffer at fixed high frequency.
///
/// Each pixel gets multi-octave simplex noise scaled by `res_scale`
/// (for resolution independence) and the grain type's contrast.
fn generate_noise_buffer(
    width: usize,
    height: usize,
    perm: &[u8; 512],
    config: &GrainTypeConfig,
    res_scale: f32,
) -> Vec<f32> {
    let octave_freqs = [
        GRAIN_BASE_FREQ,
        GRAIN_BASE_FREQ * 2.0,
        GRAIN_BASE_FREQ * 4.0,
    ];
    let mut buf = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let xf = x as f32 * res_scale;
            let yf = y as f32 * res_scale;
            let noise = multi_octave_noise(xf, yf, perm, config, &octave_freqs);
            buf.push(noise);
        }
    }
    buf
}

/// Apply grain to an sRGB gamma buffer using blur-based sizing.
///
/// When sigma is below threshold, uses per-pixel noise (no buffer allocation).
/// When sigma >= threshold, generates noise into a buffer, blurs it, then
/// reads per-pixel from the blurred noise.
///
/// Mutates `buf` in-place. Each pixel is `[r, g, b]` in sRGB gamma [0.0, 1.0].
pub fn apply_grain_buffer(
    buf: &mut [[f32; 3]],
    width: usize,
    height: usize,
    params: &GrainParams,
    seed: u64,
) {
    if params.amount == 0.0 {
        return;
    }

    let config = GrainTypeConfig::from_type(params.grain_type);
    let reference_dim = 3000.0f32;
    let dim = width.max(height) as f32;
    let res_scale = reference_dim / dim;
    let strength = params.amount / 100.0 * 0.15;
    let chroma_blend = params.chromatic / 100.0;
    let sigma = grain_sigma(params.size);

    let perm = build_permutation_table(seed);

    if sigma < GRAIN_BLUR_SIGMA_THRESHOLD {
        // Fast path: per-pixel noise, no buffer allocation
        let octave_freqs = [
            GRAIN_BASE_FREQ,
            GRAIN_BASE_FREQ * 2.0,
            GRAIN_BASE_FREQ * 4.0,
        ];
        let perm_r = build_permutation_table(seed.wrapping_add(1));
        let perm_g = build_permutation_table(seed.wrapping_add(2));
        let perm_b = build_permutation_table(seed.wrapping_add(3));

        for y in 0..height {
            for x in 0..width {
                let xf = x as f32 * res_scale;
                let yf = y as f32 * res_scale;
                let idx = y * width + x;
                let [r, g, b] = buf[idx];
                let luma = LUMA_R * r + LUMA_G * g + LUMA_B * b;
                let luma_w = luminance_weight(luma, config.luma_falloff);
                let scale = strength * luma_w;

                if chroma_blend == 0.0 {
                    let noise = multi_octave_noise(xf, yf, &perm, &config, &octave_freqs);
                    let shift = noise * scale;
                    buf[idx] = [
                        (r + shift).clamp(0.0, 1.0),
                        (g + shift).clamp(0.0, 1.0),
                        (b + shift).clamp(0.0, 1.0),
                    ];
                } else {
                    let shared = multi_octave_noise(xf, yf, &perm, &config, &octave_freqs);
                    let nr = multi_octave_noise(xf, yf, &perm_r, &config, &octave_freqs);
                    let ng = multi_octave_noise(xf, yf, &perm_g, &config, &octave_freqs);
                    let nb = multi_octave_noise(xf, yf, &perm_b, &config, &octave_freqs);
                    let blend = chroma_blend;
                    buf[idx] = [
                        (r + (shared * (1.0 - blend) + nr * blend) * scale).clamp(0.0, 1.0),
                        (g + (shared * (1.0 - blend) + ng * blend) * scale).clamp(0.0, 1.0),
                        (b + (shared * (1.0 - blend) + nb * blend) * scale).clamp(0.0, 1.0),
                    ];
                }
            }
        }
    } else {
        // Buffer path: generate noise, blur, apply
        let shared_noise = generate_noise_buffer(width, height, &perm, &config, res_scale);
        let shared_blurred = super::detail::gaussian_blur(&shared_noise, width, height, sigma);

        if chroma_blend == 0.0 {
            for y in 0..height {
                for x in 0..width {
                    let idx = y * width + x;
                    let [r, g, b] = buf[idx];
                    let luma = LUMA_R * r + LUMA_G * g + LUMA_B * b;
                    let luma_w = luminance_weight(luma, config.luma_falloff);
                    let scale = strength * luma_w;
                    let shift = shared_blurred[idx] * scale;
                    buf[idx] = [
                        (r + shift).clamp(0.0, 1.0),
                        (g + shift).clamp(0.0, 1.0),
                        (b + shift).clamp(0.0, 1.0),
                    ];
                }
            }
        } else {
            let perm_r = build_permutation_table(seed.wrapping_add(1));
            let perm_g = build_permutation_table(seed.wrapping_add(2));
            let perm_b = build_permutation_table(seed.wrapping_add(3));
            let noise_r = generate_noise_buffer(width, height, &perm_r, &config, res_scale);
            let blurred_r = super::detail::gaussian_blur(&noise_r, width, height, sigma);
            let noise_g = generate_noise_buffer(width, height, &perm_g, &config, res_scale);
            let blurred_g = super::detail::gaussian_blur(&noise_g, width, height, sigma);
            let noise_b = generate_noise_buffer(width, height, &perm_b, &config, res_scale);
            let blurred_b = super::detail::gaussian_blur(&noise_b, width, height, sigma);

            for y in 0..height {
                for x in 0..width {
                    let idx = y * width + x;
                    let [r, g, b] = buf[idx];
                    let luma = LUMA_R * r + LUMA_G * g + LUMA_B * b;
                    let luma_w = luminance_weight(luma, config.luma_falloff);
                    let scale = strength * luma_w;
                    let blend = chroma_blend;
                    buf[idx] = [
                        (r + (shared_blurred[idx] * (1.0 - blend) + blurred_r[idx] * blend)
                            * scale)
                            .clamp(0.0, 1.0),
                        (g + (shared_blurred[idx] * (1.0 - blend) + blurred_g[idx] * blend)
                            * scale)
                            .clamp(0.0, 1.0),
                        (b + (shared_blurred[idx] * (1.0 - blend) + blurred_b[idx] * blend)
                            * scale)
                            .clamp(0.0, 1.0),
                    ];
                }
            }
        }
    }
}

/// Compute luminance-aware weight.
#[inline]
fn luminance_weight(luma: f32, falloff: f32) -> f32 {
    let base = (4.0 * luma.clamp(0.0, 1.0) * (1.0 - luma.clamp(0.0, 1.0))).clamp(0.0, 1.0);
    base.powf(falloff)
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
                    i,
                    j,
                    v
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

    /// Helper: compute octave_freqs at fixed base frequency (for tests).
    fn fixed_octave_freqs() -> [f32; 3] {
        [
            GRAIN_BASE_FREQ,
            GRAIN_BASE_FREQ * 2.0,
            GRAIN_BASE_FREQ * 4.0,
        ]
    }

    #[test]
    fn grain_types_produce_different_output() {
        let types = [
            GrainType::Fine,
            GrainType::Silver,
            GrainType::Soft,
            GrainType::Cubic,
            GrainType::Tabular,
            GrainType::Harsh,
        ];
        let perm = build_permutation_table(42);
        let freqs = fixed_octave_freqs();
        let mut variances = Vec::new();
        for gt in &types {
            let config = GrainTypeConfig::from_type(*gt);
            let mut sum_sq = 0.0;
            let n = 400;
            for i in 0..20 {
                for j in 0..20 {
                    let v =
                        multi_octave_noise(i as f32 * 0.1, j as f32 * 0.1, &perm, &config, &freqs);
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
    fn size_affects_spatial_frequency() {
        // Higher size should produce smoother (lower spatial frequency) grain
        // by blurring the noise buffer, reducing adjacent-pixel deltas.
        let perm = build_permutation_table(42);
        let config = GrainTypeConfig::from_type(GrainType::Silver);
        let width = 128;
        let height = 1;

        let noise = generate_noise_buffer(width, height, &perm, &config, 1.0);

        // No blur (size=0 equivalent)
        let mut delta_raw = 0.0f32;
        for i in 0..width - 1 {
            delta_raw += (noise[i] - noise[i + 1]).abs();
        }

        // Blurred (size=100 equivalent)
        let sigma = grain_sigma(100.0);
        let blurred = super::super::detail::gaussian_blur(&noise, width, height, sigma);
        let mut delta_blurred = 0.0f32;
        for i in 0..width - 1 {
            delta_blurred += (blurred[i] - blurred[i + 1]).abs();
        }

        assert!(
            delta_raw > delta_blurred,
            "blurred grain should have lower spatial frequency: raw={delta_raw}, blurred={delta_blurred}"
        );
    }

    #[test]
    fn can_access_detail_gaussian_blur() {
        // Verify we can call the shared gaussian_blur from grain module.
        let input = vec![1.0f32; 9]; // 3x3 uniform
        let output = super::super::detail::gaussian_blur(&input, 3, 3, 1.0);
        assert_eq!(output.len(), 9);
        // Uniform input blurred should stay uniform (within float tolerance)
        for v in &output {
            assert!(
                (v - 1.0).abs() < 1e-5,
                "uniform blur should be identity: got {v}"
            );
        }
    }

    #[test]
    fn generate_noise_buffer_correct_length() {
        let perm = build_permutation_table(42);
        let config = GrainTypeConfig::from_type(GrainType::Silver);
        let buf = generate_noise_buffer(10, 8, &perm, &config, 1.0);
        assert_eq!(buf.len(), 80);
    }

    #[test]
    fn generate_noise_buffer_has_variance() {
        let perm = build_permutation_table(42);
        let config = GrainTypeConfig::from_type(GrainType::Silver);
        let buf = generate_noise_buffer(64, 64, &perm, &config, 1.0);
        let mean: f32 = buf.iter().sum::<f32>() / buf.len() as f32;
        let variance: f32 = buf.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / buf.len() as f32;
        assert!(
            variance > 0.001,
            "noise buffer should have meaningful variance: {variance}"
        );
    }

    #[test]
    fn generate_noise_buffer_deterministic() {
        let perm = build_permutation_table(42);
        let config = GrainTypeConfig::from_type(GrainType::Silver);
        let buf1 = generate_noise_buffer(16, 16, &perm, &config, 1.0);
        let buf2 = generate_noise_buffer(16, 16, &perm, &config, 1.0);
        assert_eq!(buf1, buf2);
    }

    #[test]
    fn apply_grain_buffer_modifies_image() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 50.0,
            size: 50.0,
            chromatic: 0.0,
            seed: None,
        };
        let width = 64;
        let height = 64;
        let mut buf: Vec<[f32; 3]> = vec![[0.5, 0.5, 0.5]; width * height];
        apply_grain_buffer(&mut buf, width, height, &params, 42);
        let changed = buf.iter().any(|px| (px[0] - 0.5).abs() > 1e-6);
        assert!(changed, "grain buffer should modify image pixels");
    }

    #[test]
    fn apply_grain_buffer_zero_amount_is_identity() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 0.0,
            size: 50.0,
            chromatic: 0.0,
            seed: None,
        };
        let width = 16;
        let height = 16;
        let mut buf: Vec<[f32; 3]> = vec![[0.5, 0.3, 0.1]; width * height];
        let original = buf.clone();
        apply_grain_buffer(&mut buf, width, height, &params, 42);
        assert_eq!(buf, original);
    }

    #[test]
    fn apply_grain_buffer_chromatic_shifts_channels_differently() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 50.0,
            size: 50.0,
            chromatic: 100.0,
            seed: None,
        };
        let width = 32;
        let height = 32;
        let mut buf: Vec<[f32; 3]> = vec![[0.5, 0.5, 0.5]; width * height];
        apply_grain_buffer(&mut buf, width, height, &params, 42);
        let found_diff = buf.iter().any(|px| {
            let dr = px[0] - 0.5;
            let dg = px[1] - 0.5;
            let db = px[2] - 0.5;
            (dr - dg).abs() > 1e-4 || (dg - db).abs() > 1e-4
        });
        assert!(
            found_diff,
            "chromatic grain should produce different per-channel shifts"
        );
    }

    #[test]
    fn apply_grain_buffer_variance_across_sizes() {
        // Verify grain has meaningful variance at all sizes (doesn't wash out)
        for size in [0.0, 25.0, 50.0, 75.0, 100.0] {
            let params = GrainParams {
                grain_type: GrainType::Silver,
                amount: 80.0,
                size,
                chromatic: 0.0,
                seed: None,
            };
            let width = 64;
            let height = 64;
            let mut buf: Vec<[f32; 3]> = vec![[0.5, 0.5, 0.5]; width * height];
            apply_grain_buffer(&mut buf, width, height, &params, 42);
            let deltas: Vec<f32> = buf.iter().map(|px| (px[0] - 0.5).abs()).collect();
            let mean_delta: f32 = deltas.iter().sum::<f32>() / deltas.len() as f32;
            assert!(
                mean_delta > 0.001,
                "grain at size={size} should have visible effect: mean_delta={mean_delta}"
            );
        }
    }

    #[test]
    fn apply_grain_buffer_luminance_aware_falloff() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 80.0,
            size: 50.0,
            chromatic: 0.0,
            seed: None,
        };
        let width = 128;
        let height = 1;
        let measure = |lum: f32| -> f32 {
            let mut buf: Vec<[f32; 3]> = vec![[lum, lum, lum]; width * height];
            apply_grain_buffer(&mut buf, width, height, &params, 42);
            buf.iter().map(|px| (px[0] - lum).abs()).sum::<f32>() / buf.len() as f32
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
    fn apply_grain_buffer_resolution_independent() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 50.0,
            size: 50.0,
            chromatic: 0.0,
            seed: None,
        };
        // Both images should produce non-zero grain
        let mut buf_small: Vec<[f32; 3]> = vec![[0.5, 0.5, 0.5]; 32 * 32];
        apply_grain_buffer(&mut buf_small, 32, 32, &params, 42);
        let shift_small: f32 =
            buf_small.iter().map(|px| (px[0] - 0.5).abs()).sum::<f32>() / buf_small.len() as f32;

        let mut buf_large: Vec<[f32; 3]> = vec![[0.5, 0.5, 0.5]; 128 * 128];
        apply_grain_buffer(&mut buf_large, 128, 128, &params, 42);
        let shift_large: f32 =
            buf_large.iter().map(|px| (px[0] - 0.5).abs()).sum::<f32>() / buf_large.len() as f32;

        assert!(
            shift_small > 0.0 && shift_large > 0.0,
            "both resolutions should have grain: small={shift_small}, large={shift_large}"
        );
    }

    #[test]
    fn apply_grain_buffer_output_clamped() {
        let params = GrainParams {
            grain_type: GrainType::Harsh,
            amount: 100.0,
            size: 50.0,
            chromatic: 100.0,
            seed: None,
        };
        let width = 32;
        let height = 32;
        let mut buf: Vec<[f32; 3]> = vec![[0.01, 0.01, 0.01]; width * height];
        apply_grain_buffer(&mut buf, width, height, &params, 42);
        for px in &buf {
            assert!(
                (0.0..=1.0).contains(&px[0])
                    && (0.0..=1.0).contains(&px[1])
                    && (0.0..=1.0).contains(&px[2]),
                "output must be clamped: ({}, {}, {})",
                px[0],
                px[1],
                px[2]
            );
        }
    }
}
