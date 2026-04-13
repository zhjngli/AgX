#![doc = include_str!("grain.md")]
//!
//! See [`super::dehaze`] and [`super::denoise`] for related passes.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{smoothstep, LUMA_B, LUMA_G, LUMA_R};

/// Grain type controlling the internal character of the noise.
///
/// Each type maps to a set of internal parameters: contrast, luminance falloff,
/// chromatic intensity, and amount curve exponent.
#[cfg_attr(feature = "docgen", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrainType {
    /// Tight, low-contrast grain with minimal chromatic divergence.
    Fine,
    /// Classic medium-contrast film grain (default).
    #[default]
    Silver,
    /// Heavy, punchy grain with visible color fringing on saturated pixels.
    Harsh,
}

impl std::fmt::Display for GrainType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Fine => write!(f, "fine"),
            Self::Silver => write!(f, "silver"),
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
            "harsh" => Ok(Self::Harsh),
            _ => Err(format!(
                "invalid grain type '{s}'. Use: fine, silver, or harsh"
            )),
        }
    }
}

/// Film grain simulation parameters.
///
/// - `grain_type`: selects the grain character (default: Silver)
/// - `amount`: 0–100, intensity of grain effect (0 = no grain)
/// - `size`: 0–100, fine to coarse grain (default: 50)
/// - `seed`: optional fixed seed for deterministic grain (None = random each render)
#[cfg_attr(feature = "docgen", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrainParams {
    /// Grain character preset. Default: Silver.
    #[serde(default)]
    pub grain_type: GrainType,
    /// Grain intensity, 0–100. Default 0 (no grain).
    #[serde(default)]
    #[cfg_attr(feature = "docgen", schemars(range(min = 0.0, max = 100.0)))]
    pub amount: f32,
    /// Grain particle size, 0 (fine) to 100 (coarse). Default 50.
    #[serde(default = "default_size")]
    #[cfg_attr(feature = "docgen", schemars(range(min = 0.0, max = 100.0)))]
    pub size: f32,
    /// Optional fixed seed for deterministic grain. When None, the engine
    /// generates a random seed each render. Set in e2e test presets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

fn default_size() -> f32 {
    GRAIN_DEFAULT_SIZE
}

