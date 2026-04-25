<!-- Canonical source: crates/agx/src/adjust/basic_tone.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->
<!-- This file is included into the bundled "Basic adjustments" mdbook
     page, so its top-level headings are ### (h3) to nest under the
     wrapper's `## Tone sliders` heading. -->

The basic-tone sliders — contrast, highlights, shadows, whites, blacks
— shape the brightness distribution of the image without re-introducing
hue shifts. Each slider runs as a small piecewise-linear curve targeted
at a specific part of the tone range, in sRGB gamma space so the
adjustments track perceptual brightness rather than physical light
energy.

### How it works

The five sliders all run in sRGB gamma space, after the image has been
white-balanced and exposure-corrected in linear light. Each slider
remaps a single channel value with a small piecewise-linear curve that
targets a specific part of the tone range.

#### Contrast

Contrast is the only truly global control here. The code pivots around
`0.5`, the midpoint of normalized sRGB values, and scales the distance
from that pivot:

```text
factor = (100 + contrast) / 100
output = clamp(0.5 + (input - 0.5) * factor, 0, 1)
```

Positive contrast pushes values away from the midpoint. Negative
contrast pulls them toward it.

#### Highlights

Highlights only affect values above `0.5`. The weight rises linearly
from `0` at `0.5` to `1` at `1.0`, so brighter pixels are affected more
than dimmer ones in the highlight band:

```text
weight = (input - 0.5) / 0.5
output = clamp(input + weight * highlights / 100 * 0.5, 0, 1)
```

This gives a soft, one-sided curve that leaves the lower half of the
range unchanged.

#### Shadows

Shadows mirror the highlight curve below `0.5`. The darker the pixel,
the larger the weight:

```text
weight = 1 - input / 0.5
output = clamp(input + weight * shadows / 100 * 0.5, 0, 1)
```

Values at or above midpoint are left alone, so the adjustment stays
localized to the dark half of the tone range.

#### Whites

Whites target only the upper quarter of the range. The curve is the
same idea as highlights, but it starts later and uses a narrower band:

```text
weight = (input - 0.75) / 0.25
output = clamp(input + weight * whites / 100 * 0.25, 0, 1)
```

This gives finer control over near-white detail without pushing
midtones as aggressively.

#### Blacks

Blacks are the lower-quarter counterpart to whites:

```text
weight = 1 - input / 0.25
output = clamp(input + weight * blacks / 100 * 0.25, 0, 1)
```

Only values below `0.25` are affected, so the control can lift or
crush deep shadows without changing the rest of the image much.

### Why we chose it

The five-slider model — contrast plus four band-localized controls —
matches the Lightroom Basic panel one-for-one, which is what most
preset authors expect when they think about "tone shaping." Splitting
"highlights" and "whites" (and similarly shadows / blacks) into two
overlapping sliders gives photographers fine control: highlights can
roll off the bright midtones while whites separately decide where the
brightest pixels sit. Folding them into a single curve would lose that
separation.

The implementation deliberately stays piecewise-linear and gamma-space
rather than reaching for a smooth global tone curve. That keeps each
slider's effect localized and predictable for batch-applied presets,
makes the math cheap on both CPU and GPU, and leaves global re-shaping
for the dedicated `tone_curves` stage downstream. Working in sRGB gamma
space is the standard choice for these sliders because it matches the
"perceptual brightness" mental model the controls are named for —
running them in linear light would make the slider behave differently
in shadows than in highlights.

The triggering thresholds (`0.5` for contrast/highlights/shadows, `0.75`
and `0.25` for whites/blacks) are calibrated to give Lightroom-shaped
slider feel: highlights and shadows each affect roughly half the
range, whites and blacks each affect roughly a quarter.

### Parameters and constants

| Name | Type | Range / value | Used by | Meaning |
| --- | --- | --- | --- | --- |
| `contrast` | slider | `-100..100` | Contrast | Global scale around the `0.5` midpoint. |
| `highlights` | slider | `-100..100` | Highlights | Positive values brighten, negative values darken. |
| `shadows` | slider | `-100..100` | Shadows | Positive values lift, negative values crush. |
| `whites` | slider | `-100..100` | Whites | Adjusts the top quarter of the range. |
| `blacks` | slider | `-100..100` | Blacks | Adjusts the bottom quarter of the range. |
| `0.0` | constant | neutral / floor check | All functions | Neutral value checks and lower clamp bound. |
| `0.25` | constant | quarter-range cutoff | Whites, Blacks | Boundary for the whites/blacks band. |
| `0.5` | constant | midpoint cutoff | Contrast, Highlights, Shadows | Midpoint pivot and half-range width. |
| `0.75` | constant | three-quarter cutoff | Whites | Start of the whites band. |
| `1.0` | constant | full-scale endpoint | Highlights, Shadows, Blacks | Upper bound of normalized channel space. |
| `100.0` | constant | percent scale | All functions | Converts slider percentages into fractional adjustments. |

**Beyond the expected range:** none of the basic-tone sliders are
preset-validated, so out-of-range values reach the algorithm directly.
Each per-channel adjustment is clamped to `[0, 1]` after the math, so
extreme positive contrast/highlights/whites push pixels to the upper
clamp (highlights blow out toward solid white) and extreme negative
values crush toward the lower clamp. Values past `±200` produce no
additional visible change because almost every pixel is already at the
clamp; the practical operating range is `±100`.

### Preset-slider mapping

The preset values map directly to the slider ranges used by the code:

| Slider | Input range | Curve shape |
| --- | --- | --- |
| Contrast | `-100..100` | Symmetric linear scaling around `0.5`. |
| Highlights | `-100..100` | One-sided ramp on the bright half of the range. |
| Shadows | `-100..100` | One-sided ramp on the dark half of the range. |
| Whites | `-100..100` | Narrow bright-end ramp, limited to the top quarter. |
| Blacks | `-100..100` | Narrow dark-end ramp, limited to the bottom quarter. |

All five sliders are direct numeric mappings; there is no hidden
non-linearity in the slider value itself. The curve shape comes from
the piecewise weights in the adjustment functions, which localize each
control to the tone band that photographers expect.

### Source

- **CPU (Rust):** [`crates/agx/src/adjust/basic_tone.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/basic_tone.rs)
- **GPU (WGSL):** [`crates/agx/src/shaders/gamma_adjustments.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/gamma_adjustments.wgsl) (the gamma-space stack also runs tone curves, HSL, and color grading inside the same shader).

The CPU and GPU implementations share the same per-slider piecewise
formulas; the GPU shader runs all five sliders inside the bundled
gamma-space pass.

### References

No canonical external paper applies — these are the conventional
Lightroom-style Basic-panel sliders. AgX-specific calibration notes
live inline in the source.
