# Tone Curves Design

**Date:** 2026-03-18
**Status:** Approved
**Branch:** `feat/tone-curves`

## Goal

Add point-based tone curves with 5 channels (RGB master, luminance, R, G, B) using monotone cubic hermite interpolation, matching the Lightroom/Capture One tone curve model.

## Motivation

Tone curves are the most powerful and flexible tonal control in photo editing. Point curves allow precise sculpting of tonal response — S-curves for contrast, lifted blacks for film fade, per-channel adjustments for cross-processing and color toning. This is a per-pixel operation that maps directly to preset parameters, closing the biggest remaining gap with Lightroom and Capture One.

## Data Model

### Control Points

Each curve is defined by 2 or more control points stored as `(x, y)` pairs, where x is input value and y is output value, both `f32` in `[0.0, 1.0]`. Points must be strictly ordered by x.

Endpoint constraints:
- First point: x must be 0.0, y can be any value in [0.0, 1.0] (default 0.0)
- Last point: x must be 1.0, y can be any value in [0.0, 1.0] (default 1.0)
- Interior points: x strictly increasing, x in (0.0, 1.0), y in [0.0, 1.0]

Movable endpoints enable lifted blacks (first point y > 0) and capped whites (last point y < 1) — essential for film fade and matte looks.

A curve with just the two default endpoints `[(0.0, 0.0), (1.0, 1.0)]` is the identity (no effect).

### Channels

5 independent curves:

| Channel | Purpose |
|---------|---------|
| `rgb` | Master curve — applies the same mapping to R, G, B independently. Adjusts overall contrast/tonality. Can shift colors due to uneven compression of channel values. |
| `luma` | Luminance-only curve — adjusts brightness without shifting colors. Computes pixel luminance, maps through curve, scales all channels proportionally. |
| `red` | Red channel curve |
| `green` | Green channel curve |
| `blue` | Blue channel curve |

### Structs

```rust
struct ToneCurve {
    points: Vec<(f32, f32)>,  // 2..=N, sorted by x, x[0]=0.0, x[last]=1.0
}

struct ToneCurveParams {
    rgb: ToneCurve,
    luma: ToneCurve,
    red: ToneCurve,
    green: ToneCurve,
    blue: ToneCurve,
}
```

Default for each `ToneCurve` is `[(0.0, 0.0), (1.0, 1.0)]` (identity). `ToneCurveParams::is_default()` returns true when all 5 curves are identity.

### Partial Types (for preset composability)

```rust
struct PartialToneCurve {
    points: Option<Vec<(f32, f32)>>,
}

struct PartialToneCurveParams {
    rgb: Option<PartialToneCurve>,
    luma: Option<PartialToneCurve>,
    red: Option<PartialToneCurve>,
    green: Option<PartialToneCurve>,
    blue: Option<PartialToneCurve>,
}
```

Follows the existing merge (last-write-wins per curve) and materialize (None -> identity) pattern. A curve is either fully specified or absent — no partial point merging across presets (that would be meaningless).

### Preset TOML Format

```toml
[tone_curve.rgb]
points = [[0.0, 0.0], [0.25, 0.20], [0.75, 0.85], [1.0, 1.0]]

[tone_curve.red]
points = [[0.0, 0.0], [0.5, 0.55], [1.0, 1.0]]
```

Only non-identity curves need to appear. Missing channels default to identity.

### Validation

When parsing control points (from preset TOML or CLI):
- Minimum 2 points
- First point x must be 0.0, last point x must be 1.0
- All x strictly increasing
- All x and y in [0.0, 1.0]
- Violations return a preset parse error (not a panic)

## Algorithm

Operates in sRGB gamma space. All precomputation (spline interpolation into LUT) happens once per render, not per pixel.

### Interpolation: Monotone Cubic Hermite (Fritsch-Carlson)

Standard algorithm for tone curves across photo editors. The key property is **monotonicity**: the interpolated curve never overshoots between control points, preventing tonal inversions where brighter input produces darker output.

Regular cubic splines and Catmull-Rom splines can overshoot; monotone cubic hermite avoids this by clamping tangents at each control point using the Fritsch-Carlson method.

Implemented directly — no external crate. The algorithm is ~80 lines of well-documented math:

1. Compute slopes (`delta_k`) between adjacent control points
2. Compute initial tangents (`m_k`) as average of adjacent slopes
3. Apply Fritsch-Carlson monotonicity constraints to clamp tangents
4. Evaluate the hermite basis functions at each LUT entry

### Precomputed Struct

```rust
struct ToneCurvePrecomputed {
    rgb: Option<[f32; 256]>,
    luma: Option<[f32; 256]>,
    red: Option<[f32; 256]>,
    green: Option<[f32; 256]>,
    blue: Option<[f32; 256]>,
}
```

Each `Option` is `None` when that curve is identity (skip the lookup entirely). Created once per `render()` call when `!tone_curve.is_default()`, following the `VignettePrecomputed` / `ColorGradingPrecomputed` pattern.

For each non-identity curve, precompute a 256-entry lookup table by evaluating the monotone cubic hermite spline at x = 0/255, 1/255, ..., 255/255.

### LUT Lookup

Per-pixel evaluation is a table lookup with linear interpolation between adjacent entries:

