# Basic adjustments

The Basic-panel sliders below share a mental model (they all sit at the "top of the stack" in Lightroom / Capture One), but their math divides cleanly by color space. White balance and exposure run in linear light before gamma encoding; the tone sliders run in sRGB gamma space after. See [Color spaces](../../reference/concepts/color-spaces.md) for the linear-vs-gamma distinction and the per-stage rationale.

## White balance

{{#include ../../../../../crates/agx/src/adjust/white_balance.md}}

## Exposure

{{#include ../../../../../crates/agx/src/adjust/exposure.md}}

## Tone sliders

{{#include ../../../../../crates/agx/src/adjust/basic_tone.md}}

## See also

- Concept references: [Tone](../../reference/concepts/tone.md), [Color](../../reference/concepts/color.md) (white balance entry), [Color spaces](../../reference/concepts/color-spaces.md)
- API references: [white balance](../../api/agx/adjust/white_balance/index.html), [exposure](../../api/agx/adjust/exposure/index.html), [basic tone](../../api/agx/adjust/basic_tone/index.html)
- Related explanations: [HSL](hsl.md), [Tone curves](tone-curves.md), [Color grading](color-grading.md)
- How-tos: [Write your own preset](../../how-to/write-preset.md), [Apply a preset to a folder](../../how-to/batch-apply.md)
