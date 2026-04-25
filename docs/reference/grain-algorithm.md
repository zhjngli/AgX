# Grain Algorithm Reference

This document describes AgX's film grain simulation: how it works, why it works that way, and the decisions made during development. It serves as institutional knowledge for anyone working on grain in the future.

## Overview

AgX simulates film grain by generating random noise, optionally blurring it to control particle size, and applying it to the image using exponential modulation. The grain system is designed to produce subtle, pleasing texture that resembles real analog film — not digital noise artifacts.

**User-facing parameters:**

- `grain_type` — selects grain character (Fine, Silver, Harsh)
- `amount` (0-100) — intensity of the grain effect
- `size` (0-100) — grain particle size (fine to coarse)
- `seed` — optional, for deterministic output (used in tests)

**Pipeline position:** After detail pass (sharpening/clarity), before vignette, in sRGB gamma space.

## Algorithm

The grain algorithm has three stages:

### 1. White noise generation

Each pixel gets an independent random value from a standard normal distribution (mean 0, std dev 1). Uses a seeded PRNG (StdRng) with Box-Muller transform for Gaussian sampling without external dependencies. The noise is generated with unit variance; the grain type's contrast multiplier is applied later during the scale calculation, not during generation.

The noise buffer is a flat `Vec<f32>` of size `width * height`.

### 2. Gaussian blur (size control)

The noise buffer is blurred with a separable Gaussian kernel. The blur sigma controls grain particle size:

```
sigma = (size / 100)^1.5 * max_sigma * (long_edge / 2000)
```

- The power-1.5 curve keeps low sizes sharp while higher sizes spread more
- `max_sigma = 1.0` at the reference resolution of 2000px
- Sigma scales linearly with image resolution so grain particles look the same relative size on a 1000px web export and a 6000px print file
- When sigma < 0.3, blur is skipped entirely (the kernel is too small to have visible effect, so noise is used as-is)

The blur reuses the separable Gaussian implementation from the detail pass (`detail::gaussian_blur`).

### 3. Exponential modulation (application)

Each pixel is modulated by the noise value:

```
amount_factor = (amount / 100) ^ grain_type.amount_curve
scale = grain_type.contrast * 0.04 * amount_factor
effective_falloff = grain_type.luma_falloff * (1 - 0.4 * amount_factor)
w = (1 - luma) ^ (0.5 * effective_falloff)
nws = noise * w * scale

// Additive grain for shadows, multiplicative for midtones/highlights
additive_delta = nws * 0.35
multiplicative_delta = pixel_in * (exp(nws) - 1)
blend = smoothstep(0.1, 0.2, luma)
delta = lerp(additive_delta, multiplicative_delta, blend)
pixel_out = pixel_in + delta
```

Where:

- `amount_factor` uses a per-type curve exponent so different grain types respond differently to the slider
- `scale = grain_type.contrast * 0.04 * amount_factor` — contrast is applied once here (not during noise generation)
- `effective_falloff` decreases as amount increases, spreading grain from shadows into midtones/highlights at high intensities
- `w = (1 - luma)^(0.5 * effective_falloff)` — luminance weight, stronger in shadows, fading in highlights
- `blend` smoothly transitions from additive (0) to multiplicative (1) between luma 0.1 and 0.2
- In deep shadows (luma < 0.1): fully additive grain produces visible bright specks on dark pixels, matching real film where sparse developed crystals appear as bright points
- In midtones/highlights (luma > 0.2): fully multiplicative grain gives perceptually correct proportional density modulation
- The exponential function makes brightening and darkening perceptually symmetric: `exp(+x)` brightens by the same perceptual amount that `exp(-x)` darkens

Output is clamped to [0.0, 1.0].

### Chromatic grain

Each grain type defines an internal chromatic intensity. Three additional per-channel perturbation buffers are always generated and blurred alongside the shared buffer. The per-channel noise is derived from the shared luminance noise plus a small independent component:

```
effective_chromatic = grain_type.chromatic * pixel_chroma * shadow_chromatic_boost
channel_noise = shared * (1 - effective_chromatic) + independent * effective_chromatic
```

Where:

