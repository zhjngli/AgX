# HSL Adjustments Design

**Date**: 2026-03-05
**Status**: Approved

## Overview

Add per-channel HSL (Hue, Saturation, Luminance) adjustments to the editing pipeline. Users can target 8 color ranges — Red, Orange, Yellow, Green, Aqua, Blue, Purple, Magenta — and independently shift hue, adjust saturation, and modify luminance for each. This is a core feature in every professional photo editor, essential for portrait retouching, landscape work, and creative color grading.

## Data Model

### HslChannel and HslChannels

Two new structs live in `engine/mod.rs` alongside `Parameters`:

```rust
/// Per-channel HSL adjustment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HslChannel {
    #[serde(default)]
    pub hue: f32,        // -180.0 to +180.0 (degrees of hue shift)
    #[serde(default)]
    pub saturation: f32, // -100.0 to +100.0
    #[serde(default)]
    pub luminance: f32,  // -100.0 to +100.0
}

/// HSL adjustments for all 8 color channels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HslChannels {
    #[serde(default)] pub red: HslChannel,
    #[serde(default)] pub orange: HslChannel,
    #[serde(default)] pub yellow: HslChannel,
    #[serde(default)] pub green: HslChannel,
    #[serde(default)] pub aqua: HslChannel,
    #[serde(default)] pub blue: HslChannel,
    #[serde(default)] pub purple: HslChannel,
    #[serde(default)] pub magenta: HslChannel,
}
```

`Parameters` gets one new field:

```rust
pub struct Parameters {
    // ... existing 8 scalar fields ...
    pub hsl: HslChannels,
}
```

User-facing ranges (-180/+180 degrees for hue, -100/+100 for saturation and luminance) are stored as-is in `Parameters` and TOML. The adjust function internally normalizes to whatever the math needs.

## Channel Model

8 channels with non-uniform hue spacing, matching the Lightroom/Capture One standard:

| Channel  | Center hue | Gap to next |
|----------|-----------|-------------|
| Red      | 0°        | 30°         |
| Orange   | 30°       | 30°         |
| Yellow   | 60°       | 60°         |
| Green    | 120°      | 60°         |
| Aqua     | 180°      | 60°         |
| Blue     | 240°      | 30°         |
| Purple   | 270°      | 60°         |
| Magenta  | 330°      | 30°         |

The warm range (Red/Orange/Yellow) gets tighter 30° spacing for finer skin tone control — the #1 use case for HSL adjustments. Cool tones use 60° spacing. This is a deliberate design choice, not a property of the hue wheel.

## Algorithm

### Weight function (pluggable)

Each channel's influence on a pixel is determined by a weight function that maps hue distance (in degrees) to a 0.0–1.0 weight. The weight function is passed as a `fn(f32) -> f32` parameter to `apply_hsl`, making it pluggable — swap cosine for polynomial or Gaussian in the future without changing the HSL logic.

Default implementation — cosine falloff:

```rust
/// Cosine falloff: smooth bell curve, 1.0 at center, 0.0 at half_width.
pub fn cosine_weight(hue_distance: f32, half_width: f32) -> f32 {
    if hue_distance >= half_width {
        0.0
    } else {
        ((hue_distance / half_width) * std::f32::consts::PI).cos() * 0.5 + 0.5
    }
}
```

The `half_width` is per-channel, derived from the gap to neighboring channels, so channels in the tightly-packed warm range have narrower influence than those in the wider cool range.

### Core function

```rust
pub fn apply_hsl(
    r: f32, g: f32, b: f32,
    channels: &HslChannels,
    weight_fn: fn(f32, f32) -> f32,
) -> (f32, f32, f32)
```

Per-pixel steps (sRGB gamma space):