impl Default for GrainParams {
    fn default() -> Self {
        Self {
            grain_type: GrainType::default(),
            amount: 0.0,
            size: GRAIN_DEFAULT_SIZE,
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

/// Internal configuration for each grain type.
#[derive(Debug, Clone, Copy)]
struct GrainTypeConfig {
    /// Contrast multiplier applied to raw noise — higher = stronger grain
    contrast: f32,
    /// Base luminance falloff exponent — higher = grain concentrated in shadows.
    /// Dynamically reduced as amount increases so high-amount grain spreads
    /// across more tonal range.
    luma_falloff: f32,
    /// Chromatic grain intensity [0, 1] — per-channel noise divergence on
    /// saturated pixels. Boosted in shadows, zeroed on grayscale pixels.
    chromatic: f32,
    /// Amount curve exponent — controls how the 0-100 slider maps to intensity.
    /// Lower = grain kicks in faster at low amounts. Higher = stays subtle longer.
    amount_curve: f32,
}

impl GrainTypeConfig {
    fn from_type(grain_type: GrainType) -> Self {
        match grain_type {
            GrainType::Fine => Self {
                contrast: CONTRAST_FINE,
                luma_falloff: FALLOFF_FINE,
                chromatic: CHROMATIC_FINE,
                amount_curve: AMOUNT_CURVE_FINE,
            },
            GrainType::Silver => Self {
                contrast: CONTRAST_SILVER,
                luma_falloff: FALLOFF_SILVER,
                chromatic: CHROMATIC_SILVER,
                amount_curve: AMOUNT_CURVE_SILVER,
            },
            GrainType::Harsh => Self {
                contrast: CONTRAST_HARSH,
                luma_falloff: FALLOFF_HARSH,
                chromatic: CHROMATIC_HARSH,
                amount_curve: AMOUNT_CURVE_HARSH,
            },
        }
    }
}

// Per-type constants (extracted for tuning scripts).
const CONTRAST_FINE: f32 = 0.95;
const CONTRAST_SILVER: f32 = 1.2;
const CONTRAST_HARSH: f32 = 1.5;

const FALLOFF_FINE: f32 = 2.5;
const FALLOFF_SILVER: f32 = 1.5;
const FALLOFF_HARSH: f32 = 0.8;

const CHROMATIC_FINE: f32 = 0.05;
const CHROMATIC_SILVER: f32 = 0.10;
const CHROMATIC_HARSH: f32 = 0.15;

const AMOUNT_CURVE_FINE: f32 = 0.7;
const AMOUNT_CURVE_SILVER: f32 = 0.6;
const AMOUNT_CURVE_HARSH: f32 = 0.5;

/// Minimum supported grain amount/size value.
pub const GRAIN_PARAM_MIN: f32 = 0.0;
/// Maximum supported grain amount/size value.
pub const GRAIN_PARAM_MAX: f32 = 100.0;

/// Default grain size when not specified.
///
/// Used for schema/default drift checks and serde defaults.
pub const GRAIN_DEFAULT_SIZE: f32 = 50.0;

/// Exponent for the size-to-sigma curve. Higher values keep grain tight
/// at low sizes and only spread at high sizes.
const GRAIN_SIZE_CURVE_EXPONENT: f32 = 1.5;

/// Multiplier applied to the falloff exponent in luminance_weight.
/// Controls the steepness of the shadow-to-highlight grain gradient.
const GRAIN_LUMINANCE_WEIGHT_SCALE: f32 = 0.5;

/// Minimum blur sigma below which no blur is applied (noise used as-is).
const GRAIN_BLUR_SIGMA_THRESHOLD: f32 = 0.3;

/// Maximum blur sigma at size=100, at the reference resolution (2000px long edge).
/// Actual sigma is scaled proportionally to the image's long edge.
const GRAIN_MAX_SIGMA: f32 = 1.0;

/// Reference resolution (long edge in pixels) for grain sigma scaling.
/// Images larger than this get proportionally larger blur kernels so grain
/// particles maintain consistent visual size regardless of resolution.
const GRAIN_REF_RESOLUTION: f32 = 2000.0;

/// Strength multiplier mapping amount to noise intensity.
/// Controls the standard deviation of the exponential modulation argument.
const GRAIN_STRENGTH_MULT: f32 = 0.04;

/// Luma threshold below which grain is fully additive (bright specks on
/// dark film base). Matches real film: in deep shadows only a few silver
/// halide crystals develop, appearing as sparse bright points.
const GRAIN_ADDITIVE_END: f32 = 0.1;

/// Luma threshold above which grain is fully multiplicative (proportional
/// density modulation). Between ADDITIVE_END and MULTIPLICATIVE_START the
/// two modes are smoothly blended via smoothstep.
const GRAIN_MULTIPLICATIVE_START: f32 = 0.2;

/// Scale factor for additive grain in deep shadows, relative to the
/// multiplicative strength. Additive grain is perceptually stronger per
/// unit of noise, so we scale it down to match the midtone intensity.
const GRAIN_ADDITIVE_SCALE: f32 = 0.35;

/// How much luma_falloff decreases at amount=100. At 0.4, a base falloff
/// of 2.5 drops to 1.5 at full amount, spreading grain into midtones.
const GRAIN_FALLOFF_REDUCTION: f32 = 0.4;

/// Compute blur sigma from the size parameter (0-100), scaled to image resolution.
/// Non-linear curve: low sizes stay sharp, higher sizes spread more.
fn grain_sigma(size: f32, width: usize, height: usize) -> f32 {
    let t = (size / GRAIN_PARAM_MAX).clamp(0.0, 1.0);
    let base_sigma = t.powf(GRAIN_SIZE_CURVE_EXPONENT) * GRAIN_MAX_SIGMA;
    let long_edge = width.max(height) as f32;
    base_sigma * (long_edge / GRAIN_REF_RESOLUTION)
}

/// Generate a single-channel white noise buffer with standard normal distribution.
///
/// Each pixel gets an independent random value (mean 0, std dev 1). Uses a
/// seeded PRNG for determinism. The resulting buffer is blurred by the caller
/// to control grain size, then scaled by contrast in the apply loop.
fn generate_white_noise_buffer(width: usize, height: usize, seed: u64) -> Vec<f32> {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    let mut rng = StdRng::seed_from_u64(seed);
    let len = width * height;
    let mut buf = Vec::with_capacity(len);

    // Box-Muller transform: generate Gaussian samples from uniform pairs
    // (avoids rand_distr dependency).
    let pairs = len.div_ceil(2);
    for _ in 0..pairs {
        let u1: f32 = loop {
            let v: f32 = rand::Rng::gen(&mut rng);
            if v > 0.0 {
                break v;
            }
        };
        let u2: f32 = rand::Rng::gen(&mut rng);
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = std::f32::consts::TAU * u2;
        buf.push(r * theta.cos());
        buf.push(r * theta.sin());
    }
    buf.truncate(len);
    buf
}

/// Apply grain to an sRGB gamma buffer using white noise + blur-based sizing.
///
/// Generates four white noise buffers (1 shared + 3 per-channel) and optionally
/// blurs them to control grain size. Per-channel noise is blended with the shared
/// noise based on pixel saturation and the grain type's chromatic intensity, so
/// saturated pixels get subtle color fringing while neutral pixels stay monochrome.
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
    let amount_factor = (params.amount / GRAIN_PARAM_MAX).powf(config.amount_curve);
    let sigma = grain_sigma(params.size, width, height);

    // Generate 4 independent white noise buffers in parallel: 1 shared + 3 per-channel (R, G, B)
    let ((shared_noise, noise_r_raw), (noise_g_raw, noise_b_raw)) = rayon::join(
        || {
            rayon::join(
                || generate_white_noise_buffer(width, height, seed),
                || generate_white_noise_buffer(width, height, seed.wrapping_add(1)),
            )
        },
        || {
            rayon::join(
                || generate_white_noise_buffer(width, height, seed.wrapping_add(2)),
                || generate_white_noise_buffer(width, height, seed.wrapping_add(3)),
            )
        },
    );

    // Blur all 4 buffers in parallel with the same sigma
    let blur = |noise: Vec<f32>| -> Vec<f32> {
        if sigma >= GRAIN_BLUR_SIGMA_THRESHOLD {
            super::detail::gaussian_blur(&noise, width, height, sigma)
        } else {
            noise
        }
    };
    let ((shared, noise_r), (noise_g, noise_b)) = rayon::join(
        || rayon::join(|| blur(shared_noise), || blur(noise_r_raw)),
        || rayon::join(|| blur(noise_g_raw), || blur(noise_b_raw)),
    );

    let scale = config.contrast * GRAIN_STRENGTH_MULT * amount_factor;

    // Dynamic luma falloff: at higher amounts, reduce falloff so grain
    // spreads beyond shadows into midtones and highlights.
    let effective_falloff = config.luma_falloff * (1.0 - GRAIN_FALLOFF_REDUCTION * amount_factor);

    // Chunk size balances rayon scheduling overhead against cache locality.
    buf.par_chunks_mut(1024)
        .enumerate()
        .for_each(|(chunk_idx, chunk)| {
            let base = chunk_idx * 1024;
            for (i, pixel) in chunk.iter_mut().enumerate() {
                let idx = base + i;
                let [r, g, b] = *pixel;
                let luma = LUMA_R * r + LUMA_G * g + LUMA_B * b;

                // Pixel saturation: how colorful vs neutral this pixel is.
                let pixel_chroma = r.max(g).max(b) - r.min(g).min(b);

                // Boost chromatic divergence in shadows
                let shadow_chromatic_boost = 2.0 - luma;
                let effective_chromatic = config.chromatic * pixel_chroma * shadow_chromatic_boost;

                // Per-channel noise: blend shared (correlated) with per-channel (independent)
                let shared_part = shared[idx] * (1.0 - effective_chromatic);
                let nr = shared_part + noise_r[idx] * effective_chromatic;
                let ng = shared_part + noise_g[idx] * effective_chromatic;
                let nb = shared_part + noise_b[idx] * effective_chromatic;

                let ws = luminance_weight(luma, effective_falloff) * scale;

                let blend = smoothstep(GRAIN_ADDITIVE_END, GRAIN_MULTIPLICATIVE_START, luma);

                let apply = |val: f32, noise: f32| {
                    let nws = noise * ws;
                    let additive_delta = nws * GRAIN_ADDITIVE_SCALE;
                    let multiplicative_delta = val * (nws.exp() - 1.0);
                    let delta = additive_delta + (multiplicative_delta - additive_delta) * blend;
                    (val + delta).clamp(0.0, 1.0)
                };
                *pixel = [apply(r, nr), apply(g, ng), apply(b, nb)];
            }
        });
}

/// Compute luminance-aware weight.
///
/// Grain is most visible in shadows and fades in highlights, matching real
/// film behavior (low signal-to-noise ratio in underexposed areas). The
/// `falloff` parameter from the grain type config controls how quickly grain
/// drops off as brightness increases: higher falloff = faster drop = cleaner
/// highlights.
#[inline]
fn luminance_weight(luma: f32, falloff: f32) -> f32 {
    let l = luma.clamp(0.0, 1.0);
    (1.0 - l).powf(GRAIN_LUMINANCE_WEIGHT_SCALE * falloff)
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
    fn size_affects_spatial_frequency() {
        // Higher size should produce smoother (lower spatial frequency) grain
        // by blurring the noise buffer, reducing adjacent-pixel deltas.
        let width = 128;
        let height = 1;

        let noise = generate_white_noise_buffer(width, height, 42);

        // No blur (size=0 equivalent)
        let mut delta_raw = 0.0f32;
        for i in 0..width - 1 {
            delta_raw += (noise[i] - noise[i + 1]).abs();
        }

        // Blurred (size=100 equivalent, at reference resolution)
        let sigma = grain_sigma(100.0, 2000, 1);
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
    fn generate_white_noise_buffer_correct_length() {
        let buf = generate_white_noise_buffer(10, 8, 42);
        assert_eq!(buf.len(), 80);
    }

    #[test]
    fn generate_white_noise_buffer_has_variance() {
        let buf = generate_white_noise_buffer(64, 64, 42);
        let mean: f32 = buf.iter().sum::<f32>() / buf.len() as f32;
        let variance: f32 = buf.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / buf.len() as f32;
        assert!(
            variance > 0.001,
            "noise buffer should have meaningful variance: {variance}"
        );
    }

    #[test]
    fn generate_white_noise_buffer_deterministic() {
        let buf1 = generate_white_noise_buffer(16, 16, 42);
        let buf2 = generate_white_noise_buffer(16, 16, 42);
        assert_eq!(buf1, buf2);
    }

    #[test]
    fn apply_grain_buffer_modifies_image() {
        let params = GrainParams {
            grain_type: GrainType::Silver,
            amount: 50.0,
            size: 50.0,
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
    fn apply_grain_buffer_variance_across_sizes() {
        // Verify grain has meaningful variance at all sizes (doesn't wash out)
        for size in [0.0, 25.0, 50.0, 75.0, 100.0] {
            let params = GrainParams {
                grain_type: GrainType::Silver,
                amount: 80.0,
                size,
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
        // With additive grain in shadows, dark pixels show visible bright specks
        assert!(
            dark > 0.0,
            "shadows should have visible grain from additive mode: dark={dark}"
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
    fn chromatic_grain_shifts_color_channels_differently() {
        // Harsh has chromatic=0.15 — on saturated color pixels,
        // per-channel noise should produce different mod factors per channel.
        let params = GrainParams {
            grain_type: GrainType::Harsh,
            amount: 80.0,
            size: 25.0,
            seed: None,
        };
        let width = 64;
        let height = 64;
        // Saturated red pixels — high pixel_chroma (0.8 - 0.2 = 0.6)
        let mut buf: Vec<[f32; 3]> = vec![[0.8, 0.2, 0.2]; width * height];
        apply_grain_buffer(&mut buf, width, height, &params, 42);
        // Compare per-channel RATIOS (mod factors), not absolute deltas.
        let found_diff = buf.iter().any(|px| {
            let ratio_r = px[0] / 0.8;
            let ratio_g = px[1] / 0.2;
            (ratio_r - ratio_g).abs() > 1e-4
        });
        assert!(
            found_diff,
            "chromatic grain on color pixels should shift channels differently"
        );
    }

    #[test]
    fn chromatic_grain_no_color_shift_on_grayscale() {
        let params = GrainParams {
            grain_type: GrainType::Harsh,
            amount: 80.0,
            size: 25.0,
            seed: None,
        };
        let width = 64;
        let height = 64;
        let mut buf: Vec<[f32; 3]> = vec![[0.5, 0.5, 0.5]; width * height];
        apply_grain_buffer(&mut buf, width, height, &params, 42);
        for px in &buf {
            let dr = px[0] - 0.5;
            let dg = px[1] - 0.5;
            let db = px[2] - 0.5;
            assert!(
                (dr - dg).abs() < 1e-6 && (dg - db).abs() < 1e-6,
                "grayscale pixel should have identical per-channel grain: [{}, {}, {}]",
                px[0],
                px[1],
                px[2]
            );
        }
    }

    #[test]
    fn apply_grain_buffer_output_clamped() {
        let params = GrainParams {
            grain_type: GrainType::Harsh,
            amount: 100.0,
            size: 50.0,
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
