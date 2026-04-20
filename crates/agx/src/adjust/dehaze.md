<!-- Canonical source: crates/agx/src/adjust/dehaze.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations listed in the Source section below. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Haze lowers contrast because distant scene light is attenuated on the
way to the camera and mixed with a veil of atmospheric light. AgX's
dehaze pass models that veil explicitly: positive values estimate how
much airlight was added and subtract it back out, while negative values
do the inverse and re-introduce haze. That negative path matters in
practice because it gives preset authors a scene-aware way to pair a
softer atmospheric look with stronger contrast, clarity, or color
treatments, instead of settling for a flatter-looking image.

## How it works

The implementation follows the Dark Channel Prior pipeline from He, Sun,
and Tang[^dcp], but swaps the original soft-matting refinement for a
guided filter in the later He, Sun, and Tang formulation[^guided]. In
AgX the pass runs on linear RGB data after white balance and exposure,
so the haze model operates on physically meaningful intensities before
the later gamma-space tonal work.

The haze model is:

`I(x) = J(x) * t(x) + A * (1 - t(x))`

where `I` is the observed hazy image, `J` is the recovered scene
radiance, `A` is the global atmospheric light color, and `t(x)` is the
per-pixel transmission. The core idea behind the dark channel prior is
that, in most non-sky outdoor patches, at least one RGB channel gets
very close to zero somewhere in the patch. Haze lifts those dark values
toward the airlight color, so the local minimum becomes a useful haze
estimate.

AgX computes that estimate in five stages for positive values:

```text
dark = dark_channel(I)
A = brightest original pixel among top 0.1% of dark-channel values

if amount < 0:
    strength = clamp(-amount / 100, 0, 1)
    return clamp(I * (1 - strength) + A * strength, 0, 1)

dc_norm = dark_channel(I / max(A, 0.01))
t_raw = 1 - omega * dc_norm
guide = luma(I)
t = guided_filter(guide, t_raw)
J = clamp((I - A) / max(t, 0.1) + A, 0, 1)
```

Stage by stage:

1. `dark_channel()` takes the per-pixel `min(R, G, B)`, then applies a
   separable `15 x 15` min filter. The Rust path uses an O(n) monotonic
   deque in `min_filter_1d()` for each row and column; the GPU path
   splits the same work across `dehaze_pixel_min.wgsl` and
   `dehaze_min_filter.wgsl`.
2. `estimate_airlight()` looks at the top `0.1%` brightest values in the
   dark channel and, among those candidate pixels, picks the original
   RGB pixel with the highest `r + g + b`. That gives a scene-specific
   airlight color instead of assuming neutral gray.
3. For positive dehaze only, the image is normalized by `A` and clamped
   to a minimum denominator of `0.01` per channel to avoid unstable
   division. The normalized dark channel drives the raw transmission
   estimate `t_raw = 1 - omega * dc_norm`, where `omega = amount / 100`.
4. `guided_filter()` refines `t_raw` with a grayscale guide derived from
   Rec.709 luminance. The filter computes local linear coefficients
   `a` and `b`, box-filters them, and reconstructs
   `t_refined = mean(a) * guide + mean(b)`. This removes the blockiness
   from the patch min filter without washing transmission across hard
   edges.
5. The recovery step applies
   `J = (I - A) / max(t, 0.1) + A`, then clamps each channel back to
   `[0, 1]`. The `0.1` floor prevents very small transmission values
   from exploding noise and halos in dense haze.

Negative values reuse only the first two stages. AgX still estimates the
dark channel and atmospheric light so the added haze is colored by the
image's own airlight, then skips transmission estimation and guided
filtering entirely and blends linearly toward `A`. That is why negative
dehaze behaves like a scene-aware fog control rather than a generic gray
overlay.

## Why we chose it

Dark Channel Prior is a good fit for AgX's constraints: one slider, no
per-image tuning, strong results on the outdoor scenes where users
expect dehaze to help, and a physically interpretable negative mode
that can add haze as well as remove it. AgX intentionally keeps the
user model simple. There is only one public strength control; patch
size, transmission floor, guided filter radius, and the airlight
percentile stay fixed so presets remain portable and predictable.

The same design also chose guided filtering over the original
soft-matting refinement. Soft matting is higher ceremony and more
expensive for this use case; guided filtering gives the edge-aware
transmission cleanup the algorithm needs while staying O(N) and much
easier to implement consistently on CPU and GPU.

AgX also keeps the later performance work separate from the image model.
Dehaze had become the main CPU bottleneck at large resolutions, so the
implementation parallelizes the row and column passes of the separable
min and box filters and the embarrassingly parallel pixel loops. The
important point for this explanation is that those throughput changes do
not change the math: they only change how the same dark-channel,
guided-filter, and recovery steps are scheduled.

## Parameters and constants

The user-facing control is `DehazeParams.amount`. Everything below is
fixed in code.

