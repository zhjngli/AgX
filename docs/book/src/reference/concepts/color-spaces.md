# Color spaces in AgX

AgX's render pipeline does math on pixel values in two related color spaces: **linear Rec.2020** (the working space for physical operations) and **gamma-encoded Rec.2020** (the working space for perceptual operations). Each stage runs in the space where its math is physically or perceptually correct. This page is the lookup reference for definitions, conversions, gamut matrices, and the per-stage assignment.

## Linear vs gamma-encoded

There are two common ways to represent color values:

**Linear light** (also called "scene-referred"): values are proportional to physical light intensity. Double the value = double the photons. This is how light works in the real world. The specific linear space AgX uses internally is **linear Rec.2020**.

**Gamma-encoded** (also called "display-referred"): values are perceptually spaced for human vision. Our eyes are much more sensitive to changes in dark tones than bright ones. A gamma encoding allocates more of the 0–1 range to dark values, which is why JPEGs and PNGs use a gamma encoding by default.

AgX's gamma working space reuses the **sRGB transfer-curve shape** applied to Rec.2020 linear values. A sign-preserving variant of the curve handles small negative components that can arise from wide-gamut matrix conversions.

### The conversion

The approximate relationship is a power curve:

- **Linear to gamma**: `gamma = linear ^ (1/2.2)`
- **Gamma to linear**: `linear = gamma ^ 2.2`

The exact sRGB specification uses a piecewise function with a linear segment near zero, and AgX uses that exact shape. The sign-preserving form is:

```text
gamma  = sign(x) * srgb_curve(|x|)
linear = sign(x) * srgb_curve_inverse(|x|)
```

### What 0.5 means in each space

- **Linear 0.5** = 50% of maximum light intensity (physically half as bright as 1.0).
- **Gamma 0.5** = a perceptual midtone (the gray that *looks* halfway between black and white on screen).

Doing math in the wrong space produces wrong results. Multiplying linear values by 2 doubles the light (correct exposure adjustment). Multiplying gamma values by 2 produces a non-physical result that doesn't look right.

## Gamut and matrices

AgX accepts three primary input gamuts and converts each into linear Rec.2020 at decode:

| Input primaries | Conversion to linear Rec.2020 |
|-----------------|-------------------------------|
| sRGB / BT.709   | 3×3 matrix (see below)        |
| Display P3      | 3×3 matrix (see below)        |
| BT.2020         | identity (primaries match Rec.2020) |

PQ and HLG transfer-encoded inputs are not parsed as HDR; the decoder treats them as sRGB with a stderr warning.

**Linear Rec.2020 → linear sRGB:**

|     | R         | G         | B         |
|-----|-----------|-----------|-----------|
| R   |  1.660491 | -0.587641 | -0.072850 |
| G   | -0.124550 |  1.132899 | -0.008349 |
| B   | -0.018151 | -0.100579 |  1.118730 |

**Linear sRGB → linear Rec.2020:**

|     | R        | G        | B        |
|-----|----------|----------|----------|
| R   | 0.627404 | 0.329283 | 0.043313 |
| G   | 0.069097 | 0.919541 | 0.011362 |
| B   | 0.016391 | 0.088013 | 0.895595 |

**Linear Display P3 → linear Rec.2020:**

|     | R         | G        | B        |
|-----|-----------|----------|----------|
| R   |  0.753833 | 0.198597 | 0.047570 |
| G   |  0.045744 | 0.941776 | 0.012480 |
| B   | -0.001210 | 0.017601 | 0.983610 |

The Rec.2020 gamut contains the P3 gamut, but P3 primaries expressed in Rec.2020 coordinates can produce small negative components (e.g., the blue channel of P3 red is ≈ −0.0012). Downstream stages tolerate these; the final encode clamps to the 0–1 range.

## Working space

AgX's working space is **linear Rec.2020** for physical operations and **gamma-encoded Rec.2020** for perceptual operations. The gamma encoding uses the sRGB transfer-curve shape applied to Rec.2020 linear values, in the sign-preserving variant.

## Per-stage table

Each stage runs in the space where its math is correct.

| Stage | Color space |
|-------|-------------|
| White balance | Linear Rec.2020 |
| Exposure | Linear Rec.2020 |
| Dehaze | Linear Rec.2020 |
| Noise reduction | Linear Rec.2020 |
| Contrast, highlights, shadows, whites, blacks | Gamma Rec.2020 |
| Tone curves | Gamma Rec.2020 |
| HSL adjustments | Gamma Rec.2020 |
| Color grading | Gamma Rec.2020 |
| LUT | Gamma Rec.2020 (sampled in sRGB gamma via the engine's conversion bracket) |
| Detail pass (sharpen, clarity, texture) | Gamma Rec.2020 |
| Grain | Gamma Rec.2020 |
| Vignette | Gamma Rec.2020 |

The LUT stage lives in the gamma-Rec.2020 portion of the pipeline, but the LUT itself is sampled in sRGB gamma — the engine wraps the LUT call with a conversion bracket so third-party `.cube` LUTs authored against sRGB continue to work unchanged.

## See also

- [Why each stage runs where it runs](../../explanation/concepts/color-spaces.md) — the design rationale.
- [Color models](color-models.md) — the channel structure inside each space.
- [Render pipeline](render-pipeline.md) — where in the pipeline the linear↔gamma conversions happen.
