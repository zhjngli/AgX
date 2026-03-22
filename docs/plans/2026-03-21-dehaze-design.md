# Dehaze Design (Dark Channel Prior)

**Date:** 2026-03-21
**Status:** Approved
**Branch:** `feat/dehaze`

## Goal

Add a dehaze adjustment that recovers contrast and color in hazy images using the Dark Channel Prior algorithm (He et al., 2009), with a guided filter for edge-aware transmission map refinement.

## Motivation

Atmospheric haze reduces contrast and shifts colors toward the ambient light color (usually blue/gray). Dehaze is one of the most popular adjustments in photo editing — essential for landscapes, cityscapes, and outdoor photography. A single `amount` slider controls the effect: positive values remove haze, negative values add a haze/fog effect. Preset-friendly: uniform parameters, no per-photo tuning required.

## Parameters

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| `amount` | -100–100 | 0 | Strength of dehaze effect. Positive removes haze, negative adds haze/fog. |

When amount is 0, the dehaze pass is skipped entirely.

## Data Model

### Structs

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DehazeParams {
    #[serde(default)]
    pub amount: f32,  // -100 to +100
}
```

Default: amount=0. `is_neutral()` checks `amount == 0.0` (follows the `DetailParams::is_neutral()` convention established in the detail pass — "would applying this have any effect?").

### Partial Type

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialDehazeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f32>,
}
```

Follows the flat-field merge pattern (like `PartialVignetteParams`): `overlay.amount.or(self.amount)`. `materialize()` uses `unwrap_or(0.0)`.

`PartialParameters` gets a new field: `dehaze: Option<PartialDehazeParams>`. The `merge`, `materialize`, and `From<&Parameters>` impls are updated accordingly.

## TOML Preset Format

```toml
[dehaze]
amount = 40.0
```

In `PresetRaw`, this maps to a single field: `dehaze: Option<PartialDehazeParams>`. The `build_partial_params` function passes `raw.dehaze.clone()` to `PartialParameters.dehaze`.

## CLI Flags

```
--dehaze-amount 40
```

Single flag. Needs `allow_hyphen_values = true` since negative values add haze.

## Pipeline Architecture

### Multi-Phase Render (when dehaze is active)

Dehaze operates on linear RGB data after white balance and exposure but before gamma conversion and creative tone adjustments. This matches the standard processing order in raw editors (Lightroom, darktable, RawTherapee). The dehaze algorithm operates entirely in linear RGB space and outputs linear RGB, which feeds directly into step 3 (gamma conversion) of the per-pixel loop.

**Dehaze pre-pass (new, buffer-level):**
When dehaze is active, the first two per-pixel steps (white balance + exposure) are applied to build a linear RGB buffer. The DCP algorithm then operates on this buffer. The result replaces the buffer for subsequent processing.

**Per-pixel loop (steps 3-13):**
Continues from the dehazed buffer: gamma conversion, contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading, LUT, vignette. Outputs sRGB gamma buffer.

**Detail pass (existing, buffer-level):**
Operates on the sRGB gamma buffer. Applies texture, clarity, sharpening. Linear conversion is folded into this phase (or into the per-pixel loop when detail is inactive).

**When dehaze is default (amount=0):** The per-pixel loop runs as today with no intermediate buffer allocation — zero performance cost.

**When only detail is active (no dehaze):** Same two-phase architecture as before.

**When both dehaze and detail are active:** Three buffer-level passes: dehaze → per-pixel → detail (+ linear conversion).

### Render Branching

The render method checks `dehaze_active` and `detail_active` up front:

| dehaze | detail | Render path |
|--------|--------|-------------|
| no | no | Single-pass `Rgb32FImage::from_fn` (existing) |
| no | yes | Per-pixel → detail buffer pass → linear (existing two-phase) |
| yes | no | WB+exp buffer → dehaze → per-pixel (reads from dehazed buffer) → `Rgb32FImage` |
| yes | yes | WB+exp buffer → dehaze → per-pixel buffer → detail → linear |

## Algorithm

### Haze Model

The atmospheric scattering model:

```
I(x) = J(x) * t(x) + A * (1 - t(x))
```