- `pixel_chroma = max(R,G,B) - min(R,G,B)` — grayscale pixels get zero chromatic divergence
- `shadow_chromatic_boost = 2 - luma` — ranges from 1.0 (highlights) to 2.0 (shadows), boosting color fringing in dark areas where film grain shows as color shifts rather than luminance changes

This produces correlated per-channel variation (like film emulsion layers that mostly agree but differ slightly) rather than independent RGB noise (which looks digital). The shadow boost reflects how real film grain manifests differently across the tonal range: in shadows, where luminance changes from grain are less visible, the color fringing becomes the dominant grain artifact.

## Grain Types

Each type is an internal configuration controlling noise amplitude, luminance falloff behavior, chromatic intensity, and amount curve response:

| Type | Contrast | Luma Falloff | Chromatic | Amount Curve | Character |
|------|----------|-------------|-----------|-------------|-----------|
| Fine | 0.95 | 2.5 | 0.05 | 0.7 | Subtle, fast highlight falloff. Clean, modern film. |
| Silver | 1.2 | 1.5 | 0.10 | 0.6 | Balanced. Classic film look. |
| Harsh | 1.5 | 0.8 | 0.15 | 0.5 | Strongest. Grain visible everywhere. Pushed high-ISO film. |

The `contrast` multiplier scales the noise intensity (applied once in the scale calculation, not during noise generation). Fine (0.95) produces ~79% of the noise intensity that Silver (1.2) does, while Harsh (1.5) produces 125%.

The `luma_falloff` exponent controls how quickly grain fades in bright areas. At luma_falloff=0.8 (Harsh), the falloff is gentle — grain is visible across the full tonal range. At luma_falloff=2.5 (Fine), grain drops off rapidly in highlights. This is a *base* value that decreases dynamically as amount increases (see "Dynamic luma falloff" below).

The `amount_curve` exponent controls slider response. Lower values (Harsh at 0.5) make grain kick in faster at low slider values; higher values (Fine at 0.7) keep grain subtle longer. The amount factor is `(amount / 100) ^ amount_curve`.

## Key Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `GRAIN_STRENGTH_MULT` | 0.04 | Maps amount to noise intensity. Combined with contrast and amount curve for final scale. |
| `GRAIN_MAX_SIGMA` | 1.0 | Maximum blur sigma at size=100, at 2000px reference resolution. |
| `GRAIN_REF_RESOLUTION` | 2000.0 | Reference long edge for resolution scaling. |
| `GRAIN_BLUR_SIGMA_THRESHOLD` | 0.3 | Below this sigma, blur is skipped (no visible effect). |
| `GRAIN_ADDITIVE_END` | 0.1 | Luma below which grain is fully additive (bright specks in shadows). |
| `GRAIN_MULTIPLICATIVE_START` | 0.2 | Luma above which grain is fully multiplicative (proportional modulation). |
| `GRAIN_ADDITIVE_SCALE` | 0.35 | Scale factor for additive grain relative to multiplicative strength. |
| `GRAIN_FALLOFF_REDUCTION` | 0.4 | How much luma falloff decreases at amount=100. A base falloff of 2.5 drops to 1.5, spreading grain into midtones. |
| `GRAIN_SIZE_CURVE_EXPONENT` | 1.5 | Exponent for the size-to-sigma curve. Keeps grain tight at low sizes. |
| `GRAIN_LUMINANCE_WEIGHT_SCALE` | 0.5 | Multiplier on falloff exponent in luminance weight. Controls shadow-to-highlight gradient steepness. |
| `GRAIN_PARAM_MAX` | 100.0 | Maximum value for user-facing amount and size parameters. |

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

The luminance weight function is `(1 - luma)^(0.5 * effective_falloff)`:

- At luma=0 (black): weight=1.0 — full grain
- At luma=1 (white): weight=0.0 — no grain
- The curve shape is controlled by the grain type's base `luma_falloff`, dynamically reduced as amount increases (see "Why dynamic luma falloff" below)

This matches real film behavior: underexposed areas (shadows) have low signal-to-noise ratio, making grain more visible. Well-exposed highlights have dense silver development that masks grain structure.

