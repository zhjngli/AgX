# Color Grading Design

**Date:** 2026-03-18
**Status:** Approved
**Branch:** `feat/color-grading`

## Goal

Add 3-way color wheels (shadows, midtones, highlights, global) with a balance slider, matching the Lightroom/Capture One "Color Grading" panel model.

## Motivation

3-way color wheels are the standard creative color grading tool across photo editors. They allow cinematic looks like teal shadows with orange highlights, cool blue midtones, warm global tints, etc. This is a per-pixel operation that maps directly to preset parameters — high alignment with the preset-first batch editing philosophy.

## Data Model

### Color Wheel

Each wheel has three parameters:

| Field | Range | Default | Meaning |
|-------|-------|---------|---------|
| `hue` | 0.0 - 360.0 | 0.0 | Angle on the color wheel (degrees) |
| `saturation` | 0.0 - 100.0 | 0.0 | Distance from center (strength of tint) |
| `luminance` | -100.0 - +100.0 | 0.0 | Brightness shift for this tonal range |

Hue values are stored in degrees and converted to radians during precomputation. Values outside [0, 360) are valid and handled naturally by the trigonometric functions.

Range enforcement is the caller's responsibility (CLI flag parsing and preset validation). Out-of-range values produce undefined visual results but will not cause panics due to per-pixel clamping.

### Full Parameters

```rust
struct ColorWheel {
    hue: f32,
    saturation: f32,
    luminance: f32,
}

struct ColorGradingParams {
    shadows: ColorWheel,
    midtones: ColorWheel,
    highlights: ColorWheel,
    global: ColorWheel,
    balance: f32, // -100 to +100
}
```

Default is all zeros — no effect. `ColorGradingParams::is_default()` uses the `*self == Self::default()` pattern (same as `HslChannels`), allowing the render loop to skip the step entirely.

### Partial Types (for preset composability)

```rust
struct PartialColorWheel {
    hue: Option<f32>,
    saturation: Option<f32>,
    luminance: Option<f32>,
}

struct PartialColorGradingParams {
    shadows: Option<PartialColorWheel>,
    midtones: Option<PartialColorWheel>,
    highlights: Option<PartialColorWheel>,
    global: Option<PartialColorWheel>,
    balance: Option<f32>,
}
```

Follows the existing partial/merge pattern — `merge()` (last-write-wins per field), `materialize()` (None → default), and `From<&ColorGradingParams>` impl. Same structure as `PartialVignetteParams` and `PartialHslChannels`.

### Preset TOML Format

```toml
[color_grading]
balance = -10.0

[color_grading.shadows]
hue = 200.0
saturation = 30.0
luminance = -5.0

[color_grading.highlights]
hue = 30.0
saturation = 25.0
```

All fields optional, default to 0.

## Algorithm

Operates in sRGB gamma space. All precomputation (hue+saturation → RGB tints, balance factor) happens once per render, not per pixel.

### Precomputed Struct

```rust
struct ColorGradingPrecomputed {
    shadow_tint: [f32; 3],
    midtone_tint: [f32; 3],
    highlight_tint: [f32; 3],
    global_tint: [f32; 3],
    shadow_lum: f32,
    midtone_lum: f32,
    highlight_lum: f32,
    global_lum: f32,
    balance_factor: f32,
}
```

Created once per `render()` call when `!color_grading.is_default()`, following the `VignettePrecomputed` pattern.

### Step 1: Precompute Tints

For each wheel, convert polar (hue, saturation) to a multiplicative RGB tint:

```
hue_rad = hue * PI / 180
tint_r = 1.0 + (saturation / 100) * cos(hue_rad)
tint_g = 1.0 + (saturation / 100) * cos(hue_rad - 2*PI/3)
tint_b = 1.0 + (saturation / 100) * cos(hue_rad - 4*PI/3)
```

At saturation=0, tint is (1, 1, 1) — no color shift. This is computed once per render.

### Step 2: Compute Luminance Weights (per pixel)

Pixel luminance: `lum = 0.2126*r + 0.7152*g + 0.0722*b` (Rec. 709 coefficients applied to gamma-encoded values as a perceptual approximation — using them on gamma values emphasizes perceptual brightness separation, which is preferable for creative color grading weight curves).

Balance remaps luminance before weight computation:
- `balance_factor = 2^(-balance / 100)` — range 0.5 to 2.0 (negative balance → factor > 1 → expands shadows)
- `lum_adjusted = lum^balance_factor`

3-way weights from adjusted luminance:
- Shadow weight: `(1 - lum_adjusted)^2`
- Highlight weight: `lum_adjusted^2`
- Midtone weight: `1.0 - shadow_weight - highlight_weight` = `2 * lum_adjusted * (1 - lum_adjusted)`

