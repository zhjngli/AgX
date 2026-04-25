<!-- Canonical source: crates/agx/src/adjust/color_grading.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations listed in the Source section below. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Color grading blends three tonal wheels - shadows, midtones, and
highlights - plus a global wheel, using the same lift/gamma/gain mental
model that photographers already know from tools like DaVinci Resolve.
Each wheel can push hue, saturation, and luminance independently, so the
effect can stay subtle and neutral or move all the way into a stylized
split-tone look.

## How it works

The public data model is a `ColorWheel` for each tonal region. A wheel
stores:

- `hue` in degrees
- `saturation` as a percentage
- `luminance` as a signed brightness shift

The hue/saturation pair is treated as a polar representation of a tint.
During precomputation, AgX converts each wheel into an RGB multiplier by
sampling three cosine lobes spaced 120 degrees apart. That gives a
compact, stable way to turn one angle plus one radius into a neutral
`[1.0, 1.0, 1.0]` tint at zero saturation and a smooth color bias at
higher saturation. Luminance stays separate because it is an additive
offset, not a chroma rotation.

The implementation splits work into a precompute phase and a per-pixel
hot path. `apply_color_grading_pre` is the hot-path entry point, but it
relies on a `ColorGradingPrecomputed` struct built once per render. That
precompute step does the expensive, loop-invariant work:

- convert each wheel from hue/saturation into an RGB tint
- normalize each wheel's luminance shift into `[-1.0, 1.0]`
- compute the balance exponent — `2.0` raised to the power of `-balance / 100` (in code, `2.0_f32.powf(-balance / 100.0)`; not bitwise XOR)
- cache whether balance is active at all

That keeps the inner loop free of repeated trig and `powf` work when the
effect is active. When the parameters are neutral, the CPU and GPU paths
still run their shared per-pixel gamma-adjustment stage, but they skip
the color-grading substep inside that stage.

Per pixel, the algorithm first measures luminance in sRGB gamma space
with the Rec. 709 coefficients:

`lum = 0.2126*r + 0.7152*g + 0.0722*b`

This is a perceptual proxy, not a linear-light measurement. That choice
matches the rest of the gamma-space adjustment stack and makes the
region weights feel closer to what an editor user expects to see.

The balance slider shifts where the tonal crossover lands. With a
neutral balance, the pixel luminance passes straight into the mask
curves. When balance moves negative or positive, the code remaps the
luminance with a power curve before weighting the zones. Negative
balance expands the shadow region; positive balance gives the highlights
more room.

The three zone masks are smooth, overlapping boundaries rather than hard
cutoffs. In code they are quadratic crossfades:

```text
w_shadow = (1 - lum_adj)^2
w_highlight = lum_adj^2
w_midtone = 1 - w_shadow - w_highlight
```

Those masks always sum to 1.0, so the three tonal regions stay
complementary. Near black, the shadow wheel dominates; near white, the
highlight wheel dominates; and the midtone wheel fills the overlap in
between. This behaves like a smoothstep-style transition even though the
implementation uses squared ramps rather than a literal `smoothstep`
call.

Once the weights are known, AgX blends the shadow, midtone, and
highlight RGB tints into one regional tint, then multiplies that by the
global wheel. The per-pixel color change is a channel-wise multiply
followed by an additive luminance shift:

```text
regional_tint = shadow_tint*w_shadow + midtone_tint*w_midtone + highlight_tint*w_highlight
combined_tint = regional_tint * global_tint

out = clamp(pixel * combined_tint, 0.0, 1.0)
out += shadow_lum*w_shadow + midtone_lum*w_midtone + highlight_lum*w_highlight + global_lum
```

The order matters. The tint multiply applies the color cast first, and
the luminance offset rides on top of it afterward. That keeps the
slider behavior close to a classic grading wheel: hue and saturation
change the color balance, while luminance pushes the tonal weight of
that region brighter or darker.

The three wheels map naturally onto the lift/gamma/gain vocabulary:

- shadows behaves like lift
- midtones behaves like gamma
- highlights behaves like gain

The global wheel is separate because it acts as a uniform finishing trim
on the whole image instead of one specific tonal region. That extra
global control is useful when the regional wheels establish the look but
the whole frame still needs a small overall color bias.

## Why we chose it

This model matches the way users already think about grading. The three
zone wheels give a direct path from a creative intention - cooler
shadows, warmer highlights, cleaner midtones - to a preset parameter
set. The global wheel adds the final broad correction without forcing
the user to rebalance every region by hand.

The cosine-based hue representation is also a good fit for this
problem. It is cheap to compute, it wraps cleanly around the color
circle, and it gives a smooth transition from neutral to strongly
tinted without needing a larger color model or a lookup table. In
practice, the wheel behaves like an intuitive angle-plus-strength dial
rather than a fragile RGB recipe.

