# Noise Reduction Design

**Date:** 2026-03-22
**Status:** Approved

## Goal

Add luminance and chroma noise reduction to AgX using à trous wavelet decomposition with soft thresholding. Three user-facing parameters: luminance strength, color strength, and detail preservation. Operates in linear space as a buffer-level pass, sharing the existing linear buffer with dehaze.

## Background

Sensor noise degrades image quality, especially in high-ISO and low-light photos. Noise has two perceptually distinct components: luminance noise (grain) and chroma noise (color blotches). Chroma noise is more objectionable, so stronger smoothing is typically applied to color channels. A detail preservation control protects fine texture from being smoothed away.

### Algorithm References

- **À trous wavelet transform:** Starck, Murtagh & Bijaoui, "Image Processing and Data Analysis" (1998). A stationary (non-decimated) wavelet transform that avoids downsampling artifacts by inserting gaps ("holes") between filter taps at each decomposition level.
- **Soft thresholding:** Donoho & Johnstone, "Ideal Spatial Adaptation by Wavelet Shrinkage" (1994). Shrinks wavelet coefficients toward zero smoothly: `sign(x) * max(|x| - threshold, 0)`. Produces smoother results than hard thresholding.
- **Noise estimation via MAD:** Donoho & Johnstone (1994). The median absolute deviation of the finest wavelet level divided by 0.6745 gives a robust estimate of Gaussian noise standard deviation.

## Data Model

### NoiseReductionParams

```rust
pub struct NoiseReductionParams {
    #[serde(default)]
    pub luminance: f32,  // 0–100, strength of luma denoising
    #[serde(default)]
    pub color: f32,      // 0–100, strength of chroma denoising
    #[serde(default)]
    pub detail: f32,     // 0–100, finest-scale protection (higher = more detail kept)
}
```

- Default: all `0.0`
- `is_neutral()`: returns true when all three are zero
- When neutral, the entire noise reduction pass is skipped with zero overhead

### PartialNoiseReductionParams

```rust
pub struct PartialNoiseReductionParams {
    pub luminance: Option<f32>,
    pub color: Option<f32>,
    pub detail: Option<f32>,
}
```

Follows the same merge/materialize/From pattern as PartialDehazeParams and PartialDetailParams.

### Preset TOML

```toml
[noise_reduction]
luminance = 40.0
color = 25.0
detail = 50.0
```

### CLI Flags

```
--nr-luminance <value>   Luminance noise reduction strength (0–100)
--nr-color <value>       Color noise reduction strength (0–100)
--nr-detail <value>      Detail preservation (0–100)
```

### Validation

All three parameters must be in range 0–100. Values outside this range produce a preset validation error.

## Algorithm

### Step 1: Luma/Chroma Separation (YCbCr)

Split the linear RGB buffer into luminance and chrominance channels:

```
Y  = LUMA_R * R + LUMA_G * G + LUMA_B * B
Cb = B - Y
Cr = R - Y
```

Uses the shared Rec. 709 `LUMA_R/G/B` constants from `adjust/mod.rs`.

### Step 2: À Trous Wavelet Decomposition

Decompose each channel (Y, Cb, Cr) independently into 5 detail levels + 1 residual.

**Kernel:** B3-spline `[1/16, 1/4, 3/8, 1/4, 1/16]`

Levels are indexed 0–4. At each level `k` (k = 0, 1, 2, 3, 4):
1. Convolve the current approximation with the B3-spline kernel, using tap spacing of `2^k` pixels (the "à trous" gaps). At level 0, tap spacing is 1 (adjacent pixels). At level 4, tap spacing is 16.
2. The convolution is separable: horizontal pass then vertical pass
3. Detail at level `k` = previous approximation − current approximation
4. The current approximation becomes the input for the next level

After 5 levels, the remaining approximation is the residual (coarse structure).

**Boundary handling:** Mirror (reflect) at image edges. This avoids edge artifacts and is the standard choice for wavelet denoising.

**Complexity:** O(n) per level. Total: O(5n) ≈ O(n) since the kernel size is fixed (5 taps) and the image stays at full resolution throughout.

### Step 3: Noise Estimation

Estimate noise standard deviation from the finest wavelet detail level (level 0) using the median absolute deviation (MAD):

```
sigma = median(|detail_level_0|) / 0.6745
```

This is computed once per channel and used to scale thresholds at all levels.

**Limitation:** A single global sigma is a simplification. In linear space, sensor noise variance scales with signal level (photon shot noise), so a global estimate may underestimate noise in shadows and overestimate in highlights. Per-pixel adaptive thresholding could address this in a future enhancement.

### Step 4: Soft Thresholding

For each wavelet detail level and each channel, apply soft thresholding:

```
threshold = sigma * k_level * strength_factor
output = sign(x) * max(|x| - threshold, 0)
```

Where:
- `strength_factor` maps the user parameter (0–100) to a multiplier range: `param / 100.0 * 3.0`, giving a 0–3 range. For the Y channel this uses `luminance`; for Cb/Cr it uses `color`. These initial values may be tuned based on visual results.
- `k_level` is a per-level scale factor that increases with level (coarser levels contain less noise, so they need proportionally higher thresholds to avoid over-smoothing structure). Initial schedule: `[1.0, 1.0, 1.2, 1.5, 2.0]` for levels 0–4.
- **Detail preservation:** The `detail` parameter (0–100) reduces the threshold at level 0 (finest scale). The level-0 threshold is multiplied by `1.0 - detail / 100.0`. At detail=100, level 0 is untouched. At detail=0, level 0 gets full thresholding. This protects fine texture and edges.

