# Noise reduction

## Pipeline

```mermaid
flowchart TD
    RGB[Linear RGB] --> Split["Y = 0.2126R + 0.7152G + 0.0722B<br/>Cb = B - Y, Cr = R - Y"]
    Split --> PerCh["For each channel (Y, Cb, Cr)"]
    PerCh --> Atrous["À trous decomposition<br/>5 levels, B3-spline kernel<br/>gap schedule: 1, 2, 4, 8, 16"]
    Atrous --> Bands["5 detail bands + residual"]
    Bands --> Sigma["Estimate sigma per channel<br/>MAD of finest band / 0.6745"]
    Sigma --> Thresh["Soft-threshold each level<br/>t = sigma * scale[k] * strength<br/>scale = [1.0, 1.0, 1.2, 1.5, 2.0]"]
    Thresh --> Recon["Reconstruct: residual + sum(bands)"]
    Recon --> Combine["Recombine Y, Cb, Cr -> RGB"]
    Combine --> Out["Clamp 0..1"]
```

The `luminance`, `color`, and `detail` sliders parameterize the threshold strengths: `luminance` and `color` map to a `0..3` multiplier on `Y` and on `(Cb, Cr)` respectively, while `detail` only protects the finest-scale luminance band by scaling its threshold down toward zero.

{{#include ../../../../../crates/agx/src/adjust/denoise.md}}

## See also

- Concept references: [Detail](../../reference/concepts/detail.md) (noise reduction entry), [Color models](../../reference/concepts/color-models.md)
- API references: [noise reduction](../../api/agx/adjust/denoise/index.html)
- Related explanations: [Detail pass](detail.md), [Dehaze](dehaze.md), [Grain](grain.md)
- How-tos: [Write your own preset](../../how-to/write-preset.md)