```rust
fn lut_lookup(lut: &[f32; 256], value: f32) -> f32 {
    let idx = (value * 255.0).clamp(0.0, 255.0);
    let lo = idx.floor() as usize;
    let hi = (lo + 1).min(255);
    let frac = idx - idx.floor();
    lut[lo] + frac * (lut[hi] - lut[lo])
}
```

### Per-Pixel Application Order

Applied in this order: **RGB master -> per-channel -> luminance**.

This order is not commutative — it matters:

1. **RGB master curve** — look up each channel independently: `r = lut_rgb[r]`, `g = lut_rgb[g]`, `b = lut_rgb[b]`. Establishes overall tonal shape (contrast, brightness).
2. **Per-channel curves** — `r = lut_red[r]`, `g = lut_green[g]`, `b = lut_blue[b]`. Fine-tunes individual channels for color work (cross-processing, split toning). Operates on the output of the RGB master, which is the standard Lightroom mental model.
3. **Luminance curve** — compute luma `L = 0.2126*r + 0.7152*g + 0.0722*b` (Rec. 709, same coefficients as color grading), look up `L' = lut_luma[L]`, then scale all channels proportionally: `r *= L'/L`, `g *= L'/L`, `b *= L'/L`, clamped to [0.0, 1.0]. When L is near zero (< 1e-6), skip proportional scaling and set each channel to `L'` instead (uniform gray at the mapped luminance — this handles the "lifted blacks" case where the curve maps 0.0 to a non-zero value). Adjusts brightness without undoing the color work from per-channel curves.

**Rationale for this order:** RGB + per-channel first is the standard Lightroom workflow. Luminance last means it adjusts brightness cleanly without its effect being reshaped by subsequent channel curves. If luminance were first, the RGB/per-channel curves would immediately override its brightness adjustments.

## Pipeline Placement

After contrast/highlights/shadows/whites/blacks, before HSL adjustments. This matches Lightroom's ordering — basic tone adjustments shape the image first, then curves provide precise tonal control, then HSL/color grading/LUT handle color.

Updated pipeline:

1. White balance (linear)
2. Exposure (linear)
3. Convert to sRGB gamma
4. Contrast
5. Highlights, shadows, whites, blacks
6. **Tone curves** (new)
7. HSL adjustments
8. Color grading
9. LUT
10. Vignette
11. Convert back to linear

## CLI Flags

5 flags, each accepting a comma-separated string of `x:y` point pairs:

```
--tc-rgb "0.0:0.0,0.25:0.20,0.75:0.85,1.0:1.0"
--tc-luma "0.0:0.0,0.5:0.6,1.0:1.0"
--tc-red "0.0:0.0,0.5:0.55,1.0:1.0"
--tc-green "0.0:0.0,1.0:1.0"
--tc-blue "0.0:0.0,0.5:0.45,1.0:1.0"
```

Tone curves are primarily used via presets; CLI flags are the escape hatch for scripting and one-off use. Same validation rules apply — parse errors produce clear CLI error messages.

## Files Changed

| File | Change |
|------|--------|
| `crates/agx/src/adjust/mod.rs` | `ToneCurve`, `ToneCurveParams`, `ToneCurvePrecomputed`, Fritsch-Carlson monotone cubic hermite interpolation, `lut_lookup()`, `apply_tone_curves_pre()` |
| `crates/agx/src/engine/mod.rs` | Add `tone_curve: ToneCurveParams` to `Parameters`, `PartialToneCurve` / `PartialToneCurveParams` with merge/materialize/From, insert render step between blacks and HSL |
| `crates/agx/src/preset/mod.rs` | TOML parsing/serialization for `[tone_curve.*]` sections with point validation |
| `crates/agx-cli/src/main.rs` | 5 CLI flags (`--tc-rgb`, `--tc-luma`, `--tc-red`, `--tc-green`, `--tc-blue`) with string parsing |
| `crates/agx/src/lib.rs` | Re-export new public types |

No new modules. No architecture or dependency changes.

## E2E Updates

Per the developer workflow, new editing features must ship with updated presets, LUTs, and golden files:

- Add 2-3 look presets exercising tone curves (e.g., S-curve contrast, faded film, cross-process)
- Regenerate golden files
- Add to e2e test matrix

## Testing Strategy

### Unit Tests (adjust/mod.rs)

- Identity curve (2 default endpoints) -> no change
- S-curve (3+ points) -> midtones shifted, endpoints preserved
- Lifted blacks (first point y > 0) -> black pixels mapped to gray
- Capped whites (last point y < 1) -> white pixels mapped below white
- Per-channel curve -> only that channel affected
- Luminance curve -> brightness changes without color shift
- RGB master -> all channels shifted equally
- Monotonicity: interpolated LUT values never decrease for increasing input (Fritsch-Carlson guarantee)
- Validation: reject < 2 points, wrong endpoint x, non-increasing x, out-of-range values
- LUT lookup interpolation accuracy

### Engine Tests (engine/mod.rs)

- Render with default tone curves -> identical to render without
- Render with tone curves active -> output differs from neutral
- Partial merge: overlay curve replaces base curve entirely
- Materialize: missing curves default to identity

### Preset Tests (preset/mod.rs)

- Round-trip: parse TOML -> serialize -> parse again -> same points
- Missing tone_curve section -> all identity
- Invalid points -> parse error

### E2E Tests

- Golden file comparison for new tone curve presets across the test image matrix