Where:
- `I(x)` — observed hazy image
- `J(x)` — scene radiance (what we want to recover)
- `A` — atmospheric light (global constant, the color of the haze)
- `t(x)` — transmission map (0-1, how much scene light reaches the camera)

### Step 1: Dark Channel Computation

For each pixel, compute the minimum value across all three RGB channels within a local patch:

```
dark(x) = min over (y in patch(x)) of (min(I_r(y), I_g(y), I_b(y)))
```

Patch size: 15x15 pixels (standard). This is a 2D minimum filter.

**Efficient implementation:** Separable min filter — horizontal pass (1D min over each row with window width 15), then vertical pass (1D min over each column with window width 15). Each 1D min filter uses a sliding window min algorithm (e.g., monotonic deque) for O(n) complexity per row/column, independent of patch size. Total complexity: O(w * h).

The key insight: in haze-free images, at least one color channel has a very low value in most local patches (due to shadows, colorful objects, etc.). Haze lifts this minimum toward the airlight color.

### Step 2: Atmospheric Light Estimation

1. Select the top 0.1% brightest pixels in the dark channel (highest dark channel values = haziest regions).
2. Among those pixels, find the one with the highest intensity in the original image: `max(I_r + I_g + I_b)`.
3. Use that pixel's RGB values as the atmospheric light `A = [A_r, A_g, A_b]`.

This estimates the color and brightness of the haze. For outdoor scenes, `A` is typically near-white or slightly blue.

### Step 3: Transmission Map Estimation

Normalize the image by the atmospheric light and compute the dark channel of the normalized image:

```
t_raw(x) = 1 - omega * dark_channel(I(x) / A)
```

Where `omega` controls the dehaze strength. For the `amount` slider:
- Positive amount (0 to 100): `omega = amount / 100.0` (remove haze)
- Negative amount (-100 to 0): the effect is reversed — push the image toward the airlight to add haze. Steps 1-2 (dark channel + airlight estimation) still run to obtain `A`, then steps 3-5 are skipped. Instead: `J(x) = I(x) * (1 - |amount|/100) + A * |amount|/100` (linear blend toward airlight).

Division `I(x) / A` is done per-channel. If any `A` component is near-zero (< 0.01), clamp to 0.01 to avoid division instability.

### Step 4: Guided Filter Refinement

The raw transmission map from step 3 has blocky artifacts from the patch-based min filter. A guided filter smooths `t_raw` while preserving edges from the original image.

**Guided filter** (He et al., 2010):

Given guidance image `G` (the original image, converted to grayscale), input `p` (the raw transmission map), and filter radius `r`, window size `(2r+1)`:

For each local window `w_k` centered at pixel `k`:
```
a_k = (mean(G * p) - mean(G) * mean(p)) / (var(G) + epsilon)
b_k = mean(p) - a_k * mean(G)
```

Output: `q(i) = mean_k(a_k) * G(i) + mean_k(b_k)` (average `a_k` and `b_k` over all windows containing pixel `i`).

`epsilon` is a regularization parameter (e.g., 0.001) that controls smoothness. Larger epsilon = smoother result.

**Efficient implementation:** All the `mean()` and `var()` computations use box filters (running sums), making the guided filter O(n) regardless of window size. The guided filter radius should be larger than the dark channel patch radius (e.g., radius = 40 for a patch size of 15) to fully smooth the block artifacts.

**Grayscale conversion** for the guide image uses the same Rec.709 luminance weights as the detail pass: `0.2126*R + 0.7152*G + 0.0722*B`.

### Step 5: Scene Recovery

For positive amounts (dehaze):
```
J(x) = (I(x) - A) / max(t(x), t_min) + A
```

Where `t_min = 0.1` prevents division by very small transmission values (which would amplify noise in dense haze regions).

For negative amounts (add haze):
```
J(x) = I(x) * (1 - strength) + A * strength
```

Where `strength = |amount| / 100.0`. This linearly blends the image toward the estimated airlight color.

### Step 6: Output Clamping

Clamp all output values to [0.0, 1.0]. The recovery step can produce values outside this range when haze is dense and transmission is low.