| Constant | Value | Role | Sensitivity |
|----------|-------|------|-------------|
| `DEHAZE_AMOUNT_MIN` | `-100.0` | Lowest accepted slider value | None |
| `DEHAZE_AMOUNT_MAX` | `100.0` | Highest accepted slider value | None |
| Neutral amount | `0.0` | Skips the pass entirely via `is_neutral()` | None |
| `PATCH_SIZE` | `15` | Dark-channel patch width and height | High |
| `AIRLIGHT_PERCENTILE` | `0.001` | Top `0.1%` of dark-channel samples used for airlight candidates | Medium |
| `GUIDED_FILTER_RADIUS` | `40` | Radius of the guided filter box windows | High |
| `GUIDED_FILTER_EPSILON` | `0.001` | Regularizer for the guided filter coefficients | High |
| Airlight denominator floor | `0.01` | Minimum `A` component when normalizing `I / A` | Medium |
| `T_MIN` | `0.1` | Minimum transmission during recovery | High |
| Output clamp | `[0.0, 1.0]` | Keeps recovered pixels in valid linear RGB range | None |
| `omega` / fog strength ceiling | `1.0` | Caps `amount / 100` and `-amount / 100` | None |
| Rayon chunk size | `1024` | Work scheduling chunk for parallel pixel loops | Low |

The constants have different jobs:

- `PATCH_SIZE = 15` is the standard dark-channel neighborhood from the
  original method. Smaller patches follow local detail more tightly but
  make transmission noisier; larger patches smooth more aggressively and
  can miss fine haze transitions.
- `AIRLIGHT_PERCENTILE = 0.001` is deliberately tiny so the airlight is
  chosen from the haziest-looking pixels rather than from ordinary
  bright surfaces. Raising it risks picking sunlit objects; lowering it
  makes the estimate brittle on small images.
- `GUIDED_FILTER_RADIUS = 40` is much larger than the min-filter half
  width, which is the point: it smooths over the patch artifacts while
  still respecting edges from the luminance guide.
- `GUIDED_FILTER_EPSILON = 0.001` controls how strongly the guide
  constrains the result. Larger values smooth harder and flatten local
  contrast; smaller values hug edges more tightly but can preserve more
  noise.
- The `0.01` airlight floor and `0.1` transmission floor are defensive
  clamps. The first prevents dividing by tiny airlight components during
  normalization; the second prevents unrealistically large recovery
  gains in dense haze.
- The repeated `[0, 1]` and `1.0` clamps are not stylistic. They make
  the positive and negative paths total-order bounded even when the
  estimated transmission or airlight would otherwise push channels out
  of range.
- The `1024` chunk size is not part of the image model. It only affects
  rayon's work granularity in the CPU implementation.

## Preset-slider mapping

In preset TOML, dehaze is a single-field block:

```toml
[dehaze]
amount = 40.0
```

That serialized `amount` maps directly to `DehazeParams.amount`, with
validation at `-100.0..=100.0` and a default of `0.0` when the field or
section is absent.

The slider semantics are intentionally simple:

- `0` is neutral and skips the pass.
- Positive values map linearly to `omega = amount / 100.0`, so `100`
  means "use the full transmission estimate" and smaller values back the
  effect off proportionally.
- Negative values map linearly to `strength = -amount / 100.0`, but they
  do not run the positive recovery equation with a negative sign. They
  take the scene-aware airlight estimate and blend toward it, which is
  why negative dehaze feels like adding atmosphere instead of merely
  lowering contrast.

In other words, the slider is symmetric in range but asymmetric in
behavior: positive values solve the haze model, negative values reuse
the haze color estimate to synthesize fog.

## Source

- **CPU (Rust):** [`crates/agx/src/adjust/dehaze.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/dehaze.rs)
- **GPU (dehaze-specific WGSL kernels):**
  - [`dehaze_pixel_min.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/dehaze_pixel_min.wgsl)
  - [`dehaze_min_filter.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/dehaze_min_filter.wgsl)
  - [`dehaze_transmission.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/dehaze_transmission.wgsl)
  - [`dehaze_box_filter.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/dehaze_box_filter.wgsl)
  - [`dehaze_mul.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/dehaze_mul.wgsl)
  - [`dehaze_guided_coeffs.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/dehaze_guided_coeffs.wgsl)
  - [`dehaze_fma.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/dehaze_fma.wgsl)
  - [`dehaze_recover.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/dehaze_recover.wgsl)

The Rust file above is the canonical CPU implementation. The WGSL list
here is limited to the dehaze-specific kernels that implement the GPU
side of the same stages; shared supporting shader infrastructure lives
elsewhere in the render pipeline.

## References

[^dcp]: Kaiming He, Jian Sun, and Xiaoou Tang (2009). *Single Image Haze Removal Using Dark Channel Prior.* CVPR 2009. DOI: <https://doi.org/10.1109/CVPR.2009.5206515> PDF: <https://people.csail.mit.edu/kaiming/publications/cvpr09.pdf>

[^guided]: Kaiming He, Jian Sun, and Xiaoou Tang (2013). *Guided Image Filtering.* IEEE Transactions on Pattern Analysis and Machine Intelligence, 35(6), 1397-1409. DOI: <https://doi.org/10.1109/TPAMI.2012.213> PDF: <https://people.csail.mit.edu/kaiming/publications/pami12guidedfilter.pdf>
