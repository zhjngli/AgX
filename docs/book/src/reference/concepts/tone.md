# Tone

Tone refers to the distribution of light and dark in an image — everything that controls *how bright* a pixel is, separately from *what color* it is. The Tone group covers the seven knobs that shape the brightness landscape: a global brightness, a global contrast, four targeted region adjustments, and the freeform tone curve.

## Exposure

Simulates changing the amount of light reaching the sensor. Doubling exposure doubles the linear-light value of every pixel. AgX exposes exposure in stops (a logarithmic unit: +1 stop = 2× brighter, +2 stops = 4× brighter); the AgX range is -5 to +5.

## Contrast

Pushes pixel values away from a midpoint, brightening the highlights and darkening the shadows. AgX applies contrast in sRGB gamma space so the midpoint matches the perceptual middle gray, not the linear-light midpoint (which would crush shadows).

## Highlights

Targets the brightest part of the tonal range. Negative values pull highlights down, recovering blown-out areas; positive values lift them, brightening already-bright regions. AgX defines "highlight" by luminance.

## Shadows

Targets the darkest part of the tonal range. Negative values deepen shadows; positive values lift them, revealing detail in dark areas. AgX defines "shadow" by luminance.

## Whites

Adjusts the upper extreme of the tonal range — the values at or near pure white. Distinct from highlights, which target a broader bright region; whites pulls the maximum down or pushes it up.

## Blacks

Adjusts the lower extreme of the tonal range — the values at or near pure black. Distinct from shadows, which target a broader dark region; blacks pulls the minimum up or pushes it down.

## Tone curves

A per-channel mapping from input value to output value, drawn as a curve. AgX provides five curves: a master RGB curve, a luma curve (tone-only, hue-preserving), and one per R/G/B channel for color-shifting. Tone curves give the most expressive shaping of the brightness landscape but also the easiest way to crush detail or shift color unintentionally.

---

See: [Basic adjustments](../../explanation/basic.md) (white balance, exposure, tonal sliders) and [Tone curves](../../explanation/tone-curves.md) for the algorithm-level math behind these knobs.
