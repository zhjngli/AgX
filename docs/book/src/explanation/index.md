# Explanation

This section explains how AgX works under the hood. Algorithm explanations live alongside their Rust source as sibling `.md` files; this page is the section landing page.

Each page explains one algorithm. Pages are listed in pipeline order — the order in which each stage modifies the image during a render.

## Browse by photographer-panel mental model

The pages above are listed in pipeline order. The same algorithms grouped by the photographer-panel mental model used in the [conceptual reference](../reference/concepts/index.md):

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
