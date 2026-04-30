# Color grading

## Pipeline

```mermaid
flowchart TD
    Pixel["Per-pixel RGB (gamma space)"] --> Lum["luminance l (Rec. 709)"]
    Lum --> Bal["l_adj = l ^ (2 ^ (-balance/100))"]
    Bal --> Masks["zone weights<br/>w_s = (1 - l_adj)^2<br/>w_h = l_adj^2<br/>w_m = 1 - w_s - w_h"]
    SH["Shadow wheel<br/>(hue, sat, lum)"] --> RT
    MID["Midtone wheel"] --> RT
    HI["Highlight wheel"] --> RT
    Masks --> RT["regional_tint =<br/>w_s*tint_s + w_m*tint_m + w_h*tint_h"]
    GL["Global wheel"] --> CT
    RT --> CT["combined_tint =<br/>regional_tint * tint_g"]
    Pixel --> Mul["pixel * combined_tint"]
    CT --> Mul
    Mul --> Add["+ weighted luminance offsets<br/>w_s*lum_s + w_m*lum_m + w_h*lum_h + lum_g"]
    Masks --> Add
    Add --> Out["Clamp 0..1"]
```

Each wheel's hue/saturation pair is converted to an RGB `tint` once per render via three cosine lobes spaced 120° apart; the `balance` exponent and the precomputed wheel data are then fixed inputs to the per-pixel inner loop. The three zone weights always sum to one, so regions blend smoothly instead of producing hard boundaries.

{{#include ../../../../../crates/agx/src/adjust/color_grading.md}}

## See also

- Concept references: [Color](../../reference/concepts/color.md) (color grading entry), [Color models](../../reference/concepts/color-models.md)
- API references: [color grading](../../api/agx/adjust/color_grading/index.html)
- Related explanations: [Basic adjustments](basic.md), [HSL](hsl.md), [Tone curves](tone-curves.md)
- How-tos: [Write your own preset](../../how-to/write-preset.md), [Compose layered looks](../../how-to/compose-looks.md)
