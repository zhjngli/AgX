<!-- Canonical source: crates/agx/src/adjust/tone_curves.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations listed in the Source section below. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Tone curves remap RGB values through five curve slots: a master RGB
curve, a luminance curve, and separate red, green, and blue curves.
The master curve shapes overall contrast and tonal rolloff. The
per-channel curves let the user push color relationships directly.
The luminance curve changes brightness while trying to preserve color
ratios. Together they cover the classic tone-curve workflow without
forcing the user into one fixed interpretation of "tone."

## How it works

Each curve is defined by control points in normalized coordinates:
`(x, y)` pairs in `[0.0, 1.0]`, sorted by `x`, with the first point at
`x = 0.0` and the last at `x = 1.0`. The default curve is the identity
line `[(0.0, 0.0), (1.0, 1.0)]`, so an untouched curve has no effect.

AgX converts each non-identity curve into a 256-entry lookup table once
per render. The table samples the curve at `x = i / 255.0` for
`i = 0..=255`, so the endpoints are represented exactly and the middle
of the curve is dense enough for smooth tonal work. Per-pixel evaluation
then reads from the table with linear interpolation between adjacent
entries. That keeps the hot path small, deterministic, and cheap to
share between CPU and GPU code.

The interpolation itself uses monotone cubic Hermite splines with the
Fritsch-Carlson tangent limiter.[^fritsch-carlson] This matters because
tone curves are supposed to be editing tools, not surprise generators.
Regular cubic splines can overshoot between control points, which can
create false reversals: a region the user intended to brighten can dip
dark again, or vice versa. Monotone cubic Hermite interpolation keeps a
monotone control polygon monotone in the interpolated result, so the
curve stays faithful to the points the user actually set.

The implementation follows the standard Fritsch-Carlson procedure:

1. Compute the secant slopes between adjacent control points.
2. Use the adjacent slopes to seed tangents at each control point.
3. Clamp the tangents with the Fritsch-Carlson monotonicity test.
4. Evaluate the Hermite basis functions at each of the 256 sample
   positions.

For a two-point curve, the code falls back to straight linear
interpolation. That path is both exact and simpler than forcing the
general Hermite machinery to do the same job.

Once the LUTs are built, pixel application runs in three stages and the
order matters:

1. **Master RGB curve** - look up `r`, `g`, and `b` independently in
   the master LUT. This sets the broad tonal shape first.
2. **Per-channel curves** - apply the red, green, and blue LUTs to
   their matching channels. This is where color grading moves happen:
   channel compression, split-toned bias, and cross-process style shifts.
3. **Luminance curve** - compute luminance with the same Rec. 709
   coefficients used elsewhere in gamma-space color math, map that
   luminance through the luma LUT, then scale all channels by the same
   factor:

   `scale = l_new / l`

   That proportional scaling changes brightness while preserving color
   ratios. If the luminance is near zero, the code skips the division
   and sets all three channels to the mapped luminance instead. That
   fallback avoids a divide-by-zero and turns lifted-black cases into a
   stable neutral gray instead of noisy color fringes.

The per-channel scaling step is clamped back into `[0.0, 1.0]` after
multiplication. That keeps the adjustment bounded even when a curve
pushes the mapped luminance upward or downward aggressively.

The CPU path caches each non-identity curve in an `Option<[f32; 256]>`
inside `ToneCurvePrecomputed`. Identity curves stay as `None`, so the
renderer can skip the lookup entirely when a channel is neutral. The GPU
path uses the same five 256-entry curves packed contiguously in upload
memory, so both execution paths share the same sampled transfer
function.

## Why we chose it

Monotone cubic Hermite interpolation gives the best balance of fidelity,
predictability, and implementation cost for tone curves. A piecewise
linear curve would avoid overshoot, but it would look too angular once a
curve has more than a couple of points. A standard cubic spline looks
smoother, but its overshoot can invent tonal behavior the user never
specified. The monotone Hermite variant keeps the smoothness of a cubic
while refusing to create new extrema between points, which is exactly
what a tone-curve editor needs.

The 256-entry LUT is the other half of that choice. It turns spline
evaluation into a small table lookup that is easy to reuse across the
CPU and shader code paths. Linear interpolation between adjacent LUT
entries keeps the table compact without making the result visibly
steppy. In practice, the LUT is a cache of the curve, not a lower-quality
approximation of it.

