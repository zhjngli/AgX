<!-- Canonical source: crates/agx/src/adjust/denoise.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations listed in the Source section below. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Noise reduction runs in linear RGB before the later gamma-space tone and
detail work. AgX converts the image into one luminance channel and two
chroma-difference channels, denoises each with a redundant à trous
wavelet decomposition, soft-thresholds the wavelet detail bands, then
reconstructs RGB.[^atrous] The redundant, non-decimated structure is the
point: every wavelet level stays at full image resolution, so threshold
decisions are translation-invariant and do not introduce the zippering
or shift sensitivity that a decimated pyramid can produce.

## How it works

The denoise pass has three user-facing controls:

- `luminance`: how strongly to denoise the luminance branch
- `color`: how strongly to denoise the two chroma branches
- `detail`: how much of the finest luminance band to protect

All three parameters live in `0.0..=100.0`. When they are all zero,
`is_neutral()` short-circuits the entire pass.

### 1. Split into luminance and chroma

AgX first rewrites linear RGB into one luminance-like channel and two
chroma-difference channels:

```text
Y  = 0.2126 R + 0.7152 G + 0.0722 B
Cb = B - Y
Cr = R - Y
```

That separation is important because luminance noise and chroma noise do
not look equally bad. Chroma blotches are usually more objectionable
than monochrome grain, so AgX lets the color channels be smoothed
independently from the luma channel instead of driving all three RGB
channels with one shared threshold.

### 2. Build a five-level à trous stack

Each channel is decomposed independently into five detail bands plus one
final residual. Every level uses the same separable B3-spline low-pass
kernel:

```text
[1/16, 4/16, 6/16, 4/16, 1/16]
```

At level `k`, the tap spacing is `2^k`, so the kernel footprint grows
without downsampling the image:

1. Convolve horizontally with the strided B3-spline kernel.
2. Convolve that result vertically with the same strided kernel.
3. Subtract the smoothed approximation from the previous
   approximation to get the detail band for that level.
4. Reuse the smoothed approximation as the input to the next level.

Boundary handling is mirror reflection at the image edges. The CPU and
GPU paths share the same kernel weights, gap schedule, and mirror logic.

### 3. Estimate the noise floor from the finest band

After level 0, AgX estimates one global noise sigma per channel from the
median absolute deviation of that finest detail band:

```text
sigma = median(|detail_0|) / 0.6745
```

This is a robust estimate for approximately Gaussian noise and is cheap
enough to reuse across all coarser bands. AgX does not currently model
signal-dependent sensor noise or spatially varying noise; the threshold
schedule is driven by this single per-channel sigma.

### 4. Soft-threshold each wavelet band

Each detail band is shrunk toward zero with soft thresholding:

```text
soft(x, t) = sign(x) * max(|x| - t, 0)
```

The threshold for each level is:

```text
threshold(level) = sigma * level_scale[level] * strength
```

with fixed per-level scale factors:

```text
[1.0, 1.0, 1.2, 1.5, 2.0]
```

The user sliders are mapped as follows:

- `luminance` and `color` map linearly from `0..100` to `0.0..3.0`
  threshold multipliers.
- `detail` maps to `detail_factor = 1.0 - detail / 100.0`.
- That `detail_factor` is applied only to the level-0 luminance
  threshold. At `detail = 100`, the finest luma band gets zero
  thresholding; at `detail = 0`, it gets the full threshold.

That last point is deliberate. AgX protects the finest-scale luminance
detail because that is where edge crispness and texture live. Chroma
bands are not given a matching protection term, because the algorithm
leans toward removing color speckling more aggressively than monochrome
grain.

### 5. Reconstruct the channel

Once all five detail bands have been thresholded, the denoised channel
is reconstructed as:

```text
residual + detail_0 + detail_1 + detail_2 + detail_3 + detail_4
```

The CPU path keeps the detail bands and residual explicitly, then sums
them at the end. The GPU path accumulates thresholded detail bands as it
goes and adds the final residual in a last pass. Both produce the same
wavelet reconstruction model.

### 6. Write the denoised channels back to RGB

After denoising, AgX converts the channels back into RGB. The CPU path
clamps once after reconstructing the full `(Y, Cb, Cr)` triplet, while
the GPU path clamps after each sequential channel write-back.

Conceptually the relationship is still:

```text
R = Y + Cr
B = Y + Cb
G = (Y - 0.2126 R - 0.0722 B) / 0.7152
```

