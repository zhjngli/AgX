# Color Spaces in oxiraw

This document explains how oxiraw handles color spaces in its rendering pipeline, and why different operations happen in different color spaces.

## Linear vs sRGB Gamma

There are two common ways to represent color values:

**Linear light** (also called "linear sRGB" or "scene-referred"): Values are proportional to physical light intensity. Double the value = double the photons. This is how light works in the real world: two lamps produce twice as much light as one.

**sRGB gamma** (also called "display-referred"): Values are perceptually spaced for human vision. Our eyes are much more sensitive to changes in dark tones than bright ones. sRGB gamma encoding allocates more of the 0-255 integer range to dark values, which is why JPEGs and PNGs use sRGB gamma by default.

### The conversion

The approximate relationship is a power curve:

- **Linear to sRGB gamma**: `srgb = linear ^ (1/2.2)`
- **sRGB gamma to linear**: `linear = srgb ^ 2.2`

The exact sRGB specification uses a piecewise function with a linear segment near zero, but the power approximation captures the essential idea. oxiraw uses the [palette](https://crates.io/crates/palette) crate for precise conversions.

### Why it matters

A value of 0.5 means different things in each space:

- **Linear 0.5** = 50% of maximum light intensity (physically half as bright as 1.0)
- **sRGB 0.5** = a perceptual midtone (the gray that *looks* halfway between black and white on your screen)

If you do math in the wrong space, you get wrong results. Multiplying linear values by 2 doubles the light (correct exposure adjustment). Multiplying sRGB values by 2 produces a non-physical result that doesn't look right.

## The oxiraw Pipeline

Each operation in the rendering pipeline runs in the color space where it's mathematically correct:

```
Original image (linear sRGB)
  |
  |-- 1. White balance (linear) -- channel multipliers
  |-- 2. Exposure (linear) -- multiply by 2^stops
  |
  |-- Convert: linear -> sRGB gamma
  |
  |-- 3. Contrast (sRGB gamma) -- push values away from midpoint
  |-- 4. Highlights (sRGB gamma) -- adjust bright regions
  |-- 5. Shadows (sRGB gamma) -- adjust dark regions
  |-- 6. Whites (sRGB gamma) -- adjust upper range
  |-- 7. Blacks (sRGB gamma) -- adjust lower range
  |-- 8. HSL adjustments (sRGB gamma) -- per-channel hue/saturation/luminance
  |-- 9. LUT application (sRGB gamma)
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

Applying a LUT designed for sRGB input to linear values would produce incorrect colors. oxiraw applies LUTs in sRGB gamma space, which is correct for the vast majority of creative `.cube` LUTs.

## Current Limitations

oxiraw currently works exclusively in **sRGB** color space. This is the standard color space for displays, web, and consumer photography. JPEG and PNG files are sRGB by default.

For the current scope, this means:
- Decoded images (JPEG, PNG, TIFF) are assumed to be sRGB
- No ICC profile reading or embedding
- No wide-gamut support (Adobe RGB, ProPhoto RGB, Display P3)

## Future: Wider Color Spaces

Future versions may add:

- **Adobe RGB**: Wider gamut for professional print workflows (more greens and cyans)
- **ProPhoto RGB**: Very wide gamut used internally by Lightroom for lossless editing
- **Display P3**: Apple's wide-gamut display standard
- **ICC profile handling**: Read embedded profiles from input images, embed profiles in output
- **Log input LUTs**: Support for LUTs designed for video log curves (S-Log3, LogC)

See `docs/ideas/color-management.md` for the full roadmap.
