<!-- Canonical source: crates/agx/src/adjust/white_balance.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->
<!-- This file is included into the bundled "Basic adjustments" mdbook
     page, so its top-level headings are ### (h3) to nest under the
     wrapper's `## White balance` heading. -->

White balance shifts the image's overall color cast — pulling it warmer
or cooler, more magenta or more green — by scaling the linear-light RGB
channels independently and re-normalizing so the average brightness
stays put. AgX exposes this as two sliders, `temperature` and `tint`,
both calibrated around `0.0` (no shift).

### How it works

The math runs on linear-light RGB values, before gamma encoding. Channel
scaling is only proportional to physical light energy when the data is
linear, so doing the work here keeps the adjustment physically
meaningful and preserves predictable behavior for downstream tone and
color stages.

The two slider inputs are mapped into per-channel multipliers:

```text
r_mult = 1 + temperature / 200
b_mult = 1 - temperature / 200
g_mult = 1 - tint / 200
```

Positive `temperature` boosts red and reduces blue, which warms the
image. Negative `temperature` does the opposite and cools it. Positive
`tint` reduces green, pulling the image toward magenta; negative `tint`
boosts green.

Those raw multipliers are then normalized so the adjustment preserves
the overall channel-average brightness:

```text
sum  = r_mult + g_mult + b_mult
norm = 3 / sum
output_channel = max(0, input_channel * channel_mult * norm)
```

The normalization rescales the three multipliers so they still average
to `1.0` over the slider's typical operating range. That keeps a neutral
gray from drifting brighter or darker when the user only wants to shift
color balance. It is a channel-average normalization, not a perceptual
luminance guarantee, so very strong shifts can still nudge mid-gray
appearance slightly. The trailing `max(0, …)` keeps a channel from
going negative when an extreme shift or invalid upstream value would
push it below zero.

### Why we chose it

Two design choices drive the implementation. First, **linear space**:
applying scalar multipliers to gamma-encoded values would make the
warming or cooling effect track display brightness instead of light
energy, so a "+50 warm" applied to a dark midtone would behave very
differently from the same shift applied to a highlight. Doing the math
in linear light makes the slider feel like a physical color-temperature
change.

Second, **per-channel multipliers with a brightness-preserving
normalization** rather than a full Bradford or CIECAM chromatic
adaptation transform. Bradford and friends give the most accurate
results when you know the source and target illuminants in detail, but
AgX's white-balance sliders are creative controls — the photographer
just wants the image warmer or cooler — and the linear scaling matches
how Lightroom's Temperature/Tint pair feels in practice while keeping
the algorithm a few lines of math rather than a 3×3 matrix and a
reference white.

The `200.0` denominator is the calibration that makes the slider feel
right. At `temperature = 100`, `r_mult = 1.5` and `b_mult = 0.5`,
which is a noticeable but not extreme warming shift. At `temperature
= 200`, blue would clamp to zero entirely, so the practical operating
range is `[-100, +100]`.

### Parameters and constants

| Parameter / constant | Value | Role | Sensitivity |
|----------------------|-------|------|-------------|
| `temperature` (preset) | `f32`, expected `-100.0..=+100.0`, default `0.0` | Warm/cool shift. | Linear in the per-channel multiplier. `±50` produces a 25% relative shift between red and blue; `±100` is the practical limit before blue clamps. Values outside the expected range extrapolate rather than error out. |
| `tint` (preset) | `f32`, expected `-100.0..=+100.0`, default `0.0` | Magenta/green shift. | Linear in the green-channel multiplier. Same calibration as `temperature`. |
| Calibration denominator | `200.0` | Maps slider value to `±0.5` per-channel multiplier swing. | Smaller denominators make sliders more sensitive; the chosen value matches the typical Lightroom feel. |
| Brightness normalization | `norm = 3 / (r_mult + g_mult + b_mult)` | Keeps channel-average brightness constant. | Without normalization, a "warm" shift would also brighten the image overall; with it, the user's shift is purely chromatic. |
| Channel floor | `0.0` | Prevents negative output. | Trailing `max` handles extreme shifts and out-of-range upstream values. |

**Beyond the expected range:** white balance does not preset-validate
`temperature` or `tint`, so out-of-range values reach the algorithm.
At `temperature = 200`, `b_mult = 0` and the blue channel collapses to
zero (image goes orange-magenta). Past that, multipliers go negative
and the trailing `max(0, …)` clamps each channel to zero, so very
large shifts produce a hard channel kill rather than continuing to
intensify. `tint` follows the same pattern with green. Values past
`±100` are not useful in practice.

### Preset-slider mapping

```toml
[white_balance]
temperature = 25.0   # warm by 25
tint        = -10.0  # slightly green
```

Both fields are direct numeric mappings — there is no hidden curve or
log scaling on the slider value. Preset composition (`merge` /
`materialize`) treats the two fields independently. A preset that omits
`[white_balance]` entirely, or sets both fields to `0.0`, takes the
early-out path in `apply_white_balance` and returns the input
untouched.

### Source

- **CPU (Rust):** [`crates/agx/src/adjust/white_balance.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/white_balance.rs)
- **CPU buffer orchestrator:** [`apply_white_balance_exposure_buffer`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/mod.rs) bundles white balance with exposure for a single buffer pass.
- **GPU (WGSL):** [`crates/agx/src/shaders/linear_adjustments.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/linear_adjustments.wgsl) (combined linear-space WB + exposure pass).

The CPU and GPU implementations share the formula above; the GPU shader
performs the multiply-and-normalize in a single compute dispatch over
the linear buffer.

### References

No canonical external paper applies — temperature/tint sliders backed by
linear-space channel multipliers are the conventional creative-WB
formulation in Lightroom-class editors. AgX-specific motivation is
recorded inline in the source comments.
