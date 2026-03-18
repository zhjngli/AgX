# Vignette Design

## Goal

Add a creative vignette adjustment that darkens or brightens image edges, with two shape options (elliptical and circular), controllable via preset, CLI, and library API.

## Motivation

Vignetting is one of the most common preset adjustments — it draws the viewer's eye toward the center and adds mood. Nearly every film-look preset includes some degree of vignette. It's a natural fit for AgX's preset-first workflow: simple to express in TOML, effective in batch processing, and composable with other adjustments.

This is a **creative vignette** (stylistic choice applied late in the pipeline), not a **lens correction vignette** (undoing optical falloff early in the pipeline). Follows the same pattern as Lightroom and Capture One.

## Parameters

Two parameters:

| Parameter | Type | Range | Default | Description |
|-----------|------|-------|---------|-------------|
| `amount` | f32 | -100 to +100 | 0.0 | Strength. Negative darkens edges, positive brightens. 0 = no effect. |
| `shape` | enum | `elliptical`, `circular` | `elliptical` | Falloff geometry. |

### Shape behavior

**Elliptical (default):** The falloff ellipse matches the image's aspect ratio. All four edges darken evenly. Normalized distance: `d² = (dx/half_w)² + (dy/half_h)²`. This is the standard default in most editors.

**Circular:** The falloff is a circle whose radius equals half the image's long edge. Normalized distance: `d² = (dx/R)² + (dy/R)²` where `R = max(half_w, half_h)`. On a non-square image, the short edges are closer to the circle boundary and darken more than the long edges. Corners darken the most. This simulates real lens image circle vignetting — the lens projects a circular image circle onto the rectangular sensor, and the circle radius is determined by the long edge coverage.

### Falloff curve

Power curve: `factor = clamp(1 - d², 0, 1)^n` with `n = 2` (hardcoded). This produces a smooth, natural-looking falloff that matches industry-standard behavior. The clamp handles `d² > 1` in circular mode where corners extend beyond the circle boundary.

### Amount mapping

`strength = amount / 100.0`

Final pixel value: `pixel * (1.0 + strength * (1.0 - factor))`

This formula works for both darkening (negative strength) and brightening (positive strength). At the center (`factor = 1.0`), the multiplier is 1.0 (no change). At the edges (`factor → 0.0`), the multiplier approaches `1.0 + strength`. Output is clamped to [0.0, 1.0].

## Preset Format

```toml
[vignette]
amount = -30.0
shape = "circular"   # optional, defaults to "elliptical"
```

When `[vignette]` section is absent or `amount` is 0, no vignette is applied. The `shape` field is optional — omitting it defaults to `"elliptical"`.

Vignette parameters participate in preset composability: a child preset can override `amount` and/or `shape` independently from the base preset.

## CLI

```bash
# Darken edges with elliptical falloff (default shape)
agx-cli edit -i photo.jpg -o out.jpg --vignette-amount -30

# Circular vignette
agx-cli edit -i photo.jpg -o out.jpg --vignette-amount -30 --vignette-shape circular
```

## Library API

```rust
engine.params_mut().vignette.amount = -30.0;
engine.params_mut().vignette.shape = VignetteShape::Circular;
```

## Implementation

### New types

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VignetteShape {
    #[default]
    Elliptical,
    Circular,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VignetteParams {
    pub amount: f32,        // default 0.0
    pub shape: VignetteShape, // default Elliptical
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PartialVignetteParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<VignetteShape>,
}
```

### Files modified

| File | Change |
|------|--------|
| `crates/agx/src/adjust/mod.rs` | Add `apply_vignette(r, g, b, amount, shape, x, y, w, h) -> (f32, f32, f32)` and `VignetteShape` enum |
| `crates/agx/src/engine/mod.rs` | Add `VignetteParams` to `Parameters`, `PartialVignetteParams` to `PartialParameters` with `merge()`/`materialize()`/`From` impls, call vignette in render loop after LUT |
| `crates/agx/src/preset/mod.rs` | Add `vignette: Option<PartialVignetteParams>` to `PresetRaw`, propagate in `build_partial_params` |
| `crates/agx/src/lib.rs` | Re-export `VignetteParams`, `PartialVignetteParams`, `VignetteShape` |
| `crates/agx-cli/src/main.rs` | Add `--vignette-amount` and `--vignette-shape` flags |

### Preset composability

`PartialVignetteParams` follows the same pattern as other partial types: `merge()` overlays non-None fields, `materialize()` fills in defaults, and `From<&VignetteParams>` wraps all fields in `Some`. `PartialParameters::merge()` and `PartialParameters::materialize()` are updated to handle the new `vignette` field. No runtime validation of `amount` range — values outside -100..+100 produce extrapolated results, consistent with other adjustments.

### Pipeline position

After LUT application (step 10), before sRGB→linear conversion (step 11) in `engine::render()`. This is the creative vignette position — applied after all tonal and color adjustments, in sRGB gamma space where the perceptual falloff looks natural. Matches Lightroom/Capture One placement.

Early-out: skip entirely when `amount == 0.0`.

### Architecture compliance

- `adjust` module: pure math only, receives pixel values + coordinates + dimensions as parameters. No imports from engine/decode/encode/preset/lut/metadata.
- `engine` module: passes `(x, y, w, h)` to the adjust function. This is the first position-dependent adjustment — existing adjustments are value-only.

## Testing

### Unit tests (adjust module)

- `vignette_zero_amount_is_identity` — amount 0 returns pixel unchanged
- `vignette_center_pixel_unchanged` — center pixel unaffected regardless of amount
- `vignette_corner_darkened` — negative amount darkens corner pixels
- `vignette_corner_brightened` — positive amount brightens corner pixels
- `vignette_circular_top_bottom_darker_than_sides` — on a 3:2 image, circular mode darkens short edges more than long edges
- `vignette_elliptical_edges_even` — on a 3:2 image, compare vignette factor at midpoints of all four edges (top, bottom, left, right) and assert they are equal

### E2E tests

- Add subtle vignette (`amount = -10` to `-15`) to 2-3 existing look presets where it fits the aesthetic (e.g., neo_noir, portra_400)
- Create a dedicated `vignette_test.toml` preset with a strong vignette (`amount = -50`) in isolation — tests the feature without other adjustments obscuring the comparison
- Add `vignette_test` to the test matrix for at least one JPEG and one RAW image
- Regenerate affected golden files

### Library pipeline test

- One test applying vignette via `engine.params_mut().vignette.amount = -30.0` to verify parameter flows through the API

## Out of Scope

- Lens correction vignette (early pipeline, linear space) — separate feature under geometric-corrections
- Midpoint, feather, roundness parameters — can be added later if presets need them
- Off-center vignette — would require center-point parameters, not needed now
