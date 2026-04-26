# Color Spaces in AgX

This document explains how AgX handles color spaces in its rendering pipeline, and why different operations happen in different color spaces.

## Linear vs sRGB Gamma

There are two common ways to represent color values:

**Linear light** (also called "linear sRGB" or "scene-referred"): Values are proportional to physical light intensity. Double the value = double the photons. This is how light works in the real world: two lamps produce twice as much light as one.

**sRGB gamma** (also called "display-referred"): Values are perceptually spaced for human vision. Our eyes are much more sensitive to changes in dark tones than bright ones. sRGB gamma encoding allocates more of the 0-255 integer range to dark values, which is why JPEGs and PNGs use sRGB gamma by default.

### The conversion

The approximate relationship is a power curve:

- **Linear to sRGB gamma**: `srgb = linear ^ (1/2.2)`
- **sRGB gamma to linear**: `linear = srgb ^ 2.2`

The exact sRGB specification uses a piecewise function with a linear segment near zero, but the power approximation captures the essential idea. AgX uses the [palette](https://crates.io/crates/palette) crate for precise conversions.

### Why it matters

A value of 0.5 means different things in each space:

- **Linear 0.5** = 50% of maximum light intensity (physically half as bright as 1.0)
- **sRGB 0.5** = a perceptual midtone (the gray that *looks* halfway between black and white on your screen)

If you do math in the wrong space, you get wrong results. Multiplying linear values by 2 doubles the light (correct exposure adjustment). Multiplying sRGB values by 2 produces a non-physical result that doesn't look right.

## Working space

A render pipeline does math on pixel values somewhere in the linear-vs-gamma spectrum. The space the pipeline does its math in is the **working space**. AgX's working space is sRGB — both the linear-light variant (for physical operations like exposure, white balance, dehaze, and noise reduction) and the gamma-encoded variant (for perceptual operations like contrast and tone curves).

Working in sRGB constrains the gamut to what an sRGB display can show. The trade-off is simplicity and a clean match to the input format that consumer cameras and editing software already use. Wider working spaces (Adobe RGB, ProPhoto RGB, Display P3) can hold colors that would clip in sRGB, at the cost of more careful color management.

See [Color models](color-models.md) for the channel structure inside each space, and the [render pipeline overview](render-pipeline.md) for where the conversions happen.

## The AgX Pipeline

Each operation in the rendering pipeline runs in the color space where it's mathematically correct:

```
Original image (linear sRGB)
  |
  |-- White balance (linear) -- per-channel multipliers
  |-- Exposure (linear) -- multiply by 2^stops
  |-- Dehaze (linear) -- local-contrast restoration
  |-- Noise reduction (linear) -- luminance + chroma denoise
  |
  |-- Convert: linear -> sRGB gamma
  |
  |-- Per-pixel adjustments (sRGB gamma):
  |     contrast, highlights, shadows, whites, blacks,
  |     tone curves, HSL, color grading, LUT
  |
  |-- Detail pass (sRGB gamma) -- sharpen, clarity, texture
  |-- Grain (sRGB gamma)
  |-- Vignette (sRGB gamma)
  |
  |-- Convert: sRGB gamma -> linear
  |
  Output (linear sRGB) -> encode to file
```

### Why exposure and white balance are in linear space

These are **physical** operations:

- **Exposure** simulates changing the amount of light hitting the sensor. Doubling the light means doubling the linear value. The formula `value * 2^stops` only works correctly in linear space.
- **White balance** adjusts the relative intensity of color channels to correct for the color temperature of the light source. This is a physical property of light, so it must operate on linear (physically proportional) values.

### Why tone adjustments are in sRGB gamma space

These are **perceptual** operations:

- **Contrast** pushes values away from or toward a midpoint. The "midpoint" that looks right is the perceptual midtone (sRGB 0.5), not the physical midpoint (linear 0.5, which looks very bright).
- **Highlights, shadows, whites, blacks** target specific tonal regions. These regions are defined by how they *look* on screen, which means they're defined in the perceptual (sRGB gamma) space.

If you applied contrast in linear space, the result would look wrong: the midpoint would be too bright, and shadows would get crushed while highlights barely change.

### Why LUTs are in sRGB gamma space

LUTs are created by colorists while looking at a screen displaying sRGB. When a colorist tweaks a film emulation LUT, they're working with pixel values as they appear on screen (sRGB gamma). The input-output mapping in the LUT corresponds to sRGB values, not linear light values.

Applying a LUT designed for sRGB input to linear values would produce incorrect colors. AgX applies LUTs in sRGB gamma space, which is correct for the vast majority of creative `.cube` LUTs.

## Current limitations

AgX currently works exclusively in **sRGB** color space. This is the standard color space for displays, web, and consumer photography. JPEG and PNG files are sRGB by default.

For the current scope, this means:

- Decoded images (JPEG, PNG, TIFF) are assumed to be sRGB
- No ICC profile reading or embedding
- No wide-gamut support (Adobe RGB, ProPhoto RGB, Display P3)

## Future: wider color spaces

Future versions may add:

- **Adobe RGB**: Wider gamut for professional print workflows (more greens and cyans)
- **ProPhoto RGB**: Very wide gamut used internally by Lightroom for lossless editing
- **Display P3**: Apple's wide-gamut display standard
- **ICC profile handling**: Read embedded profiles from input images, embed profiles in output
- **Log input LUTs**: Support for LUTs designed for video log curves (S-Log3, LogC)
