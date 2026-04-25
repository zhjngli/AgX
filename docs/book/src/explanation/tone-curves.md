# Tone curves

## Pipeline

```mermaid
flowchart TD
    CP["Control points<br/>5 curves: rgb, luma, R, G, B"] --> Build["Build 5 LUTs (256 entries each)<br/>1. Compute secant slopes<br/>2. Seed Hermite tangents<br/>3. Fritsch-Carlson tangent clamp<br/>4. Sample at x = i / 255"]
    Build --> LUTs["5 cached LUTs"]
    Pixel["Per-pixel RGB"] --> Master["Master RGB curve<br/>r, g, b lookup in LUT_rgb"]
    LUTs --> Master
    Master --> PerCh["Per-channel curves<br/>r via LUT_R, g via LUT_G, b via LUT_B"]
    LUTs --> PerCh
    PerCh --> Luma["Luminance curve<br/>l = 0.2126R + 0.7152G + 0.0722B<br/>l_new = LUT_luma(l)<br/>scale = l_new / l<br/>RGB *= scale"]
    LUTs --> Luma
    Luma --> Out["Clamp 0..1"]
```

The Fritsch-Carlson tangent limiter at LUT-build time is what keeps the cubic Hermite interpolation monotone: regular cubic splines can overshoot between control points, inventing tonal reversals the user never specified, so AgX clamps the tangent magnitudes whenever the standard monotonicity test fails. Identity curves are detected and skipped at lookup time so untouched channels add no work to the hot path.

{{#include ../../../../crates/agx/src/adjust/tone_curves.md}}

## See also

- Concept references: [Tone](../reference/concepts/tone.md) (tone curves entry), [Color models](../reference/concepts/color-models.md) (luminance section)
- API references: [tone curves](../api/agx/adjust/tone_curves/index.html)
- Related explanations: [Basic adjustments](basic.md), [HSL](hsl.md), [Color grading](color-grading.md)
