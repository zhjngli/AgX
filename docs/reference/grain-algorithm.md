# Grain Algorithm Reference

This document describes AgX's film grain simulation: how it works, why it works that way, and the decisions made during development. It serves as institutional knowledge for anyone working on grain in the future.

## Overview

AgX simulates film grain by generating random noise, optionally blurring it to control particle size, and applying it to the image using exponential modulation. The grain system is designed to produce subtle, pleasing texture that resembles real analog film — not digital noise artifacts.

**User-facing parameters:**
- `grain_type` — selects grain character (Fine, Silver, Soft, Cubic, Tabular, Harsh)
- `amount` (0-100) — intensity of the grain effect
- `size` (0-100) — grain particle size (fine to coarse)
- `seed` — optional, for deterministic output (used in tests)

**Pipeline position:** After detail pass (sharpening/clarity), before vignette, in sRGB gamma space.

## Algorithm

The grain algorithm has three stages:

### 1. White noise generation

Each pixel gets an independent random value from a Gaussian distribution (mean 0, std dev scaled by the grain type's contrast multiplier). Uses a seeded PRNG (StdRng) with Box-Muller transform for Gaussian sampling without external dependencies.

The noise buffer is a flat `Vec<f32>` of size `width * height`.

### 2. Gaussian blur (size control)

The noise buffer is blurred with a separable Gaussian kernel. The blur sigma controls grain particle size:

```
sigma = (size / 100)^1.5 * max_sigma * (long_edge / 2000)
```

- The power-1.5 curve keeps low sizes sharp while higher sizes spread more
- `max_sigma = 2.5` at the reference resolution of 2000px
- Sigma scales linearly with image resolution so grain particles look the same relative size on a 1000px web export and a 6000px print file
- When sigma < 0.5, blur is skipped entirely (the kernel is too small to have visible effect, so noise is used as-is)

The blur reuses the separable Gaussian implementation from the detail pass (`detail::gaussian_blur`).

### 3. Exponential modulation (application)

Each pixel is modulated by the noise value:

```
mod_factor = exp(noise * strength * luminance_weight)
pixel_out = pixel_in * mod_factor
```

Where:
- `strength = (amount / 100) * 0.08 * grain_type.contrast`
- `luminance_weight = (1 - luma)^(0.5 * grain_type.luma_falloff)` — stronger in shadows, fading in highlights

The exponential function makes brightening and darkening perceptually symmetric: `exp(+x)` brightens by the same perceptual amount that `exp(-x)` darkens. Output is clamped to [0.0, 1.0].

### Chromatic grain

Each grain type defines an internal chromatic intensity. When non-zero, three additional per-channel perturbation buffers are generated and blurred. The per-channel noise is derived from the shared luminance noise plus a small independent component:

```
channel_noise = shared * (1 - chromatic) + independent * chromatic
```

This produces correlated per-channel variation (like film emulsion layers that mostly agree but differ slightly) rather than independent RGB noise (which looks digital). The chromatic effect is further scaled by pixel saturation (`max(R,G,B) - min(R,G,B)`) so that grayscale pixels receive no color shift.

## Grain Types

Each type is an internal configuration controlling noise amplitude, luminance falloff behavior, and chromatic intensity:

| Type | Contrast | Luma Falloff | Chromatic | Character |
|------|----------|-------------|-----------|-----------|
| Fine | 0.6 | 3.0 | 0.03 | Low intensity, fast highlight falloff. Clean, modern film. |
| Silver | 1.0 | 2.0 | 0.05 | Balanced. Classic film look. |
| Soft | 0.7 | 3.0 | 0.05 | Gentle, fast highlight falloff. Portrait film. |
| Tabular | 0.8 | 2.0 | 0.08 | Medium intensity, balanced. Modern T-grain films. |
| Cubic | 1.3 | 1.5 | 0.12 | Higher intensity, more even across tones. Traditional emulsions. |
| Harsh | 1.5 | 1.0 | 0.15 | Strongest. Grain visible everywhere. Pushed high-ISO film. |

The `contrast` multiplier scales the Gaussian noise standard deviation. At the extremes: Fine (0.6) produces ~60% of the noise amplitude that Silver (1.0) does, while Harsh (1.5) produces 150%.

The `luma_falloff` exponent controls how quickly grain fades in bright areas. At luma_falloff=1.0 (Harsh), the falloff is gentle — grain is visible across the full tonal range. At luma_falloff=3.0 (Fine, Soft), grain drops off rapidly in highlights.

## Key Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `GRAIN_STRENGTH_MULT` | 0.08 | Maps amount to noise intensity. At amount=50 with Silver: ~95% of pixels within ±4% brightness change. |
| `GRAIN_MAX_SIGMA` | 2.5 | Maximum blur sigma at size=100, at 2000px reference resolution. |
| `GRAIN_REF_RESOLUTION` | 2000.0 | Reference long edge for resolution scaling. |
| `GRAIN_BLUR_SIGMA_THRESHOLD` | 0.5 | Below this sigma, blur is skipped (no visible effect). |

## Design Decisions and History

### Why white noise, not simplex noise

The original grain implementation (2026-03-23) used multi-octave 2D simplex noise. Simplex noise is spatially coherent by design — neighboring pixels produce correlated values, creating smooth, organic patterns. This is good for terrain or cloud generation but wrong for film grain.

At moderate-to-high size values, simplex noise produced low-frequency cloud-like blobs rather than discrete grain particles. All e2e presets had to cap grain size at 22-30 to avoid this. The problem is fundamental to how simplex noise works: reducing the sampling frequency makes the noise smoother and blobber, not larger-grained.

White noise (independent random value per pixel) combined with Gaussian blur produces correct grain character because the blur radius directly controls particle size. Unblurred white noise looks like fine-grain film; lightly blurred looks like medium-grain; more blurred looks like pushed high-ISO film. This is the technique used by GIMP/Photoshop grain workflows and Boris FX.

The original design rejected white noise because it required buffer-level processing (a blur pass). At the time, grain was the first implementation and a per-pixel approach was simpler. After the detail pass established buffer-level processing as routine, the buffer cost became justified — especially since the simplex approach produced unacceptable results at moderate sizes.

### Why exponential modulation, not additive or linear multiplicative

Three blending approaches were tried:

1. **Additive** (`pixel + noise * scale`): Creates an "overlaid dark splotches" artifact. All channels shift by the same absolute amount regardless of pixel brightness, which means a pink area turns grayish-dark rather than darker-pink. The noise reads as a dark pattern laid on top of the image.

2. **Linear multiplicative** (`pixel * (1 + noise * scale)`): Preserves color (pink stays pink) but creates a "dark spots only" artifact. Brightening is imperceptible on already-bright pixels, while darkening is very visible on mid-tone pixels. The asymmetry makes noise read as predominantly dark spots.

3. **Exponential** (`pixel * exp(noise * scale)`): Symmetric in log space — brightening and darkening are perceptually equal. A noise value of +0.1 brightens by the same visual amount that -0.1 darkens. This eliminates the dark-spots bias and produces balanced grain texture.

### Why shadow-heavy luminance falloff

The luminance weight function is `(1 - luma)^(0.5 * falloff)`:
- At luma=0 (black): weight=1.0 — full grain
- At luma=1 (white): weight=0.0 — no grain
- The curve shape is controlled by the grain type's `luma_falloff` parameter

This matches real film behavior: underexposed areas (shadows) have low signal-to-noise ratio, making grain more visible. Well-exposed highlights have dense silver development that masks grain structure.

An earlier implementation used a parabolic mid-tone peak (strongest grain at luma=0.5, falling off in both shadows and highlights). This was wrong — grain was too visible in bright skies and not visible enough in shadow areas. The correction came from direct feedback comparing rendered grain against expectations from real film editing experience.

### Why resolution-scaled sigma

Without resolution scaling, a sigma of 2.5px produces very different visual grain on a 1000px web export versus a 6000px print file. The fix: scale sigma proportionally to the image's long edge relative to a 2000px reference resolution.

```
effective_sigma = base_sigma * (long_edge / 2000)
```

A 4000px image gets sigma=5.0 at size=100; a 1000px image gets sigma=1.25. Grain particles maintain consistent visual size relative to the image regardless of resolution.

### Why max_sigma = 2.5

Determined through visual grid search tuning across 4 rounds:

1. Sigma 2.5-5.0 with high strength (0.3-0.8): all too aggressive, dark spots problem persisted
2. Sigma 2.0-5.0 with lower strength (0.05-0.12): sigma ≤ 2.0 looked like grain; higher sigma produced visible blotches
3. Sigma 0.8-1.8 with varied strength: all looked good, confirmed the fine-grain range
4. Sigma 2.0-4.0 with moderate strength: confirmed 2.5 as the upper boundary where grain still reads as grain

Above sigma ~2.5 (at reference resolution), the blurred noise clumps become large enough to read as patches or splotches overlaid on the image rather than as grain texture. The effect transitions from "subtle film texture" to "ugly blotchy overlay." This boundary is perceptual, not mathematical — it's where the viewer's eye starts grouping the noise clumps as discrete objects rather than perceiving them as texture.

### Why strength_mult = 0.08

The original value of 0.4 caused ±24% brightness swings at moderate settings (amount=35, Silver). This was far too aggressive — grain was the dominant visual element rather than a subtle texture.

At 0.08, the math works out to:
- amount=35, Silver (contrast=1.0): `exp_arg_std ≈ 0.028`, ~95% of pixels within ±5% brightness — barely perceptible
- amount=50, Silver: ~95% within ±8% — clearly visible texture but not distracting
- amount=100, Harsh (contrast=1.5): ~95% within ±24% — bold and prominent

This range matches the user's expectation: grain should be "subtle but pleasing" at moderate settings, with the ability to push it harder for creative effect.

### Why type-driven chromatic, not a user slider

The original design had a `chromatic` parameter (0-100) as a user-facing slider. This generated fully independent per-channel white noise buffers, producing digital-looking RGB confetti — each channel gets completely uncorrelated random values, which looks nothing like film.

Industry research showed that most pro photo editors (Lightroom, Capture One, darktable) don't offer a chromatic grain slider at all. Capture One bakes chromatic behavior into the grain type implicitly. The decision was made to follow Capture One's model: remove the user-facing slider and let each grain type define its own chromatic intensity internally.

The chromatic variation is implemented as correlated per-channel noise (shared luminance noise + small independent perturbation per channel) rather than fully independent per-channel noise. This produces the look of "film emulsion layers that mostly agree but differ slightly" — subtle warm/cool shifts at grain boundaries rather than random color speckles.

The chromatic effect is scaled by pixel saturation so that grayscale/BW images receive no color shifts automatically, without needing explicit BW detection.

## Memory Profile

Grain allocates single-channel f32 noise buffers:

- **Without chromatic grain:** 1 noise buffer + blur (peak ~3 buffers during blur, 1 after)
- **With chromatic grain:** 1 shared + 3 per-channel perturbation buffers + blur (peak ~5 buffers)
- Per buffer at 24MP: ~92MB

All buffers are freed immediately after the grain step completes.

## Related Documents

- [Original grain design](../plans/2026-03-23-grain-design.md) — initial simplex noise approach
- [Grain size fix design](../plans/2026-03-27-grain-size-fix-design.md) — white noise + blur rework
- [Chromatic grain design](../plans/2026-03-29-chromatic-grain-design.md) — type-driven chromatic
- [Processing parity backlog](../backlog/processing-parity.md) — grain size bug tracking