### Step 5: Reconstruction

Sum all (thresholded) detail levels + residual to reconstruct each channel:

```
channel = residual + detail_1 + detail_2 + detail_3 + detail_4 + detail_5
```

### Step 6: Convert Back to RGB

```
R = Y + Cr
B = Y + Cb
G = (Y - LUMA_R * R - LUMA_B * B) / LUMA_G
```

Clamp all channels to [0, 1].

## Pipeline Integration

### Render Pipeline Position

Step 2c, after dehaze, before sRGB gamma conversion:

```
1. WB
2. Exposure
   2b. Dehaze (linear space, buffer-level, when active)
   2c. Noise reduction (linear space, buffer-level, when active)
3. Convert to sRGB gamma space
4–8. Tone/color adjustments
9. Detail pass (sharpening, clarity, texture)
10. Convert back to linear space
```

### Buffer Sharing with Dehaze

The existing linear buffer logic in `engine/mod.rs` generalizes:

- Build WB+exposure linear buffer when `dehaze_active || nr_active`
- If dehaze active → apply dehaze to that buffer
- If NR active → apply noise reduction, taking `&[[f32; 3]]` and returning a new `Vec<[f32; 3]>` (same pattern as `apply_dehaze`)
- `get_linear` reads from the buffer (no changes to the closure)

The condition for building the buffer changes from `dehaze_active` to `dehaze_active || nr_active`.

### Memory

The à trous decomposition requires storing 5 detail levels + 1 residual per channel. For a 24MP image, each single-channel buffer is ~92 MB. To limit peak memory, channels are processed sequentially (Y first, then Cb, then Cr) — only one channel's wavelet stack is in memory at a time. This requires 6 full-resolution single-channel buffers (~550 MB for 24MP) plus the 3-channel input/output buffers. The wavelet buffers are freed after reconstruction of each channel.

### Zero Overhead When Neutral

`is_neutral()` returns true when all three parameters are zero. The engine skips the entire NR pass — no buffer allocation, no decomposition.

## File Structure

### New File

- `crates/agx/src/adjust/denoise.rs` — `NoiseReductionParams`, à trous wavelet decomposition, soft thresholding, noise estimation, YCbCr conversion, `apply_noise_reduction`, unit tests

### Modified Files

- `crates/agx/src/adjust/mod.rs` — `pub mod denoise; pub use denoise::NoiseReductionParams;`
- `crates/agx/src/engine/mod.rs` — `PartialNoiseReductionParams`, add `noise_reduction` to `Parameters`/`PartialParameters`, update render buffer logic
- `crates/agx/src/preset/mod.rs` — `[noise_reduction]` section parsing, validation, serialization
- `crates/agx/src/lib.rs` — re-export `NoiseReductionParams`, `PartialNoiseReductionParams`
- `crates/agx-cli/src/main.rs` — `--nr-luminance`, `--nr-color`, `--nr-detail` flags
- `ARCHITECTURE.md` — design doc link

### E2E

- `crates/agx-e2e/fixtures/looks/nr_landscape.toml` — moderate NR (luminance=30, color=20, detail=50)
- `crates/agx-e2e/fixtures/looks/nr_heavy.toml` — aggressive NR (luminance=80, color=60, detail=30)
- Add both to `ALL_LOOKS` in `cli_pipeline.rs`
- Generate 10 new golden files (5 color images × 2 presets; B&W image uses BW_LOOKS only)

## Testing Strategy

### Unit Tests (denoise.rs)

- `default_params_are_neutral` / `non_zero_is_not_neutral`
- `atrous_decompose_and_reconstruct_is_identity` — verify lossless round-trip within float tolerance
- `soft_threshold_zero_threshold_is_identity`
- `soft_threshold_removes_small_coefficients`
- `estimate_noise_sigma_on_known_noise` — verify estimated sigma matches known input
- `apply_nr_zero_amount_is_identity`
- `apply_nr_luminance_reduces_luma_variation` — noisy gradient, verify variance reduction
- `apply_nr_color_reduces_chroma_variation` — chroma noise, verify reduction
- `apply_nr_detail_preserves_edges` — sharp edge, verify high detail keeps edges
- `ycbcr_roundtrip` — RGB → YCbCr → RGB is identity
- `atrous_boundary_handling_small_image` — verify correctness on image smaller than kernel reach at coarsest level

### Engine Tests

- `partial_nr_merge_and_materialize`
- `render_with_nr_changes_output`
- `render_default_nr_is_identity`

### Preset Tests

- `nr_section_roundtrip`
- `missing_nr_section_defaults_to_neutral`
- `nr_validation_rejects_out_of_range`

### E2E

Two presets exercised across 5 color test images (10 new golden files) via the existing `ALL_LOOKS` matrix.

## Module Dependencies

No new module dependencies. `denoise.rs` lives in `adjust/` (pure pixel math, no I/O, no engine knowledge) — same as `dehaze.rs` and `detail.rs`. Uses only `serde` and the shared `LUMA_R/G/B` constants from `adjust/mod.rs`.
