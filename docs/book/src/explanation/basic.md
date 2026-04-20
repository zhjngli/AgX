# Basic adjustments

<!-- TODO(sub-project-5): replace this paragraph with a link to the
     Color Spaces reference page once #5 ships. -->
The Basic-panel sliders below share a mental model (they all sit at the
"top of the stack" in Lightroom / Capture One), but their math divides
cleanly by color space. White balance and exposure run in **linear
light** before gamma encoding; the tone sliders run in **sRGB gamma
space** after. This split is one reason
`apply_white_balance_exposure_buffer` is a separate orchestrator from
the later `apply_per_pixel_adjustments` stage in the AgX code, even
though that later stage also includes tone curves, HSL, color grading,
and LUT work.

## White balance

{{#include ../../../../crates/agx/src/adjust/white_balance.md}}

## Exposure

{{#include ../../../../crates/agx/src/adjust/exposure.md}}

## Tone sliders

{{#include ../../../../crates/agx/src/adjust/basic_tone.md}}

## Related

- API references: [white balance](../api/agx/adjust/white_balance/index.html), [exposure](../api/agx/adjust/exposure/index.html), [basic tone](../api/agx/adjust/basic_tone/index.html)
- [HSL](hsl.md)
- [Tone curves](tone-curves.md)
