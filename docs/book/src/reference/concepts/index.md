# Concepts

The conceptual reference covers the photographic and AgX-specific ideas the rest of the documentation builds on. It serves CLI users, preset authors, and curious photo nerds — readers who want to look up *what* a concept is, separately from the algorithmic *how* (covered under [Explanation](../../explanation/index.md)) and the field-level schema (covered by the auto-generated [Preset format](../preset.md) page).

## Foundations

The substrate everything else relies on.

- [Color spaces](color-spaces.md) — Linear vs sRGB definitions, conversion formulas, and per-stage assignment.
- [Color models](color-models.md) — RGB, HSL, and luminance: when AgX uses each.

## Photography lexicon

Short entries grouped by photographer-panel mental model. Tutorials and how-to guides cite these by anchor (e.g., `color.md#white-balance`).

- [Tone](tone.md) — Exposure, contrast, highlights, shadows, whites, blacks, tone curves.
- [Color](color.md) — White balance, HSL, color grading.
- [Detail](detail.md) — Sharpening, clarity, dehaze, noise reduction.
- [Effects](effects.md) — Grain, vignette.

## AgX-specific

Concepts that aren't covered by general photography references because they are AgX inventions or AgX integrations.

- [Preset model](preset-model.md) — The three-part structure and the `extends` chain merge semantics.
- [Render pipeline](render-pipeline.md) — The conceptual journey from decoded image to encoded output.
- [LUT format](lut-format.md) — `.cube` syntax, trilinear interpolation, and supported sizes and features.