The CPU implementation reconstructs RGB from the full denoised
`(Y, Cb, Cr)` triplet in one step, then clamps the final pixel. The GPU
path writes the channels back sequentially: it rescales RGB for the `Y`
pass, then updates `Cb`, then `Cr`, clamping after each pass. The two
paths are meant to produce near-identical output, but they do not use
the exact same write-back mechanism.

## Why we chose it

AgX uses à trous denoising because it matches the shape of the pipeline
well:

- It is isotropic and translation-invariant, so it behaves predictably
  on photographic texture and edges.
- The same fixed B3-spline filter bank works on CPU and GPU with almost
  identical math.
- Per-subband thresholding makes the user model simple: one luma
  strength, one chroma strength, one finest-detail protection term.
- It fits AgX's adjustment pipeline cleanly as a linear-space full-image
  pass, alongside dehaze.

AgX intentionally keeps the implementation conservative. There is no
learned noise model, no local variance estimation, and no cross-channel
joint thresholding. The pass is a fixed five-level stationary wavelet
shrinkage stage whose behavior is meant to be stable, portable, and
easy to reason about from preset values.

## Parameters and constants

| Constant | Value | Role |
|----------|-------|------|
| `NR_MIN` | `0.0` | Lowest accepted slider value |
| `NR_MAX` | `100.0` | Highest accepted slider value |
| Neutral state | all params `0.0` | Skip the pass entirely |
| `NUM_LEVELS` | `5` | Number of à trous detail bands |
| B3 kernel | `[1/16, 4/16, 6/16, 4/16, 1/16]` | Separable low-pass filter at every level |
| Gap schedule | `1, 2, 4, 8, 16` | Tap spacing per level |
| `LEVEL_SCALE` | `[1.0, 1.0, 1.2, 1.5, 2.0]` | Per-band threshold multiplier |
| Sigma constant | `0.6745` | MAD-to-sigma conversion factor |
| Output clamp | `[0.0, 1.0]` | Keep RGB in valid linear range |

One subtle but important implementation detail: `detail` by itself does
not add denoising. If `luminance == 0` and `color == 0`, the channel
strengths stay at zero, so the output remains unchanged even if
`detail > 0`.

**Beyond the expected range:** preset validation rejects `luminance`,
`color`, and `detail` outside `0.0..=100.0`. The internal constants
(filter taps, gap schedule, level scaling) are part of the algorithm
itself rather than tuning knobs — they are listed for transparency,
not exposed for tweaking.

## Preset-slider mapping

In preset TOML, noise reduction is serialized as:

```toml
[noise_reduction]
luminance = 40.0
color = 25.0
detail = 50.0
```

Those values map directly to `NoiseReductionParams`. The semantics are:

- `luminance = 0` disables denoising on `Y`; `100` maps to a threshold
  multiplier of `3.0`.
- `color = 0` disables denoising on both `Cb` and `Cr`; `100` likewise
  maps to `3.0`.
- `detail = 0` gives the finest luminance band its full threshold;
  `detail = 100` suppresses level-0 luminance thresholding entirely.

The mapping is linear and intentionally narrow. Presets stay portable
because all deeper algorithm constants remain fixed in code.

## Source

- **CPU (Rust):** [`crates/agx/src/adjust/denoise.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/denoise.rs)
- **GPU dispatcher:** [`crates/agx/src/engine/gpu/stages/denoise.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/engine/gpu/stages/denoise.rs)
- **GPU WGSL kernels:**
  - [`denoise_rgb_to_channel.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/denoise_rgb_to_channel.wgsl)
  - [`denoise_atrous_h.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/denoise_atrous_h.wgsl)
  - [`denoise_atrous_v.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/denoise_atrous_v.wgsl)
  - [`denoise_threshold_accum.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/denoise_threshold_accum.wgsl)
  - [`denoise_add_residual.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/denoise_add_residual.wgsl)
  - [`denoise_channel_to_rgb.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/denoise_channel_to_rgb.wgsl)

## References

[^atrous]: Albert Bijaoui, Jean-Luc Starck, and Fionn Murtagh (1994). *Multiscale Image Restoration by the À Trous Algorithm* / *Restauration des images multi-échelles par l'algorithme à trous*. PDF: <https://gretsi.fr/data/ts/pdf/1994_11_3_1863_1.pdf>
