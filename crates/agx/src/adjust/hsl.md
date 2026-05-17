<!-- Canonical source: crates/agx/src/adjust/hsl.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations listed in the Source section below. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

HSL adjustments let users target the same familiar color bands they see
in mainstream editors: red, orange, yellow, green, aqua, blue, purple,
and magenta. Each band can shift hue, saturation, and luminance
independently, so a preset can cool shadows, tame greens, or warm skin
tones without forcing the user into a global color cast.

## Working space

This stage runs in gamma-encoded Rec.2020 — the sRGB transfer curve
applied to Rec.2020 linear values — alongside basic tone, color
grading, tone curves, detail, grain, and vignette. The HSL palette
geometry (band centers, half-widths, cosine windows) keeps its
perceptual meaning because the gamma curve shape is unchanged from the
previous sRGB-only design; what's different is the wider gamut
underneath, so wide-gamut headroom from Display P3 or other wide-gamut
inputs survives the per-pixel math instead of being clamped at the
boundary. The final clamp to display gamut happens only at encode. The
`[0, 1]` clamp on the per-pixel RGB before entering the RGB↔HSL
conversion stays as a domain-safety guard, because the HSL palette is
only defined on that range.

## How it works

HSL runs in the gamma-space per-pixel stage, after the basic tone curve
controls and before color grading and the final LUT. That placement
matters: HSL is a perceptual color tool, so it belongs with the other
gamma-encoded adjustments instead of the linear-light exposure and
white balance work. On the GPU path, the shared gamma-adjustment stage
still dispatches, but the shader skips the HSL substep entirely when no
HSL channel is active.

The public data model is `HslChannels`, which stores one `HslChannel`
for each of the 8 bands:

| Band | Center hue | Half-width |
|------|------------|------------|
| Red | `0°` | `30°` |
| Orange | `30°` | `30°` |
| Yellow | `60°` | `30°` |
| Green | `120°` | `60°` |
| Aqua | `180°` | `60°` |
| Blue | `240°` | `30°` |
| Purple | `270°` | `30°` |
| Magenta | `330°` | `30°` |

The centers are not evenly spaced. The warm side gets tighter spacing
so the red, orange, and yellow bands can separate skin-tone work more
precisely, while the green and aqua bands get wider windows because the
gaps around them are larger. The half-width values are explicit
per-band radii in code, not one universal cutoff, so each band reaches
zero at its own configured edge.

Each `HslChannel` stores three user-facing controls:

- `hue` in degrees, from `-180` to `+180`
- `saturation` in percent, from `-100` to `+100`
- `luminance` in percent, from `-100` to `+100`

The implementation first converts the pixel from gamma Rec.2020 RGB
into HSL, then accumulates weighted per-band deltas:

```text
if pixel_saturation < 1e-4:
    return original RGB

for each band i:
    distance = hue_distance(pixel_hue, center[i])
    weight = cosine_weight(distance, half_width[i]) * pixel_saturation
    hue_delta += weight * hue_shift[i]
    sat_delta += weight * (saturation_shift[i] / 100)
    lum_delta += weight * (luminance_shift[i] / 100)

new_hue = wrap(pixel_hue + hue_delta, 0, 360)
new_sat = clamp(pixel_sat + sat_delta, 0, 1)
new_lum = clamp(pixel_lum + lum_delta, 0, 1)
```

The hue math uses the shortest arc on the color wheel. `hue_distance`
returns a value in `[0, 180]`, so a pixel near `350°` can still match
the red and magenta bands correctly instead of taking the long way
around the circle.

The saturation guard handles the case where hue is effectively
undefined. Near-gray pixels do not carry a stable hue signal, so the
code skips the pass entirely when saturation is almost zero. Even above
that cutoff, the per-band weight is scaled by the pixel saturation, so
low-chroma pixels fade toward neutrality instead of getting a strong
band-specific shove.

The final step converts the adjusted HSL value back to gamma Rec.2020
RGB. Hue wraps around 360 degrees, saturation and luminance stay
clamped to `[0, 1]`, and the RGB result returns to the rest of the
gamma-space pipeline.

The cosine window is the key targeting function. `cosine_weight` is
`1.0` at the band center and falls smoothly to `0.0` at the half-width.
That creates a soft bell-shaped response with zero slope at both ends,
so neighboring bands meet cleanly instead of stepping into each other.
The weight function is pluggable through `WeightFn`, but cosine is the
default because it gives a smooth, cheap, and predictable band mask.

## Why we chose it

The cosine window gives smoother behavior than a boxcar mask. A boxcar
would turn each band on at full strength until a hard cutoff, then drop
to zero instantly. That makes the boundary visible whenever a pixel sits
near two adjacent bands or when a user drags a slider across a hue
transition. Cosine weighting keeps the response continuous, so the same
pixel changes gradually as it moves through the overlap region.

