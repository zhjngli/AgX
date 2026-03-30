# Grain Size Algorithm Fix

**Date:** 2026-03-27
**Status:** Approved
**Branch:** `fix/grain-size-algorithm`

## Problem

The grain size parameter maps to simplex noise frequency via an exponential curve:
```
base_freq = 0.1 * (0.02).powf(size / 100.0)
```

This shifts the entire frequency spectrum downward as size increases. At moderate values (size=40-50), the noise becomes low-frequency cloud-like blobs rather than visible film grain. Real film grain at larger sizes should look like bigger discrete particles (1-6px), not smooth undulations.

All current e2e presets cap grain size at 22-30 to work around this.

### Current behavior

- size=0: base_freq=0.1 — fine, dense grain (good)
- size=25: base_freq≈0.038 — starting to get soft
- size=50: base_freq≈0.014 — visible low-frequency blobs
- size=100: base_freq=0.002 — huge cloud-like splotches

## Industry Research

| Editor | Size mechanism | Result |
|--------|---------------|--------|
| **Lightroom** | Frequency + blur hybrid. Adobe docs: "At sizes of 25 or above, blurring is added to better blend the grain with the image." | No blob problem; grain particles scale naturally |
| **darktable** | Pure frequency scaling (same as our current approach) | Same blob problem at high coarseness; [open issue #4451](https://github.com/darktable-org/darktable/issues/4451) |
| **RawTherapee** | Ported from darktable, identical algorithm | Same blob problem |
| **GIMP/Photoshop** | White noise + Gaussian blur (manual technique) | Blur radius directly controls particle size; clean results |
| **Boris FX** | Explicit separate "Grain Size" and "Grain Blur" sliders | Most transparent decomposition of the technique |

The blur-based approach is what produces the best results in practice. Pure frequency scaling is recognized as inferior at larger sizes.

## Solution: Blur-Based Grain Sizing

Replace frequency-based sizing with a Lightroom-style hybrid: generate noise at a fixed high frequency, then blur the noise field to control particle size.

### Architectural note

The original grain design spec (`docs/plans/2026-03-23-grain-design.md`) rejected a blur-based approach as "architecturally heavier" since it requires a buffer-level pass. That rejection was correct at the time — grain was the first implementation, and a per-pixel approach was simpler. Now that we've seen the per-pixel frequency approach produce unacceptable blob artifacts at moderate sizes, the buffer cost is justified. The detail pass already establishes the buffer-level processing pattern, and memory allocations of this size are routine in the pipeline.

### Algorithm

1. **White noise generation** — generate independent random values per pixel using a seeded PRNG (Gaussian distribution, mean 0). Each pixel is statistically independent — no spatial correlation. This produces the sharp, discrete variation that characterizes real film grain.

2. **Post-blur controlled by size** — blur the noise buffer with a Gaussian whose sigma scales with the size parameter:
   - size=0: sigma=0, no blur (skip buffer allocation entirely)
   - size=100: sigma at tuned maximum (starting point: ~2-3px)
   - Mapping: `sigma = (size / 100.0).powf(1.5) * max_sigma` (non-linear — low sizes stay sharp, higher sizes spread more)

3. **Apply blurred noise (multiplicative blending)** — read per-pixel from the blurred noise buffer and apply as a multiplicative modulation: `pixel * (1.0 + noise * scale)`. This preserves the underlying color (pink stays pink, just brighter/darker) and provides natural luminance falloff (dark pixels change less in absolute terms). The original additive blending (`pixel + noise * scale`) created an "overlaid dark splotches" artifact because it shifted all channels by the same absolute amount regardless of pixel brightness.

### Why white noise instead of simplex noise

The original grain implementation and the first iteration of this fix used multi-octave simplex noise. Simplex noise is spatially coherent by design — neighboring pixels produce correlated values, creating smooth, organic patterns. This is desirable for terrain or cloud generation, but produces the wrong character for film grain: even at high frequencies, simplex noise features are inherently blobby and smooth rather than sharp and discrete.

White noise (independent random values per pixel) combined with Gaussian blur produces the correct grain character because:
- **Unblurred:** pure pixel-level randomness, like fine-grain film (ISO 50-100)
- **Lightly blurred:** small clusters of correlated pixels, like medium-grain film (ISO 400)
- **More blurred:** larger soft clusters, like pushed/high-ISO film (ISO 1600+)
- The blur radius directly and intuitively controls particle size

This is the technique used by GIMP/Photoshop (manual grain workflow) and Boris FX (explicit grain size + blur sliders). The original grain design (`docs/plans/2026-03-23-grain-design.md`) rejected white noise because it required buffer-level processing, but that architectural constraint no longer applies — this fix already uses buffers.

The simplex noise implementation is retained in the codebase for potential future use (e.g., texture generation, other effects) but is no longer used for grain.

### Upper bound on blur

If sigma is too large, noise averages out to near-zero and grain disappears. The max sigma must be constrained so that grain remains visible at all size values. This is validated empirically during tuning (see Verification Process).

### Tuning constants

Tuned via visual grid search (3 test images × multiple parameter combinations, scripts/grain_tuning.sh):

- **Base frequency:** 0.08 (unused by white noise, retained for simplex compatibility)
- **Max sigma:** 2.5px at reference resolution — sigma above ~2.5 starts producing visible blotchiness; below ~2.0 grain stays fine-textured. 2.5 is the upper limit where grain still reads as grain rather than splotches.
- **Sigma curve shape:** power 1.5 (low sizes stay sharp, higher sizes spread more)
- **Strength multiplier:** 0.08 — the original 0.4 caused ±24% brightness swings at moderate settings, producing aggressive dark spots. At 0.08, amount=50 with Silver gives exp argument std ≈ 0.04, meaning ~95% of pixels within ±4% brightness change — subtle and pleasing.
- **Reference resolution:** 2000px long edge — sigma scales proportionally so grain particles maintain consistent visual size regardless of image resolution.

Tuning rounds:
1. First round (strength 0.3–0.8, sigma 2.5–5.0): all too aggressive, dark spots problem persisted
2. Second round (strength 0.05–0.12, sigma 2.0–5.0): sigma ≤2.0 looked good; higher sigma still blotchy
3. Third round (strength 0.06–0.16, sigma 0.8–1.8): all looked good, confirmed fine-grain range
4. Fourth round (strength 0.08–0.12, sigma 2.0–4.0): confirmed 2.5 as upper boundary, 3.0+ too blotchy

## Size vs Grain Type

- **Grain type** (fine, silver, harsh, etc.) controls the *character* of the grain: octave weights, contrast multiplier, luminance falloff strength. It's choosing the film stock.
- **Size** controls how *big* each grain particle appears. It's choosing the enlargement/print size.

The blur-based approach keeps these orthogonal. Grain type still controls the multi-octave noise generation. Size controls the post-blur. They don't interact.

## Pipeline Integration

No change to grain's position: after detail pass, before vignette, in sRGB gamma space.

### Fast path (sigma below threshold)

No buffer allocation. Noise is computed per-pixel inline, same as the current code path but with a fixed base frequency. This preserves the current zero-overhead behavior when grain size is small.

The fast path is used when `sigma < 0.5` (approximately size < 15, depending on the curve). Below this threshold, the blur kernel is too small to produce a visible effect, so buffer allocation would be wasted. This also means size=0 uses the fixed base frequency (starting point: 0.08) instead of the current 0.1 — a minor output change for existing presets at low size values. All grain-using goldens will be regenerated regardless.

### Buffer path (sigma >= 0.5)

1. Allocate a single-channel f32 noise buffer (width × height)
2. Fill it with multi-octave simplex noise at fixed high frequency
3. Blur the noise buffer with separable Gaussian (sigma from size parameter), using a temporary buffer for the horizontal pass
4. In the per-pixel grain application loop, read from the blurred noise buffer instead of computing noise inline
5. For chromatic grain (chromatic>0): generate and blur 3 additional per-channel noise buffers. The temp blur buffer can be reused across channels.
6. Free all noise buffers after grain step completes

### Memory

The existing `gaussian_blur` in `detail.rs` allocates an internal temp buffer and returns a new output buffer. Peak memory during a single blur call is 3 single-channel f32 buffers (input + internal temp + output). The internal temp is freed when the function returns.

- **Luminance-only grain:** Generate noise buffer (1), blur it (peak: 3 during blur, 1 after). Peak: 3 buffers.
- **Chromatic grain:** Generate shared noise (1) + 3 per-channel noise buffers (3), blur each sequentially. Peak during any one channel blur: existing buffers + 2 (internal temp + output). Total peak: ~5 buffers if channel results are retained, fewer if applied and freed sequentially.
- Per buffer: width x height x 4 bytes. For 24MP: ~92MB each.
- Comparable to detail pass buffer allocations. Freed immediately after grain step.

### Gaussian blur reuse

The `gaussian_blur` function in `detail.rs` already implements separable single-channel Gaussian blur. Make it `pub(crate)` so the grain module can call it directly. If peak memory becomes a concern, a future optimization could add an in-place blur variant, but the current API is sufficient for this fix.

## Files Changed

| File | Change |
|------|--------|
| `crates/agx/src/adjust/grain.rs` | Replace frequency-based sizing with blur-based sizing. Add `generate_noise_buffer` and `apply_grain_buffer` functions. Refactor `GrainPrecomputed` to hold fixed base frequency instead of size-dependent frequency. Keep per-pixel fast path for size=0. |
| `crates/agx/src/adjust/detail.rs` | Make `gaussian_blur` and `build_gaussian_kernel` `pub(crate)` (or extract to shared util) |
| `crates/agx/src/engine/mod.rs` | Call buffer-level grain when sigma >= threshold, per-pixel grain otherwise. Update stale pipeline doc comment to reflect correct order (detail -> grain -> vignette). |
| E2E presets (10 files) | Raise grain size values above the current 22-30 cap |
| Golden files | Regenerate all grain-using preset goldens |
| `docs/backlog/grain-size-algorithm.md` | Check off completed sub-tasks |

## Testing Strategy

### Unit tests (grain.rs)

- Blurred noise at all size values (0, 25, 50, 75, 100) has variance above a minimum threshold (grain doesn't wash out)
- Blur at sigma=0 produces identical output to unburred noise (fast path equivalence)
- Higher size values produce lower spatial frequency (adjacent-pixel delta decreases) but variance stays high
- Existing tests (determinism, luminance falloff, chromatic, clamping, resolution independence) continue to pass

### Visual verification (manual, during tuning)

Render a test image with grain at size=0, 25, 50, 75, 100:
- Discrete grain particles visible at every step
- Clear size progression from fine to coarse
- No cloud-like blobs at any size
- Compare against Lightroom output at equivalent settings

### E2E golden tests

- Update grain-using presets with higher size values (40-70 range)
- Regenerate goldens
- Confirm output images show grain, not blobs

## References

- [Adobe Camera Raw grain docs](https://helpx.adobe.com/camera-raw/using/vignette-grain-effects-camera-raw.html) — "At sizes of 25 or above, blurring is added"
- [darktable grain.c source](https://github.com/darktable-org/darktable/blob/master/src/iop/grain.c) — pure frequency scaling
- [darktable issue #4451](https://github.com/darktable-org/darktable/issues/4451) — blob problem discussion
- `crates/agx/src/adjust/grain.rs` — current implementation
- `docs/plans/2026-03-23-grain-design.md` — original grain design spec