An earlier implementation used a parabolic mid-tone peak (strongest grain at luma=0.5, falling off in both shadows and highlights). This was wrong — grain was too visible in bright skies and not visible enough in shadow areas. The correction came from direct feedback comparing rendered grain against expectations from real film editing experience.

### Why resolution-scaled sigma

Without resolution scaling, a given sigma produces very different visual grain on a 1000px web export versus a 6000px print file. The fix: scale sigma proportionally to the image's long edge relative to a 2000px reference resolution.

```
effective_sigma = base_sigma * (long_edge / 2000)
```

A 4000px image gets sigma=2.0 at size=100; a 1000px image gets sigma=0.5. Grain particles maintain consistent visual size relative to the image regardless of resolution.

### Why max_sigma = 1.0

The original value of 2.5 was determined through visual tuning but proved too high in practice — at moderate-to-high sizes, noise clumps became visible blotches. The reduction to 1.0 keeps grain firmly in the "texture" range across all size values. At reference resolution (2000px), even size=100 produces sigma=1.0, which reads as coarse but believable film grain rather than overlaid patches.

The lower ceiling works in concert with the reduced `GRAIN_BLUR_SIGMA_THRESHOLD` (0.3, down from 0.5), which means blur is applied across more of the size range. Previously, a large portion of the low-to-mid size range was unblurred; now blur kicks in earlier, providing smoother gradation from fine to coarse.

### Why strength_mult = 0.04

The original value of 0.4 was far too aggressive. It was reduced to 0.08, which worked when contrast was applied during noise generation (scaling the Gaussian std dev). After fixing the double-counting bug (contrast was being applied in both noise generation AND the scale calculation), contrast was removed from noise generation — the noise now has std dev 1.0. This meant the effective strength doubled, requiring a halving of the multiplier from 0.08 to 0.04 to maintain the same output intensity.

At 0.04 with the current per-type amount curves, the math produces appropriately subtle grain at moderate settings and bold grain when pushed:

- amount=50, Silver (contrast=1.2, amount_curve=0.6): visible texture but not distracting
- amount=100, Harsh (contrast=1.5, amount_curve=0.5): bold and prominent

This range matches the expectation: grain should be "subtle but pleasing" at moderate settings, with the ability to push it harder for creative effect.

### Why type-driven chromatic, not a user slider

The original design had a `chromatic` parameter (0-100) as a user-facing slider. This generated fully independent per-channel white noise buffers, producing digital-looking RGB confetti — each channel gets completely uncorrelated random values, which looks nothing like film.

Industry research showed that most pro photo editors (Lightroom, Capture One, darktable) don't offer a chromatic grain slider at all. Capture One bakes chromatic behavior into the grain type implicitly. The decision was made to follow Capture One's model: remove the user-facing slider and let each grain type define its own chromatic intensity internally.

The chromatic variation is implemented as correlated per-channel noise (shared luminance noise + small independent perturbation per channel) rather than fully independent per-channel noise. This produces the look of "film emulsion layers that mostly agree but differ slightly" — subtle warm/cool shifts at grain boundaries rather than random color speckles.

The chromatic effect is scaled by pixel saturation so that grayscale/BW images receive no color shifts automatically, without needing explicit BW detection.

### Why only three grain types

The original six types (Fine, Silver, Soft, Cubic, Tabular, Harsh) were reduced to three. Soft overlapped heavily with Fine (both had high falloff, low contrast); Tabular was a slight variation of Silver; Cubic sat between Silver and Harsh without a distinct character. The three remaining types cover the useful range clearly: Fine (subtle/modern), Silver (balanced/classic), Harsh (aggressive/pushed). Fewer types also simplifies the per-type tuning matrix and reduces the combinatorial burden on e2e testing.

### Why per-type amount curve exponents

A linear amount mapping (amount/100) treats all grain types identically on the slider. In practice, Harsh grain is already intense at low values while Fine grain needs to be pushed higher before it's visible. The per-type `amount_curve` exponent reshapes the slider response:

- Fine (0.7): more gradual ramp, grain stays subtle through the mid-range
- Silver (0.6): balanced response
- Harsh (0.5): grain kicks in earlier, reaching full effect faster

The exponent is applied as `(amount / 100) ^ amount_curve`, so values < 1.0 make the curve concave (faster initial ramp).