### Algorithm Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `PATCH_SIZE` | 15 | Dark channel patch width/height (pixels) |
| `AIRLIGHT_PERCENTILE` | 0.001 | Top 0.1% for airlight estimation |
| `T_MIN` | 0.1 | Minimum transmission to prevent noise amplification |
| `GUIDED_FILTER_RADIUS` | 40 | Guided filter window radius |
| `GUIDED_FILTER_EPSILON` | 0.001 | Guided filter regularization |

These are internal constants, not user-facing parameters. They can be tuned during implementation without changing the external interface.

### Memory Budget

The dehaze pass requires temporary buffers:
- 1 dark channel buffer (single-channel f32, w*h*4 bytes)
- 1 transmission map buffer (same size)
- Guided filter intermediates: ~5 single-channel buffers (mean_G, mean_p, mean_Gp, var_G, output)
- Atmospheric light is 3 floats (negligible)

For a 24MP image (e.g., 6000x4000): ~96MB per single-channel buffer, ~670MB peak during the guided filter. Buffers for intermediate guided filter steps can be reused to reduce peak allocation. After the dehaze pass completes, all temporary buffers are freed.

## Module Structure

### New file: `crates/agx/src/adjust/dehaze.rs`

Public entry point:

```rust
pub fn apply_dehaze(
    buf: &[[f32; 3]],
    width: usize,
    height: usize,
    params: &DehazeParams,
) -> Vec<[f32; 3]>
```

Contents:
- `DehazeParams` struct with `Default`, `is_neutral()`
- `min_filter_1d()` — O(n) sliding window min using monotonic deque
- `dark_channel()` — separable 2D min filter via two 1D passes
- `estimate_airlight()` — top percentile selection
- `box_filter_1d()` — O(n) running sum for guided filter
- `guided_filter()` — edge-aware transmission refinement
- `apply_dehaze()` — pub orchestrator (entry point)
- Internal helpers: grayscale conversion, scene recovery, haze addition
- Unit tests

### Changes to existing files

| File | Change |
|------|--------|
| `crates/agx/src/adjust/mod.rs` | Add `pub mod dehaze;` and re-export `DehazeParams` |
| `crates/agx/src/engine/mod.rs` | Add `dehaze: DehazeParams` to `Parameters`, `PartialDehazeParams` with merge/materialize/From impls, three-phase render branching |
| `crates/agx/src/preset/mod.rs` | Add `dehaze` field to `PresetRaw`, validation (amount range), round-trip support |
| `crates/agx-cli/src/main.rs` | Add `--dehaze-amount` CLI flag |
| `crates/agx/src/lib.rs` | Re-export new types |
| `ARCHITECTURE.md` | Add design doc link |

## Testing Strategy

### Unit tests (in `dehaze.rs`)
- Dark channel of uniform buffer → uniform value
- Dark channel picks minimum across RGB channels in patch
- Min filter on impulse → spreads the minimum across patch window
- Airlight estimation selects brightest pixel in haziest region
- Guided filter with uniform input → identity (no change)
- Guided filter preserves sharp edges (step edge in guide)
- `apply_dehaze` with amount=0 → identity
- Positive amount reduces haze (output has more contrast than input)
- Negative amount adds haze (still runs dark channel + airlight estimation to get `A`, then blends toward `A`)
- Output clamped to [0, 1]
- Scene recovery with t_min prevents extreme values

### Engine tests (in `engine/mod.rs`)
- Default dehaze → render unchanged (identity)
- Partial dehaze merge/materialize
- Render with dehaze amount>0 produces different output than neutral

### Preset tests (in `preset/mod.rs`)
- Round-trip TOML serialization for `[dehaze]`
- Missing dehaze section defaults to neutral
- Parameter range validation (amount must be -100 to 100)

### E2E tests
- 2 dehaze presets:
  - `dehaze_landscape` — amount=50 (moderate dehaze)
  - `haze_effect` — amount=-30 (add haze/fog)
- Added to ALL_LOOKS test matrix, golden files generated

## Future Work

- **Adaptive patch size**: Scale patch size with image resolution for consistent behavior across resolutions
- **Performance**: Downsample for dark channel and transmission estimation, upsample result (common optimization for large images)
- **Alternative algorithms**: Can swap DCP internals (e.g., color attenuation prior, learning-based methods) without changing parameter interface
- **Sky detection**: Improve handling of bright sky regions where DCP assumptions break down (soft matting or separate sky mask)
