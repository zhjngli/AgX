# Detail Pass Design (Sharpening, Clarity, Texture)

**Date:** 2026-03-21
**Status:** Approved
**Branch:** `feat/detail-pass`

## Goal

Add sharpening, clarity, and texture adjustments — the first neighborhood operations in AgX. This requires a new buffer-level processing stage after the existing per-pixel render loop, introducing a two-phase render architecture.

## Motivation

Sharpening recovers detail lost in demosaicing and lens softness. Clarity enhances local contrast at medium frequencies (textures, edges). Texture targets fine detail at high frequencies. Together they control perceived image quality and are essential for any serious photo editing workflow. All three are preset-friendly (uniform parameters, no per-photo tuning required).

## Parameters

### Sharpening

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| `amount` | 0–100 | 0 | Strength of sharpening effect |
| `radius` | 0.5–3.0 | 1.0 | Size of detail edges to sharpen (pixels). Maps directly to Gaussian sigma (`sigma = radius`). |
| `threshold` | 0–100 | 25 | Edge magnitude threshold — higher values sharpen finer detail, lower values only sharpen strong edges |
| `masking` | 0–100 | 0 | Limits sharpening to textured areas (protects smooth sky/skin from noise amplification) |

### Clarity & Texture

| Parameter | Range | Default | Description |
|-----------|-------|---------|-------------|
| `clarity` | -100–100 | 0 | Local contrast at medium frequencies (sigma ~20px). Negative values soften. |
| `texture` | -100–100 | 0 | Local contrast at high frequencies (sigma ~3px). Negative values soften. |

All parameters default to 0 (no effect). When all are 0, the detail pass is skipped entirely.

## Data Model

### Structs

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharpeningParams {
    #[serde(default)]
    pub amount: f32,      // 0–100
    #[serde(default = "default_sharpening_radius")]
    pub radius: f32,      // 0.5–3.0, default 1.0
    #[serde(default = "default_sharpening_threshold")]
    pub threshold: f32,   // 0–100, default 25
    #[serde(default)]
    pub masking: f32,     // 0–100
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DetailParams {
    #[serde(default)]
    pub sharpening: SharpeningParams,
    #[serde(default)]
    pub clarity: f32,   // -100–100
    #[serde(default)]
    pub texture: f32,   // -100–100
}
```

Default for `SharpeningParams`: amount=0, radius=1.0, threshold=25, masking=0. Default for `DetailParams`: sharpening defaults, clarity=0, texture=0.

`DetailParams::is_default()` checks only the "effect" fields: sharpening amount == 0, clarity == 0, and texture == 0. The sharpening radius, threshold, and masking values are irrelevant when amount is 0 (no sharpening applied), so they are not checked. This ensures the detail pass is skipped when no feature is active.

### Partial Types (for preset composability)

```rust
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct PartialSharpeningParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    radius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    threshold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    masking: Option<f32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct PartialDetailParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sharpening: Option<PartialSharpeningParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    clarity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    texture: Option<f32>,
}
```

`PartialSharpeningParams` follows the flat-field merge pattern (like `PartialVignetteParams`): each `Option` field uses `overlay.field.or(self.field)`. `materialize()` uses `unwrap_or` with the concrete defaults (amount=0, radius=1.0, threshold=25, masking=0).

`PartialDetailParams` follows the nested merge pattern (like `PartialColorGradingParams`): the `sharpening` field uses the 4-arm match (`None/None → None`, `Some/None → Some(base)`, `None/Some → Some(overlay)`, `Some/Some → Some(base.merge(overlay))`). Scalar fields (`clarity`, `texture`) use `overlay.or(self)`.

Both types need `From<&ConcreteType>` impls (required by `PartialParameters::from(&Parameters)` for `layer_preset`).

`PartialParameters` gets one new field: `detail: Option<PartialDetailParams>`. The `merge`, `materialize`, and `From<&Parameters>` impls on `PartialParameters` are updated accordingly.

## TOML Preset Format

```toml
[detail]
clarity = 30
texture = 15

