# Effects

Effects are overlaid or added artifacts — they don't restore or correct anything in the underlying image, they layer something on top. AgX applies effects late in the pipeline so they aren't disturbed by earlier color or tone work. The Effects group covers two knobs: grain (added noise) and vignette (edge darkening or brightening).

## Grain

Simulates film grain by adding spatial noise to the image. Real film shows grain because silver halide crystals form in discrete clumps; AgX models the look without the chemistry. The effect is shadow-weighted — grain is more visible in dark areas, matching how real film behaves at higher ISOs.

See [Grain](../../explanation/algorithms/grain.md) for the full algorithm and the design history behind the choices.

## Vignette

Darkens (or brightens) the corners of the image relative to the center. Photographers use vignettes for two reasons: to correct lens fall-off (real lenses produce some natural darkening at the edges) and as a creative tool to draw the eye toward the center subject.

AgX's vignette is creative — symmetric, controllable in amount and falloff. Lens-correction vignetting (geometric, lens-profile-driven) is not part of the current pipeline.

---

See: [Grain](../../explanation/algorithms/grain.md) and [Vignette](../../explanation/algorithms/vignette.md) for the algorithm-level math behind these knobs.