The luminance path uses proportional scaling instead of a separate RGB
reconstruction because it preserves chroma better than remapping each
channel independently. That keeps the luminance curve useful for global
brightness shaping while leaving the earlier master and per-channel
curves in control of color intent.

## Parameters and constants

The public model is `ToneCurveParams`, which contains five independent
`ToneCurve` values: `rgb`, `luma`, `red`, `green`, and `blue`. The
internal constants below shape interpolation and lookup behavior.

| Constant | Value | Role | Sensitivity |
|----------|-------|------|-------------|
| LUT size | `256` | Number of cached samples per curve | Enough for smooth 8-bit and 10-bit output; doubling it would barely change visible quality but doubles upload cost. Halving introduces visible stair-stepping in steep curves. |
| LUT sample step | `1 / 255` | Maps table indices to normalized curve space | Pure index math; tied to LUT size. |
| Fritsch-Carlson limiter threshold | `9.0` | Clamps tangent pairs when `alpha^2 + beta^2` is too large | The standard Fritsch-Carlson bound. Lowering it flattens the curve and removes overshoot at the cost of expressiveness; raising it lets users sketch curves that overshoot near tight control-point clusters. |
| Near-zero luminance guard | `1e-6` | Switches to gray fallback instead of proportional scaling | Deliberately tiny — only triggers near pure black where proportional scaling becomes numerically unstable. Raising it would visibly desaturate dark midtones; lowering it risks NaN-ish artifacts. |
| Zero-length segment guard | `1e-9` | Avoids division by zero for degenerate or repeated x spacing | Defensive. Public validation already rejects non-increasing x values; the guard only matters if a malformed curve slips through. |
| Rec. 709 luma coefficients | `0.2126`, `0.7152`, `0.0722` | Luminance weights shared with the rest of gamma-space color math | Standard Rec. 709 — changing them shifts which pixels register as bright vs dark across every luma-aware adjustment. |
| Output clamp | `[0.0, 1.0]` | Keeps sampled and scaled values in the public normalized range | Hard clamp at the boundary; not a tuning knob. |

**Beyond the expected range:** the public `ToneCurve::validate()` path
rejects control points outside `[0, 1]` and any non-monotonic x
sequence. Curves that pass validation but produce y values outside
`[0, 1]` after Fritsch-Carlson interpolation are clamped at lookup
time, so out-of-range curves cannot push pixels past valid linear RGB.
The internal constants above are not user-addressable.

## Preset-slider mapping

Tone curves are serialized as per-channel point lists in preset TOML.
Each curve maps directly to the matching field in `ToneCurveParams`:

```toml
[tone_curve.rgb]
points = [[0.0, 0.0], [0.25, 0.20], [0.75, 0.85], [1.0, 1.0]]

[tone_curve.luma]
points = [[0.0, 0.0], [0.5, 0.6], [1.0, 1.0]]

[tone_curve.red]
points = [[0.0, 0.0], [0.5, 0.55], [1.0, 1.0]]
```

Missing curve sections mean identity for that channel. That keeps
presets concise: users only serialize the curves they actually touch,
and untouched slots stay neutral.

Validation follows the code in `ToneCurve::validate()`:

- At least two points are required.
- The first point must start at `x = 0.0`.
- The last point must end at `x = 1.0`.
- `x` must increase strictly from point to point.
- Both coordinates must stay in `[0.0, 1.0]`.

Those rules make the preset format predictable and keep the
interpolator's assumptions intact.

## Source

- **CPU (Rust):** [`crates/agx/src/adjust/tone_curves.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/tone_curves.rs)
- **GPU lookup path:** [`crates/agx/src/shaders/gamma_adjustments.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/gamma_adjustments.wgsl)
- **GPU upload path:** [`crates/agx/src/engine/gpu/mod.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/mod.rs), [`crates/agx/src/engine/gpu/runtime.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/runtime.rs), and [`crates/agx/src/engine/gpu/params.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/params.rs)
- **GPU shared helpers:** [`crates/agx/src/shaders/common/tone.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/common/tone.wgsl)

The CPU and GPU paths share the same five curve slots and the same
256-sample layout. The CPU precomputes the table values with
Fritsch-Carlson interpolation; the GPU consumes the uploaded data and
does the same linear-in-LUT lookup at render time.

## References

[^fritsch-carlson]: F. N. Fritsch and R. E. Carlson (1980). *Monotone Piecewise Cubic Interpolation.* SIAM J. Numer. Anal. 17(2): 238–246. DOI: <https://doi.org/10.1137/0717021>.
