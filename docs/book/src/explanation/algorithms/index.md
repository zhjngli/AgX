# Algorithm explanations

This sub-section explains how each editing algorithm works under the hood. Pages are listed in pipeline order — the order in which each stage modifies the image during a render.

## In pipeline order

1. [Basic adjustments](basic.md) — white balance, exposure, tonal sliders.
2. [HSL](hsl.md)
3. [Color grading](color-grading.md)
4. [Tone curves](tone-curves.md)
5. [Dehaze](dehaze.md)
6. [Noise reduction](denoise.md)
7. [Detail pass](detail.md) — sharpening, clarity, texture.
8. [Grain](grain.md)
9. [Vignette](vignette.md)

## Browse by photographer-panel mental model

The same algorithms grouped by the panels used in the [conceptual reference](../../reference/concepts/index.md):

### Basic

- [Basic adjustments](basic.md) — white balance, exposure, tonal sliders.

### Color

- [HSL](hsl.md)
- [Color grading](color-grading.md)
- [Tone curves](tone-curves.md)

### Detail

- [Detail pass](detail.md) — sharpening, clarity, texture.
- [Dehaze](dehaze.md)
- [Noise reduction](denoise.md)

### Effects

- [Grain](grain.md)
- [Vignette](vignette.md)

## See also

- [Conceptual explanations](../concepts/index.md) — architecture, preset-first philosophy, design decisions, and cross-cutting topics like the render pipeline and color spaces.
- [Algorithm API reference](../../api/agx/adjust/index.html) — rustdoc for the `adjust` module.
