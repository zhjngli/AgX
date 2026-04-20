<!-- Canonical source: crates/agx/src/adjust/basic_tone.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

## How it works

Basic tone works in sRGB gamma space, after the image has already been
decoded and white balance / exposure have run in linear light. Each
slider remaps a single channel value with a small piecewise-linear curve
that targets a specific part of the tone range.

### Contrast

Contrast is the only truly global control here. The code pivots around
`0.5`, the midpoint of normalized sRGB values, and scales the distance
from that pivot:

```text
factor = (100 + contrast) / 100
output = clamp(0.5 + (input - 0.5) * factor, 0, 1)
```

Positive contrast pushes values away from the midpoint. Negative
contrast pulls them toward it.

### Highlights

Highlights only affect values above `0.5`. The weight rises linearly
from `0` at `0.5` to `1` at `1.0`, so brighter pixels are affected more
than dimmer ones in the highlight band:

```text
weight = (input - 0.5) / 0.5
output = clamp(input + weight * highlights / 100 * 0.5, 0, 1)
```

This gives a soft, one-sided curve that leaves the lower half of the
range unchanged.

### Shadows

Shadows mirror the highlight curve below `0.5`. The darker the pixel,
the larger the weight:

```text
weight = 1 - input / 0.5
output = clamp(input + weight * shadows / 100 * 0.5, 0, 1)
```

Values at or above midpoint are left alone, so the adjustment stays
localized to the dark half of the tone range.

### Whites

Whites target only the upper quarter of the range. The curve is the same
idea as highlights, but it starts later and uses a narrower band:

```text
weight = (input - 0.75) / 0.25
output = clamp(input + weight * whites / 100 * 0.25, 0, 1)
```

This gives finer control over near-white detail without pushing midtones
as aggressively.

### Blacks

Blacks are the lower-quarter counterpart to whites:

```text
weight = 1 - input / 0.25
output = clamp(input + weight * blacks / 100 * 0.25, 0, 1)
```

Only values below `0.25` are affected, so the control can lift or crush
deep shadows without changing the rest of the image much.

## Parameters and constants

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

## Preset-slider mapping

The preset values map directly to the slider ranges used by the code:

| Slider | Input range | Curve shape |
| --- | --- | --- |
| Contrast | `-100..100` | Symmetric linear scaling around `0.5`. |
| Highlights | `-100..100` | One-sided ramp on the bright half of the range. |
| Shadows | `-100..100` | One-sided ramp on the dark half of the range. |
| Whites | `-100..100` | Narrow bright-end ramp, limited to the top quarter. |
| Blacks | `-100..100` | Narrow dark-end ramp, limited to the bottom quarter. |

All five sliders are direct numeric mappings; there is no hidden
nonlinearity in the slider value itself. The curve shape comes from the
piecewise weights in the adjustment functions, which localize each
control to the tone band that photographers expect.
