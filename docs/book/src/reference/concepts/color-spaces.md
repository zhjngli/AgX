# Color spaces in AgX

AgX's render pipeline does math on pixel values in two related color spaces: linear sRGB and sRGB gamma. Each stage runs in the space where its math is physically or perceptually correct. This page is the lookup reference for definitions, conversions, and the per-stage assignment.

## Linear vs sRGB gamma

There are two common ways to represent color values:

**Linear light** (also called "linear sRGB" or "scene-referred"): values are proportional to physical light intensity. Double the value = double the photons. This is how light works in the real world.

**sRGB gamma** (also called "display-referred"): values are perceptually spaced for human vision. Our eyes are much more sensitive to changes in dark tones than bright ones. sRGB gamma encoding allocates more of the 0–255 integer range to dark values, which is why JPEGs and PNGs use sRGB gamma by default.

### The conversion

The approximate relationship is a power curve:

- **Linear to sRGB gamma**: `srgb = linear ^ (1/2.2)`
- **sRGB gamma to linear**: `linear = srgb ^ 2.2`

The exact sRGB specification uses a piecewise function with a linear segment near zero, but the power approximation captures the essential idea. AgX uses the [palette](https://crates.io/crates/palette) crate for precise conversions.

### What 0.5 means in each space

- **Linear 0.5** = 50% of maximum light intensity (physically half as bright as 1.0).
- **sRGB 0.5** = a perceptual midtone (the gray that *looks* halfway between black and white on screen).

Doing math in the wrong space produces wrong results. Multiplying linear values by 2 doubles the light (correct exposure adjustment). Multiplying sRGB values by 2 produces a non-physical result that doesn't look right.

## Working space

A render pipeline does math on pixel values somewhere in the linear-vs-gamma spectrum. The space the pipeline does its math in is the **working space**. AgX's working space is sRGB — both the linear-light variant (for physical operations) and the gamma-encoded variant (for perceptual operations).

## Per-stage table

Each stage runs in the space where its math is correct.

| Stage | Color space |
|-------|-------------|
| White balance | Linear |
| Exposure | Linear |
| Dehaze | Linear |
| Noise reduction | Linear |
| Contrast, highlights, shadows, whites, blacks | sRGB gamma |
| Tone curves | sRGB gamma |
| HSL adjustments | sRGB gamma |
| Color grading | sRGB gamma |
| LUT | sRGB gamma |
| Detail pass (sharpen, clarity, texture) | sRGB gamma |
| Grain | sRGB gamma |
| Vignette | sRGB gamma |

## See also

- [Why each stage runs where it runs](../../explanation/concepts/color-spaces.md) — the design rationale.
- [Color models](color-models.md) — the channel structure inside each space.
- [Render pipeline](render-pipeline.md) — where in the pipeline the linear↔gamma conversions happen.
