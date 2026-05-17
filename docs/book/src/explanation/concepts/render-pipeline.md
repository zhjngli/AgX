# Render pipeline

The render pipeline is a fixed sequence of stages, and the stage order is load-bearing. This page explains why — what would break if the order changed, and what design constraints the order encodes.

If you want to look up which stage runs in which color space, see the [render pipeline reference](../../reference/concepts/render-pipeline.md).

## Why pipeline order matters

Stage order is not interchangeable. A few examples:

- **Exposure before tonal sliders.** Exposure scales linear light; tonal sliders re-shape the resulting brightness landscape. Reversing them would re-shape unexposed values, then scale the result, which produces different output.
- **Dehaze and denoise in linear space.** Both operate on physical light intensities — dehaze increases local contrast where atmospheric haze has reduced it, and denoise smooths sensor noise. Running them in linear space (before gamma encoding) keeps the math operating on the same domain the optical effects originate in.
- **Dehaze before denoise.** Dehaze can amplify low-level structure that includes noise; denoise then cleans up the result. Reversing them would let dehaze re-amplify noise that denoise had just removed.
- **LUT inside the per-pixel pass, before detail.** The LUT applies a creative color transform; sharpening and clarity then operate on the graded result. Sharpening before the LUT would amplify edges that the LUT would then re-grade.
- **Grain after detail and dehaze, before vignette.** Grain is added texture; the surrounding stages should not modify it. Vignette is a final overlay that doesn't disturb grain structure.

The stage order encodes design decisions made when each adjustment was added. The [algorithm explanations](../algorithms/index.md) for each algorithm record the algorithm-specific reasoning.

## See also

- [Render pipeline reference](../../reference/concepts/render-pipeline.md) — the lookup view of the pipeline.
- [Color spaces](color-spaces.md) — why operations live in linear vs gamma Rec.2020 working space.
- [Algorithm explanations](../algorithms/index.md) — per-algorithm walkthroughs.