[detail.sharpening]
amount = 40
radius = 1.0
threshold = 25
masking = 50
```

Everything lives under a single `[detail]` section. Sharpening is a `[detail.sharpening]` sub-table, consistent with how color grading uses `[color_grading.shadows]`, `[color_grading.highlights]`, etc.

In `PresetRaw`, this maps to a single field: `detail: Option<PartialDetailParams>`. The `build_partial_params` function passes `raw.detail.clone()` to `PartialParameters.detail`.

## CLI Flags

```
--sharpen-amount 40
--sharpen-radius 1.0
--sharpen-threshold 25
--sharpen-masking 50
--clarity 30
--texture 15
```

## Pipeline Architecture

### Two-Phase Render

The existing single-pass per-pixel render is split into two phases:

**Phase 1 — Per-pixel loop (existing steps 1-13):**
Runs as today but outputs an sRGB gamma-space buffer instead of converting back to linear. The per-pixel loop stops after vignette (step 13) and skips the linear conversion step (currently step 14).

**Phase 2 — Detail pass (new, buffer-level):**
Operates on the full sRGB gamma buffer. Applies three operations in order:
1. Texture (sigma ~3px, unsharp mask blend)
2. Clarity (sigma ~20px, unsharp mask blend)
3. Sharpening (user sigma, unsharp mask + threshold + masking edge map)

After the detail pass, a final per-pixel pass converts the sRGB buffer back to linear RGB for output encoding.

**When detail is default (all zeros):** The per-pixel loop includes the linear conversion at the end as it does today — no intermediate buffer allocation, no performance cost.

### Application Order

Texture → Clarity → Sharpening. Rationale: build up local contrast at increasing frequency scales first, then sharpen the enhanced result. The internal order can be adjusted later without changing the external interface.

## Algorithm

### Gaussian Blur (shared foundation)

All three features use the same separable Gaussian blur:

1. **Build kernel**: 1D Gaussian weights for `ceil(3 * sigma)` in each direction. Kernel width = `2 * ceil(3 * sigma) + 1`. Normalize weights to sum to 1.0. For sharpening, `sigma = radius` (the radius parameter maps directly to Gaussian sigma). For texture, sigma ≈ 3.0. For clarity, sigma ≈ 20.0.
2. **Horizontal pass**: Convolve each row with the 1D kernel. Edge handling: clamp (repeat edge pixels). Write to temporary buffer.
3. **Vertical pass**: Convolve each column of the horizontal result with the same kernel. Write to output buffer.

Complexity: O(w * h * kernel_width) per blur. Called once per active feature (up to 3 blurs total).

### Luminance Extraction

Convert sRGB buffer to single-channel luminance using Rec.709 weights (`0.2126*R + 0.7152*G + 0.0722*B`). Detail operations work on luminance only, then blend back into all three RGB channels equally. This avoids introducing color fringing.

### Unsharp Mask (Texture & Clarity)

For each pixel:
```
high_freq = luminance - blurred_luminance
output_r = original_r + strength * high_freq
output_g = original_g + strength * high_freq
output_b = original_b + strength * high_freq
```

Where `strength = amount / 100.0`. Negative amounts (clarity, texture) make `strength` negative → softening. Output clamped to [0.0, 1.0]. The strength mapping may need tuning during implementation — `amount / 100.0` is a starting point.

Texture uses sigma ≈ 3.0. Clarity uses sigma ≈ 20.0.

### Sharpening (Unsharp Mask + Threshold + Masking)

Base unsharp mask with user-configurable radius (sigma = radius, 0.5–3.0), plus two gating mechanisms:

**Threshold (edge magnitude gating):**
- Compute `high_freq = luminance - blurred_luminance`
- Suppress high_freq below a magnitude threshold controlled by the threshold slider
- Threshold=100: sharpen everything (threshold at zero). Threshold=0: only sharpen strong edges (high threshold).
- Implementation: `high_freq *= smoothstep(threshold_low, threshold_high, abs(high_freq))`

**Masking (edge map):**
1. Compute luminance gradient magnitude at each pixel using Sobel or simple finite differences
2. Normalize gradient using a fixed scale factor (not per-image normalization) so that masking presets are portable across images. The scale factor should be tuned so that typical edge gradients map to the 0.5–1.0 range.
3. Apply masking slider as a threshold: `mask = smoothstep(threshold, 1.0, gradient)` where threshold scales with the masking parameter (masking=0 → threshold=0 → mask all 1.0 → sharpen everything; masking=100 → threshold high → only sharpen strong edges)
4. Final: `output = original + (amount / 100.0) * high_freq * mask`

### Memory Budget

The detail pass requires temporary buffers:
- 1 luminance buffer (single-channel f32, w*h*4 bytes)
- 1 blurred luminance buffer (same size, reused across features)
- 1 temporary buffer for horizontal blur pass (same size)
- 1 edge map buffer for sharpening masking (same size, only when masking > 0)

For a 24MP image: ~92MB per buffer, up to ~370MB total during the detail pass. Buffers are freed after the pass completes. The luminance and blur buffers can be reused across the three features to reduce peak allocation.

## Module Structure

### New file: `crates/agx/src/adjust/detail.rs`

Keeps `adjust/mod.rs` from growing further (already ~1800 lines with tone curves). Contents:
- `SharpeningParams`, `DetailParams` structs with `Default`, `is_default()`
- `gaussian_blur(luminance_buffer, sigma) -> blurred_buffer` — separable two-pass
- `compute_edge_map(luminance_buffer) -> edge_buffer` — gradient magnitude
- `apply_detail_pass(buffer, params) -> buffer` — orchestrates texture → clarity → sharpening
- Internal helpers: luminance extraction, unsharp mask blend, kernel builder

### Changes to existing files

| File | Change |
|------|--------|
| `crates/agx/src/adjust/mod.rs` | Add `pub mod detail;` and re-export `SharpeningParams`, `DetailParams` |
| `crates/agx/src/engine/mod.rs` | Add `detail: DetailParams` to `Parameters` (+ manual Default impl), `PartialDetailParams`/`PartialSharpeningParams` partial types with merge/materialize/From impls, split render into per-pixel → detail pass → linear conversion |
| `crates/agx/src/preset/mod.rs` | Add `detail` field to `PresetRaw`, validation (parameter ranges), round-trip support |
| `crates/agx-cli/src/main.rs` | Add 6 CLI flags |
| `crates/agx/src/lib.rs` | Re-export new types |
| `ARCHITECTURE.md` | Add design doc link |
| `docs/ideas/README.md` | Remove completed per-pixel idea files (`tone-curves.md`, `color-grading.md`), update neighborhood ops section |

## Testing Strategy

### Unit tests (in `detail.rs`)
- Gaussian blur of uniform buffer → identity (no change)
- Gaussian blur kernel weights sum to 1.0
- Separable blur matches naive 2D blur for small test case
- Unsharp mask with amount=0 → identity
- Negative clarity → output smoother than input (lower variance)
- Masking edge map ≈ 0 in uniform regions, ≈ 1 at sharp edges
- Threshold suppresses low-magnitude detail
- All-default DetailParams → identity output

### Engine tests (in `engine/mod.rs`)
- Default detail params → render unchanged (identity)
- Partial detail merge/materialize
- Render with sharpening produces output different from no sharpening

### Preset tests (in `preset/mod.rs`)
- Round-trip TOML serialization for `[detail]` and `[detail.sharpening]`
- Missing detail section defaults to neutral
- Parameter range validation

### E2E tests
- 2-3 detail look presets:
  - `sharp_landscape` — sharpening + clarity for landscape photography
  - `soft_portrait` — negative texture for skin smoothing
  - `detail_boost` — all three features active
- Added to ALL_LOOKS test matrix, golden files generated

## Future Work

- **Noise reduction**: Wavelet-based or bilateral filtering — different algorithm, separate spec
- **Dehaze**: Dark Channel Prior — different algorithm, separate spec
- **adjust/mod.rs split**: Refactor into per-feature submodules (`adjust/exposure.rs`, `adjust/hsl.rs`, etc.)
- **Optimization**: Downsample-blur-upsample for clarity's larger radius if profiling shows need
- **Alternative algorithms**: Can swap internals (e.g., bilateral filter for clarity) without changing parameter interface
- **Strength tuning**: The `amount / 100.0` strength mapping may need a curve or different divisor to feel natural — tune during implementation
