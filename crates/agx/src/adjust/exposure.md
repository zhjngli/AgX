<!-- Canonical source: crates/agx/src/adjust/exposure.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->
<!-- This file is included into the bundled "Basic adjustments" mdbook
     page, so its top-level headings are ### (h3) to nest under the
     wrapper's `## Exposure` heading. -->

Exposure scales linear-light pixel values by a power-of-two factor so
that the slider value reads as photographic stops: `+1` brightens by a
factor of two, `-1` halves the light, `0` leaves the image alone. AgX
applies this in linear space before gamma encoding so the math behaves
like a real exposure change.

### How it works

The slider value (`stops`) is converted into a multiplier and applied
per channel:

```text
factor          = 2^stops
output_channel  = max(0, input_channel * factor)
```

The multiplier is always positive because it comes from a power of two,
so the trailing `max(0, …)` only matters when an upstream value would
otherwise be negative — the clamp keeps the output well-defined even
for invalid input.

The expected slider feel:

- `0` stops → multiplier `1.0`, no change
- `+1` stop → `2.0`, twice as bright
- `-1` stop → `0.5`, half as bright
- `+2` stops → `4.0`, four times as bright

Working in linear space matters. Stops are a *ratio of light energy*,
not a ratio of display-encoded brightness, so the multiplier only
behaves photographically when it lands on linear pixel values. Applied
after gamma encoding the same multiplier would skew midtones and
clip the highlights asymmetrically, no longer matching how a camera's
exposure dial behaves.

### Why we chose it

The whole adjustment is a single multiply, which is exactly the level of
machinery the operation deserves. Some editors expose exposure as a log
slider with a hidden non-linearity; AgX keeps it as a literal `2^stops`
so a preset author can reason about a "+0.5 stop" lift the same way they
reason about a half-stop in a camera.

The pipeline placement is the other choice. Exposure runs in linear
space alongside white balance, before sRGB encoding. This means the
later tone sliders, HSL, and color grading all see the
exposure-adjusted image — which is what photographers expect:
"correct" exposure first, then shape the look on top.

### Parameters and constants

| Parameter / constant | Value | Role | Sensitivity |
|----------------------|-------|------|-------------|
| `exposure` (preset) | `f32`, in stops, default `0.0` | Brightness shift. | Each unit doubles or halves the light. `+0.5` ≈ 41% brighter, `-0.5` ≈ 29% darker. No hard upper bound, but very large values quickly push everything past 1.0 in linear space — the highlight tail is then handled by downstream clamping or tone shaping. |
| Power base | `2.0` | Photographic-stop semantics. | Hard-coded; switching to a different base would break the "stops" mental model. |
| Channel floor | `0.0` | Guards against negative output from invalid upstream values. | Multiplier is always positive, so this only matters for malformed input. |

### Preset-slider mapping

```toml
[tone]
exposure = 0.5   # +½ stop
```

The `exposure` field maps directly to `stops` — no hidden non-linearity
on the slider value. Preset composition merges this field independently
of the other tone sliders. A preset that omits `[tone]` or sets
`exposure = 0.0` leaves the image unchanged at this stage.

### Source

- **CPU (Rust):** [`crates/agx/src/adjust/exposure.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/exposure.rs)
- **CPU buffer orchestrator:** [`apply_white_balance_exposure_buffer`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/mod.rs) bundles exposure with white balance for a single buffer pass.
- **GPU (WGSL):** [`crates/agx/src/shaders/linear_adjustments.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/linear_adjustments.wgsl) (combined linear-space WB + exposure pass).

The CPU and GPU implementations share the same `2^stops` multiplier and
linear-space placement.

### References

No canonical external paper applies — `2^stops` is the standard
photographic exposure formulation. The pipeline placement and slider
range are documented inline in the source.