The luminance masks deliberately stay soft. Hard region boundaries would
make the effect visible as bands or halos whenever the user pushes the
wheels hard. The smooth overlap keeps the tonal zones continuous, so the
image can cross from shadow to midtone to highlight without an obvious
seam.

The precompute split is the other important choice. Hue-to-RGB
conversion and balance setup are invariant across the image, so they
belong outside the pixel loop. `apply_color_grading_pre` keeps the hot
path lean while still using the same math on CPU and GPU. That makes the
rendering cost predictable and keeps the implementation easy to mirror
in shader code.

## Parameters and constants

The public model is `ColorGradingParams`, which contains four wheels and
one balance slider. The internal constants below shape how the math
behaves.

| Constant | Value | Role | Sensitivity |
|----------|-------|------|-------------|
| Hue units | degrees | User-facing hue angle for each wheel | Wraps modulo 360°; out-of-range hues land at the equivalent angle. |
| Saturation units | percent | Strength of the tint derived from each hue angle | Scales the tint before mixing; doubling has near-doubled visible effect when other controls are neutral. |
| Luminance units | `-100` to `+100` (informal — no runtime range validation; the per-pixel luminance result is clamped after the adjustment) | Additive brightness shift for each wheel | At small values feels like "lift the shadows by N%"; large values quickly saturate against the per-pixel `[0, 1]` clamp and stop being visually proportional. |
| Balance units | `-100` to `+100` | Shifts the shadow/highlight crossover | The whole feel of the wheel weighting; ±50 is a strong shift, ±100 collapses one zone almost entirely. |
| Rec. 709 luma coefficients | `0.2126`, `0.7152`, `0.0722` | Gamma-space luminance proxy used for zone weighting | Standard Rec. 709 — changing them shifts which pixels count as shadows vs highlights and changes the feel of the entire tool. |
| Hue-lobe spacing | `120°` | Separates the RGB cosine lobes used to form the tint | Tied to RGB channel symmetry; changing it would distort the tint conversion and break expected channel balance. |
| Balance exponent | `2.0` raised to the power of `-balance / 100` | Remaps luminance before the zone weights are computed | The exponential shape gives a smooth crossover as the user drags the slider; a linear remap would feel abrupt at the extremes. |

**Beyond the expected range:** color grading does **not** preset-validate
its slider values, so out-of-range numbers reach the algorithm directly.
Per-pixel luminance is clamped to `[0, 1]` after the adjustment, so
pushing a wheel's `luminance` past `±100` mostly saturates against that
clamp rather than producing larger visible change. Hue values wrap
modulo 360°. Saturation behaves as a multiplier — values above `100`
just amplify the tint proportionally.

## Preset-slider mapping

Preset TOML uses one root `[color_grading]` table plus four nested
wheel tables:

```toml
[color_grading]
balance = -10.0

[color_grading.shadows]
hue = 200.0
saturation = 30.0
luminance = -5.0

[color_grading.midtones]
hue = 45.0
saturation = 10.0

[color_grading.highlights]
hue = 30.0
saturation = 25.0

[color_grading.global]
hue = 15.0
saturation = 5.0
```

Each wheel field maps directly to `ColorWheel { hue, saturation,
luminance }`. Missing fields stay neutral, so an untouched wheel still
acts like a no-op. That keeps presets compact and lets users store only
the parts of a grade they actually care about.

The mapping is intentionally literal:

- `shadows` controls the lift-like region
- `midtones` controls the gamma-like region
- `highlights` controls the gain-like region
- `global` applies the same tint and luminance bias everywhere
- `balance` moves the tonal crossover point between shadow and highlight

A saturation of zero is neutral regardless of hue, and a luminance of
zero leaves that wheel's brightness contribution unchanged. That makes
it easy to keep a wheel present in a preset without forcing it to
affect the image.

## Source

- **CPU (Rust):** [`crates/agx/src/adjust/color_grading.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/color_grading.rs)
- **CPU render-stage hookup:** [`crates/agx/src/engine/stages/per_pixel.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/stages/per_pixel.rs)
- **GPU upload path:** [`crates/agx/src/engine/gpu/params.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/params.rs) and [`crates/agx/src/engine/gpu/mod.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/mod.rs)
- **GPU dispatch path:** [`crates/agx/src/engine/gpu/stages/gamma_adjustments.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/stages/gamma_adjustments.rs)
- **GPU shader path:** [`crates/agx/src/shaders/gamma_adjustments.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/gamma_adjustments.wgsl)

The CPU and GPU implementations follow the same math. The CPU version
precomputes wheel tints and balance data once per render, and the GPU
path uploads the same derived values into storage buffers before running
the per-pixel grading pass.

## References

No canonical external paper applies — three-way lift/gamma/gain color
grading is a long-standing convention in colorist tooling rather than a
published algorithm. AgX-specific calibration (Rec. 709 luma proxy,
120° hue-lobe spacing, exponential balance) is recorded inline in the
source.