The cosine curve also matches the mental model of an HSL panel better
than a rigid mask. Editors expect a color band to have a center of
maximum influence and a gradual falloff toward neighboring bands. That
is exactly what the cosine window expresses. The implementation pays a
tiny trigonometric cost per active band, but in return it avoids the
hard-edged transitions and banding artifacts that a boxcar introduces.

The shortest-arc hue distance is the other important choice. Hue is
cyclic, so red at `0°` and red at `360°` are the same color. Using the
wrapped distance keeps the band masks symmetric around the color wheel
and makes the red and magenta edges behave correctly at the wrap point.

The saturation guard is just as deliberate. HSL is only meaningful when
the pixel already has some chroma. If the input is nearly gray, the
implementation leaves it alone instead of inventing a hue from numerical
noise. That keeps neutral areas stable and avoids pushing highlight
noise into a colored tint.

## Parameters and constants

The public model is `HslChannels`, which contains eight `HslChannel`
values in band order. The internal constants below shape the targeting
math.

| Constant | Value | Role | Sensitivity |
|----------|-------|------|-------------|
| Channel count | `8` | Number of color bands exposed to the user | Defines the entire user surface — adding or removing bands would change the preset schema. |
| Band centers | `0° / 30° / 60° / 120° / 180° / 240° / 270° / 330°` (red / orange / yellow / green / aqua / blue / purple / magenta) | Hue angle each band targets | Shifts the perceived "what counts as orange" line; the chosen values match Lightroom's HSL layout so presets cross-port well. |
| Warm-band half-width | `30°` | Influence radius for red, orange, and yellow | Narrower would make warm-band edits more surgical but produce visible band transitions; wider blurs the distinction between adjacent warm bands. |
| Mid-band half-width | `60°` | Influence radius for green and aqua | Wider than warm/cool bands because the green-aqua region of the wheel is sparser; narrowing would leave gaps where pixels match no band strongly. |
| Cool-band half-width | `30°` | Influence radius for blue, purple, and magenta | Same trade-offs as the warm-band half-width. |
| Gray cutoff | `1e-4` | Skips pixels with effectively undefined hue | Deliberately tiny — only suppresses pixels whose hue signal is too weak to trust. Raising it visibly desaturates near-gray pixels under HSL adjustments. |

**Beyond the expected range:** HSL does **not** preset-validate the
per-channel hue / saturation / luminance shifts, so out-of-range values
reach the algorithm. Hue shifts are angles in degrees and wrap modulo
360°. Saturation and luminance shifts are percents that the
implementation maps to `±1.0` internally; values past `±100` accumulate
proportionally but the per-pixel result is clamped to `[0, 1]` after the
adjustment, so very large shifts saturate against the clamp instead of
producing more visible change.

## Preset-slider mapping

In preset TOML, HSL lives under one `[hsl]` block with one nested table
per band:

```toml
[hsl.red]
hue = 5.0
saturation = -15.0

[hsl.orange]
saturation = 10.0
luminance = 5.0

[hsl.green]
saturation = -40.0
```

Each nested table maps directly to `HslChannel { hue, saturation,
luminance }`. Missing fields default to `0.0`, so an omitted channel or
an omitted field inside a channel stays neutral. That keeps presets
compact and lets users touch only the bands they actually want to
change.

The units map literally:

- hue shifts are degrees around the color wheel
- saturation shifts are percentages that become `-1.0..=1.0` internally
- luminance shifts are percentages that become `-1.0..=1.0` internally

A `0.0` saturation shift leaves chroma unchanged, and a `0.0`
luminance shift leaves brightness unchanged. A channel only affects the
image when its own values are non-zero and the pixel hue falls inside
that channel's cosine window.

## Source

- **CPU (Rust):** [`crates/agx/src/adjust/hsl.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/hsl.rs)
- **GPU helper functions:** [`crates/agx/src/shaders/common/color.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/common/color.wgsl)
- **GPU dispatch path:** [`crates/agx/src/engine/gpu/stages/gamma_adjustments.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/stages/gamma_adjustments.rs)
- **GPU shader path:** [`crates/agx/src/shaders/gamma_adjustments.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/gamma_adjustments.wgsl)
- **GPU parameter upload:** [`crates/agx/src/engine/gpu/params.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/params.rs)

The CPU and GPU implementations use the same band order, the same
center hues, the same half-widths, and the same shortest-arc hue math.
The CPU code exposes `WeightFn` so the window function stays swappable,
while the current pipeline uses `cosine_weight` on both sides of the
renderer.

## References

No canonical external paper applies — eight-band HSL with cosine-window
hue selection is a conventional photo-editor formulation. AgX-specific
band centers, half-widths, and the shortest-arc hue math are recorded
inline in the source.