1. **Early exit** if all 8 channels are at defaults (zero hue/sat/lum) — skip the color space conversion entirely
2. **Convert RGB to HSL** via `palette` crate: `Hsl::from_color(Srgb::new(r, g, b))`
3. **For each of the 8 channels**, compute the hue distance from the pixel's hue to the channel center (wrapping at 360°), then compute the weight via `weight_fn(distance, half_width)`. Accumulate weighted hue shift, saturation delta, and luminance delta
4. **Apply accumulated adjustments**: shift hue (wrapping at 360°), add saturation delta (clamped), add luminance delta (clamped)
5. **Convert HSL back to RGB**: `Srgb::from_color(hsl)`

## Pipeline Integration

HSL adjustments operate in sRGB gamma space and slot in after tone adjustments, before LUT application. This matches the industry standard: tone adjustments set the base, HSL fine-tunes colors, then the LUT applies a creative grade on the corrected result.

```
Original (linear)
  → 1. White balance (linear)
  → 2. Exposure (linear)
  → 3. Linear → sRGB gamma
  → 4. Contrast (sRGB gamma)
  → 5. Highlights (sRGB gamma)
  → 6. Shadows (sRGB gamma)
  → 7. Whites (sRGB gamma)
  → 8. Blacks (sRGB gamma)
  → 9. HSL adjustments (sRGB gamma)     ← NEW
  → 10. LUT application (sRGB gamma)
  → 11. sRGB gamma → linear
  → Encode to output file
```

## Preset Format

New `[hsl]` TOML section with sub-tables per channel:

```toml
[metadata]
name = "Portrait Warmth"

[tone]
exposure = 0.3

[hsl.red]
hue = 5.0
saturation = -15.0

[hsl.orange]
saturation = 10.0
luminance = 5.0

[hsl.green]
saturation = -40.0
```

Missing `[hsl]` section, missing channels, or missing fields within a channel all default to 0.0 via `#[serde(default)]`. The preset module adds an `HslSection` type alias or direct mapping to `HslChannels` on `PresetRaw`.

## CLI

The `edit` subcommand gets flags per channel per axis, with short aliases:

```bash
# Long form
oxiraw edit -i photo.jpg -o out.jpg --hsl-red-hue 15 --hsl-red-saturation -30

# Short aliases
oxiraw edit -i photo.jpg -o out.jpg --hsl-red-h 15 --hsl-red-s -30 --hsl-green-l 10
```

24 flags total (8 channels x 3 axes), each with a short alias (`-h`, `-s`, `-l` suffix). All default to 0.0. Users only pass the ones they need. Implemented via clap's `visible_alias`.

## Scope

**In scope:**
- `HslChannel` and `HslChannels` structs on `Parameters`
- `apply_hsl()` function in adjust module with pluggable weight function
- `cosine_weight()` as default weight function
- Engine integration (step 9, after blacks, before LUT)
- Preset `[hsl]` TOML section
- CLI `--hsl-{channel}-{axis}` flags with short aliases
- Unit tests for weight function, per-channel targeting, hue wrapping, early exit
- Engine integration tests

**Out of scope:**
- Alternative weight functions (polynomial, Gaussian) — the interface supports them but we only implement cosine
- Color range visualization or UI
- Interaction with future color grading (3-way wheels)

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| Nested `HslChannels` struct, not flat fields | 24 flat fields would bloat `Parameters`. Nested struct maps naturally to TOML sub-tables and reads clearly in code (`params.hsl.red.hue`). |
| 8 channels with non-uniform spacing | Matches Lightroom/Capture One. Finer warm-tone control for skin tones (30° spacing) vs wider cool-tone spacing (60°). |
| Cosine weight falloff | Smooth transitions, no visible banding at channel boundaries. Trivial cost vs piecewise linear. |
| Pluggable weight function | Pass `fn(f32, f32) -> f32` to `apply_hsl`. Swap to polynomial/Gaussian later without changing HSL logic. |
| Pipeline position: after tone, before LUT | Industry standard. Tone sets the base, HSL fine-tunes colors, LUT applies creative grade on corrected input. |
| User-facing ranges (degrees, -100/+100) | Intuitive for users. Internal normalization is hidden in the adjust function. |
