<!-- Canonical source: crates/agx/src/adjust/detail.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

The detail pass is AgX's three-slider neighborhood stage. It runs on
the sRGB gamma-space buffer after the per-pixel tone, HSL, color
grading, and LUT work, but before the final conversion back to linear
RGB. The pass covers sharpening, clarity, and texture with one common
idea: build a blurred version of the image, subtract it from the
original to isolate a frequency band, then add some of that band back
in. All three controls work on luminance only, so the code can change
local detail without pulling color channels apart and creating colored
halos.

## How it works

The shared machinery is a separable Gaussian blur plus a luminance-only
unsharp mask:

1. Convert each RGB pixel to luminance with the Rec. 709 weights stored
   in `adjust::LUMA_R`, `adjust::LUMA_G`, and `adjust::LUMA_B`.
2. Build a 1D Gaussian kernel whose half-width is `ceil(3 * sigma)`.
3. Blur horizontally, then vertically, clamping sample coordinates at
   the image edges.
4. Compute `high_freq = luminance - blurred_luminance`.
5. Add `strength * high_freq` back into R, G, and B equally.

That shared structure is what keeps the three sliders consistent. The
pass applies the controls sequentially to the evolving buffer in this
order: texture, then clarity, then sharpening. That ordering matters:
later sliders see the result of earlier ones instead of always sampling
the original image. The only thing that changes between the three
sub-passes is the blur scale and, for sharpening, the extra
threshold/masking gates.

### Texture

Texture targets fine detail. In the current implementation it uses a
fixed `sigma` of `3.0`, so it reacts to the highest-frequency visible
structure in the image: pores, fabric weave, leaf edges, and other small
surface variation. Positive values add that fine-scale contrast back in;
negative values soften it.

Because texture is just the shared unsharp-mask pipeline with a small
blur radius, it stays local. It does not try to infer semantic regions
or preserve edges differently from texture. It simply works at the
finest band the pass exposes.

### Clarity

Clarity uses the same unsharp-mask math, but with a much broader fixed
`sigma` of `20.0`. That moves the effect into the mid-frequency range:
larger local transitions, broad texture, and the kind of tonal
modulation that reads as "punch" rather than crisp edge sharpening.

Positive clarity strengthens that medium-scale contrast. Negative
clarity softens it. The slider is therefore symmetric in sign but not in
scale: it is a single frequency-band adjustment, not a separate contrast
system.

### Sharpening

Sharpening is the most controlled of the three. It uses the user-facing
`radius` slider as the Gaussian `sigma`, with a floor of `0.1` so the
kernel stays well-defined. That makes the control act on the lower end
of the fine-detail range: edge crispness, micro-contrast, and the
smallest recoverable structures.

Sharpening adds two gates on top of the basic unsharp mask:

- `threshold` removes low-magnitude high-frequency differences before
  they get amplified. In code, the slider is converted with
  `threshold / 255.0`, and pixels below that absolute luminance delta
  are left unchanged.
- `masking` computes a simple edge map from luminance gradients and
  uses `smoothstep` to limit sharpening to stronger edges. The edge map
  is normalized with the fixed `EDGE_SCALE = 4.0` constant so the
  slider behaves consistently across images.

The result is a conventional sharpening control with a little more
protection against noise and smooth-surface artifacts than a plain
unsharp mask.

## Why we chose it

The 2026-03-21 design chose a multi-scale unsharp-mask model because it
fits photo-editing expectations well: texture, clarity, and sharpening
map cleanly to increasing spatial frequency bands, and the behavior is
easy to reason about from preset values alone.

AgX also keeps the implementation conservative on purpose. Texture and
clarity use fixed blur scales, while sharpening keeps only one user
radius. That makes presets portable: a value means the same thing across
images instead of depending on a learned model or per-photo tuning.

The neutral case is equally important. `DetailParams::is_neutral()`
checks only the active effect fields - sharpening amount, clarity, and
texture. Radius, threshold, and masking are ignored when sharpening
amount is zero, so the pass can short-circuit completely when the detail
panel is effectively off.

## Parameters and constants

The public sliders live on `DetailParams` and `SharpeningParams`. The
rest of the numbers below are fixed in code.

| Control | Range | Default | Role |
|---------|-------|---------|------|
| `sharpening.amount` | `0.0..=100.0` | `0.0` | Sharpening strength |
| `sharpening.radius` | `0.5..=3.0` | `1.0` | Sharpening blur sigma |
| `sharpening.threshold` | `0.0..=100.0` | `25.0` | Hard cutoff for low-magnitude detail |
| `sharpening.masking` | `0.0..=100.0` | `0.0` | Edge-aware sharpening gate |
| `clarity` | `-100.0..=100.0` | `0.0` | Mid-frequency local contrast |
| `texture` | `-100.0..=100.0` | `0.0` | Fine-frequency local contrast |

| Constant | Value | Role |
|----------|-------|------|
| `TEXTURE_SIGMA` | `3.0` | Fixed blur scale for texture |
| `CLARITY_SIGMA` | `20.0` | Fixed blur scale for clarity |
| `SHARPEN_RADIUS_DEFAULT` | `1.0` | Default sharpening radius |
| `SHARPEN_THRESHOLD_DEFAULT` | `25.0` | Default sharpening threshold |
| `SHARPEN_RADIUS_MIN` / `MAX` | `0.5` / `3.0` | Sharpening radius bounds |
| `SHARPEN_THRESHOLD_MIN` / `MAX` | `0.0` / `100.0` | Sharpening threshold bounds |
| `SHARPEN_MASKING_MIN` / `MAX` | `0.0` / `100.0` | Sharpening masking bounds |
| `DETAIL_SLIDER_MIN` / `MAX` | `-100.0` / `100.0` | Clarity and texture bounds |
| `EDGE_SCALE` | `4.0` | Fixed gradient normalization for masking |

## Preset-slider mapping

In preset TOML, the detail pass lives under one `[detail]` block, with
sharpening nested underneath:

```toml
[detail]
clarity = 30.0
texture = 15.0

[detail.sharpening]
amount = 40.0
radius = 1.0
threshold = 25.0
masking = 50.0
```

That maps directly to `DetailParams` and `SharpeningParams`. Missing
fields fall back to the defaults above, and an entirely absent `[detail]`
section materializes as a neutral detail pass.

## Source

- **CPU (Rust):** [`crates/agx/src/adjust/detail.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/detail.rs)
- **GPU dispatcher:** [`crates/agx/src/engine/gpu/stages/detail.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/stages/detail.rs)
- **GPU WGSL shaders:**
  - [`detail_extract_lum.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/detail_extract_lum.wgsl)
  - [`blur_horizontal.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/blur_horizontal.wgsl)
  - [`blur_vertical.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/blur_vertical.wgsl)
  - [`detail_apply.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/detail_apply.wgsl)

The CPU path implements the full threshold and masking behavior. The
current GPU dispatcher runs the same three sequential passes, but it
sets `detail_masking = 0.0` today, so the masking gate is not yet part
of the GPU path.
