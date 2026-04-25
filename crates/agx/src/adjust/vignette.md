<!-- Canonical source: crates/agx/src/adjust/vignette.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Vignette darkens or brightens the image edges with a position-dependent
multiplicative mask anchored at the center. It is one of the simplest
adjustments in AgX but a staple of nearly every film-look preset because
it draws the viewer's eye toward the subject.

## How it works

The mask is built from the pixel's normalized distance to the image
center. For each pixel `(x, y)` in a `w × h` image, the algorithm
computes:

```text
dx = (x - half_w) * inv_x
dy = (y - half_h) * inv_y
d²  = dx² + dy²
base   = clamp(1 - d², 0, 1)
factor = base²
multiplier = 1 + strength * (1 - factor)
output_channel = clamp(input_channel * multiplier, 0, 1)
```

with `strength = amount / 100.0`. At the center, `factor = 1.0` and the
multiplier is exactly `1.0` (no change). As `factor` falls toward `0.0`
near the edges, the multiplier approaches `1.0 + strength`, so the
border darkens uniformly across RGB for negative `amount` and brightens
for positive `amount`. The trailing per-channel `clamp` handles the
small slice of values that would otherwise leave the displayable range.

The shape is chosen by the `inv_x`/`inv_y` precompute. Elliptical mode
uses `inv_x = 1 / half_w` and `inv_y = 1 / half_h`, so the four edge
midpoints reach `d² = 1` at the same time and the fall-off matches the
image aspect ratio. Circular mode uses a single radius — `R =
max(half_w, half_h)` — for both axes. On a non-square image that leaves
the short edges less affected at their midpoints (because they sit
closer to the center than `R`) and the corners more affected (because
they extend past the circle boundary; the `clamp` keeps `factor` from
going negative there).

The squaring `factor = base²` gives a soft fall-off with a slightly
stronger core than a linear `1 - d²` would, and avoids a hard ring at
the boundary. AgX hard-codes this curve rather than exposing it as a
slider — see "Why we chose it" below.

On the CPU path, `VignettePrecomputed::new` caches `half_w`,
`half_h`, the per-axis reciprocals, and the normalized strength once
per render. `apply_vignette_pre` reuses those cached values, so the
hot path is a handful of multiplies plus the clamp. The GPU shader
reproduces the same mask equation but recomputes the geometry terms per
invocation rather than sharing a struct.

## Why we chose it

This is a **creative vignette** (a stylistic effect applied late in the
pipeline), not a **lens-correction vignette** (undoing optical falloff
early). The two have different placements: lens correction runs on
linear-light data near the start of the pipeline, before tonal and
color adjustments, so it cancels a physical artifact before later math
amplifies it. Creative vignette runs in sRGB gamma space late in the
pipeline, after every tonal and color adjustment, so it shapes the
final perceptual image the way a darkroom dodge or a software wash
would. AgX puts this stage right before the final sRGB-to-linear
conversion, matching where Lightroom and Capture One place their
"Effects" vignette.

The two-parameter API (`amount`, `shape`) is deliberate. Lightroom and
Capture One expose midpoint, feather, roundness, and highlight
priority, but most preset authors only ever touch amount and shape, and
the additional parameters compound surprises in batch processing. The
hard-coded `factor = base²` curve gives a result that consistently reads
as "vignette" without tuning. If a preset library later demands
midpoint or feather, they can be added as opt-in `Option<f32>` fields
without breaking existing presets.

Two shape options cover the common cases. Elliptical falloff is the
right default because it darkens all four image edges evenly. Circular
falloff approximates real lens image-circle behavior — the lens
projects a disc onto the rectangular sensor, so the corners receive
less light than the centers of the long edges — and is useful for
recreating that look on already-corrected files. Off-center placement
is intentionally not supported; the effect is anchored to the image
midpoint.

## Parameters and constants

| Parameter / constant | Value | Role | Sensitivity |
|----------------------|-------|------|-------------|
| `amount` (preset) | `f32`, expected `-100.0..=+100.0`, default `0.0` | Strength. Negative darkens, positive brightens, `0.0` is identity (early-out). | Linear in the multiplier — `±50` halves or doubles the edge brightness; `±100` reaches `0.0` or `2.0` at the corner before clamping. Values outside the expected range extrapolate rather than error out. |
| `shape` (preset) | enum `Elliptical` (default) or `Circular` | Falloff geometry. | Elliptical = even edge darkening regardless of aspect ratio; Circular = stronger short-edge / corner effect on non-square images. |
| Falloff exponent | `2` (hardcoded — `factor = base * base`) | Smoothness of the radial transition. | Higher exponents push the effect toward the edges, leaving more of the center untouched; lower exponents spread the falloff inward. Not exposed because adjusting it in a preset rarely beats tweaking `amount`. |
| Circular radius rule | `R = max(half_w, half_h)` | Single radius for both axes in `Circular` mode. | Picking `min` instead would clip the corners exactly to the image; picking `max` is the convention that mimics real lenses. |

Output is per-channel-clamped to `[0.0, 1.0]` after multiplication so a
strong brightening or out-of-range upstream value cannot push a channel
past displayable bounds.

**Beyond the expected range:** vignette does not preset-validate
`amount`, so out-of-range values reach the algorithm directly. The
formula extrapolates linearly — `amount = 200` gives a corner
multiplier of `3.0` before clamping, `amount = -200` gives `-1.0`
(everything in the corner clamps to black). Values past `±100` quickly
saturate against the per-channel clamp and stop being visually
proportional. `shape` accepts only the two enum variants; anything else
fails preset parsing.

## Preset-slider mapping

```toml
[vignette]
amount = -30.0          # darkens edges
shape  = "circular"     # optional; defaults to "elliptical"
```

`amount` maps linearly to `strength`: `strength = amount / 100.0`. A
preset that omits `[vignette]` entirely, or sets `amount = 0.0`, takes
the early-out path in `apply_vignette` and skips the multiplication
loop. Preset composition (`merge`/`materialize`) treats the two fields
independently — a child preset can override `amount` without touching
`shape`.

## Source

- **CPU (Rust):** [`crates/agx/src/adjust/vignette.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/vignette.rs)
- **GPU (WGSL):** [`crates/agx/src/shaders/vignette.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/vignette.wgsl)
- **GPU dispatcher:** [`crates/agx/src/engine/gpu/stages/vignette.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/stages/vignette.rs)
- **Render-pipeline placement:** runs in sRGB gamma space immediately before the final sRGB-to-linear conversion in `engine::render`.

The CPU and GPU implementations follow the same mask equation. The CPU
path precomputes `VignettePrecomputed` once per render; the GPU path
recomputes the per-pixel geometry inline.

## References

No canonical external paper applies — this is a standard creative-vignette formulation rather than a published algorithm. The motivation and parameter choices are documented in [`docs/plans/2026-03-18-vignette-design.md`](https://github.com/zhjngli/AgX/blob/main/docs/plans/2026-03-18-vignette-design.md).