### Why dynamic luma falloff

With a fixed luma falloff, high-amount grain was overly concentrated in shadows — turning up the slider just made shadows grainier without spreading grain into midtones and highlights. Real pushed film shows grain across the entire tonal range.

The dynamic falloff reduces the base falloff exponent as amount increases:

```
effective_falloff = base_falloff * (1 - 0.4 * amount_factor)
```

At amount=0, the full base falloff applies (grain concentrated in shadows). At amount=100, falloff is reduced by 40% — for Fine (base 2.5), this drops to 1.5, spreading grain well into midtones. For Harsh (base 0.8), it drops to 0.48, putting grain almost everywhere. This makes the amount slider feel like "pushing" the film harder, which matches how real film behaves at higher ISO or longer development.

### Why additive/multiplicative blending

Pure exponential modulation (`pixel * exp(noise * scale)`) is imperceptible on near-black pixels. A pixel at 0.01 brightened by 5% becomes 0.0105 — invisible. Yet real film shows prominent grain in deep shadows as bright specks from sparse developed silver halide crystals on a dark base.

An earlier approach used a "shadow floor" (`max(pixel_in, 0.05)`) as the multiplicative base. This made shadow grain visible but didn't match film physics — it produced symmetric brightening/darkening rather than the bright-speck character of real shadow grain.

The current approach blends two grain modes:

- **Additive grain** (luma < 0.1): `delta = noise * weight * scale * 0.35`. Adds brightness directly, producing visible bright specks on dark pixels. The 0.35 scale factor prevents additive grain from being perceptually stronger than the multiplicative mode.
- **Multiplicative grain** (luma > 0.2): `delta = pixel * (exp(noise * weight * scale) - 1)`. Proportional density modulation, perceptually correct for well-exposed areas.
- **Transition** (luma 0.1–0.2): smoothstep blend between the two modes for artifact-free crossover.

This matches how real film grain behaves: shadows show bright specks (sparse crystal development), while highlights show both bright and dark modulation (dense emulsion with proportional density variation).

### Why contrast was removed from noise generation

The original implementation scaled the Gaussian noise by `grain_type.contrast` during generation (producing noise with std dev = contrast). The same contrast value was then multiplied into the scale factor during application: `scale = contrast * strength_mult * amount_factor`. This double-counted contrast — Fine (0.95) got 0.95^2 = 0.9x effective strength, while Harsh (1.5) got 1.5^2 = 2.25x.

The fix: generate standard normal noise (std dev 1) and apply contrast exactly once in the scale calculation. This makes the contrast parameter work as documented — a direct multiplier on grain intensity.

### Why shadow-boosted chromatic divergence

Real film grain shows different behavior across the tonal range. In highlights and midtones, grain manifests primarily as luminance texture (brightness variation). In shadows, where luminance changes from grain are less visible (both because the pixel is dark and because human vision has lower contrast sensitivity at low luminance), the grain instead manifests as color fringing between emulsion layers.

The shadow boost factor (`2 - luma`, ranging from 1.0 to 2.0) doubles the effective chromatic divergence in pure black compared to pure white. Combined with the existing saturation gating (`pixel_chroma`), this means: grayscale pixels never get color shift, saturated highlights get moderate color shift, and saturated shadows get the strongest color shift — matching how real film chromatic grain behaves.

## Memory Profile

Grain allocates single-channel f32 noise buffers:

- 4 noise buffers (1 shared + 3 per-channel) are always generated and optionally blurred
- Peak memory: ~5 buffers during blur passes, 4 after
- Per buffer at 24MP: ~92MB

The per-channel buffers are always allocated even when chromatic intensity is low, because the chromatic gating is per-pixel (based on saturation and shadow boost). All buffers are freed immediately after the grain step completes.

## Related Documents

- [Original grain design](../plans/2026-03-23-grain-design.md) — initial simplex noise approach
- [Grain size fix design](../plans/2026-03-27-grain-size-fix-design.md) — white noise + blur rework
- [Chromatic grain design](../plans/2026-03-29-chromatic-grain-design.md) — type-driven chromatic
- [Processing parity backlog](../backlog/processing-parity.md) — grain size bug tracking