These always sum to 1.0. The quadratic curves provide smooth crossfades with no hard transitions.

**Open question:** The quadratic exponent is a starting point. The exact curve shape should be refined during the processing parity work (see `docs/ideas/processing-parity.md`) by visual comparison against Lightroom/Capture One output.

### Step 3: Blend and Apply (per pixel)

Compute the regional tint as a weighted blend of the three wheels:

```
regional_r = shadow_tint_r * w_shadow + midtone_tint_r * w_midtone + highlight_tint_r * w_highlight
regional_g = (same for green)
regional_b = (same for blue)
```

Then apply global tint on top:

```
combined_r = regional_r * global_tint_r
combined_g = regional_g * global_tint_g
combined_b = regional_b * global_tint_b
```

Apply to pixel:

```
out_r = clamp(pixel_r * combined_r, 0.0, 1.0)
out_g = clamp(pixel_g * combined_g, 0.0, 1.0)
out_b = clamp(pixel_b * combined_b, 0.0, 1.0)
```

### Step 4: Luminance Shifts

Apply per-wheel luminance shifts, also weighted by the same 3-way weights plus global. This is an additive brightness shift (same offset per channel), which preserves hue but slightly reduces saturation at extremes.

```
lum_shift = shadow_lum * w_shadow + midtone_lum * w_midtone + highlight_lum * w_highlight + global_lum
adjustment = lum_shift / 100.0
out = clamp(out + adjustment, 0.0, 1.0)  // per channel
```

## Pipeline Placement

After HSL adjustments, before LUT application. This matches Lightroom and Capture One ordering. In the render loop, color grading is inserted between the current HSL step and LUT step, shifting LUT and vignette numbering down by one.

Updated pipeline:

1. White balance (linear)
2. Exposure (linear)
3. Convert to sRGB gamma
4. Contrast, highlights, shadows, whites, blacks
5. HSL adjustments
6. **Color grading** (new)
7. LUT
8. Vignette
9. Convert back to linear

## CLI Flags

Prefix `--cg-` to avoid collision with existing `--shadows` (tone adjustment):

```
--cg-shadows-hue, --cg-shadows-sat, --cg-shadows-lum
--cg-midtones-hue, --cg-midtones-sat, --cg-midtones-lum
--cg-highlights-hue, --cg-highlights-sat, --cg-highlights-lum
--cg-global-hue, --cg-global-sat, --cg-global-lum
--cg-balance
```

13 flags total. Color grading is primarily used via presets; CLI flags are for one-off use and batch-edit.

## Files Changed

| File | Change |
|------|--------|
| `crates/agx/src/adjust/mod.rs` | `ColorWheel`, `ColorGradingParams`, `ColorGradingPrecomputed`, `apply_color_grading()` |
| `crates/agx/src/engine/mod.rs` | Add `color_grading: ColorGradingParams` to `Parameters`, `color_grading: Option<PartialColorGradingParams>` to `PartialParameters`, update `merge()` / `materialize()` / `From<&Parameters>`, insert render step |
| `crates/agx/src/preset/mod.rs` | Add `color_grading: Option<PartialColorGradingParams>` to `PresetRaw`, update `build_partial_params()` and `to_toml()` |
| `crates/agx-cli/src/main.rs` | 13 CLI flags (`--cg-*`) |
| `crates/agx/src/lib.rs` | Re-export new public types |

No new modules. No architecture or dependency changes.

## E2E Updates

Per the developer workflow, new editing features must ship with updated presets, LUTs, and golden files:

- Add 2-3 look presets exercising color grading (e.g. teal/orange cinema, cool shadows + warm highlights)
- Generate paired LUTs via agx-lut-gen
- Regenerate golden files
- Add to e2e test matrix

## Testing Strategy

### Unit Tests (adjust/mod.rs)

- All wheels at default → identity (no change)
- Shadow wheel with teal hue → dark pixels shift toward teal, bright pixels unaffected
- Highlight wheel with orange hue → bright pixels shift toward orange, dark pixels unaffected
- Midtone wheel → affects mid-luminance pixels, leaves extremes alone
- Global wheel → uniform tint across all luminances
- Balance negative → expands shadow region
- Balance positive → expands highlight region
- Weights always sum to 1.0 across luminance range
- Saturation=0 on any wheel → that wheel has no color effect regardless of hue

### Engine Tests (engine/mod.rs)

- Render with default color grading → identical to render without
- Render with color grading active → output differs from neutral

### E2E Tests

- Golden file comparison for new color grading presets across the test image matrix
