# Grain Simulation Design

**Date:** 2026-03-23
**Status:** Implemented — see the grain explanation in the mdbook for algorithm details
**Category:** Editing — Grain simulation

## Problem

Film grain simulation adds organic texture that mimics analog film stocks. It's one of the most popular creative effects in photo editing — grain gives digital images a tactile, analog quality. AgX currently has no grain capability, and presets cannot express this common creative effect.

## Goals

- Add film grain simulation with amount, size, grain type, and chromatic controls
- Support grain types matching industry-standard options (Capture One's six types)
- Grain is non-deterministic for users (different pattern each render) but deterministic in tests via explicit seed
- Integrate into the preset system and CLI
- Per-pixel operation using spatially coherent simplex noise, applied inline in the render loop (like vignette)

## Non-Goals

- Film emulation database (curated Portra/Tri-X/etc. profiles) — separate future idea. Film emulation profiles are presets combining existing features (LUT + tone curve + grain) and can be authored once grain exists.
- Physics-based Monte Carlo grain simulation — academically accurate but computationally infeasible (see Alternatives Considered).

## Alternatives Considered

### Simplex Noise (chosen)

2D simplex noise sampled per-pixel, layered with 2-3 octaves for natural variation. The noise function is spatially coherent — neighboring pixels get correlated values based on their (x, y) coordinates, producing clumpy, organic texture rather than random static. The `size` parameter controls sampling frequency; `amount` controls blend strength.

This is what the photo editing industry uses in practice. Darktable's grain module uses simplex noise on the L channel in Lab color space. Lightroom and Capture One don't publish their algorithms, but given they require real-time preview performance, they almost certainly use procedural noise (likely simplex or similar) with careful parameterization.

**Pros:** Per-pixel (no buffer pass), fast, well-understood, maps naturally to size/amount/type parameters.
**Cons:** Not physically accurate to real silver halide crystal behavior. Darktable's own community has an [open issue](https://github.com/darktable-org/darktable/issues/4451) calling their simplex approach "seemingly hacky." However, the proposed replacement (Monte Carlo) proved impractical.

### Filtered White Noise (rejected)

Generate per-pixel white noise from a PRNG, then apply a Gaussian blur kernel to control grain size.

**Rejected because:** Requires a buffer-level blur pass to control spatial correlation, making it a neighborhood operation. More expensive and architecturally heavier for what simplex noise achieves per-pixel. The blur approach is sometimes used in video pipelines but is not standard in photo editors.

### Spectral / FFT-based (rejected)

Generate white noise, transform to frequency domain via FFT, shape the power spectrum to control grain character, then inverse FFT back to spatial domain. This gives more precise control over grain texture than simplex.

**Rejected because:** Requires an FFT library dependency and a buffer-level pass. The additional precision over simplex noise is marginal for photographic grain simulation and not worth the architectural complexity.

### Monte Carlo / Physics-based (rejected)

The Newson et al. 2017 algorithm ([IPOL paper](https://www.ipol.im/pub/art/2017/192/)) simulates actual silver halide crystals as random circles placed on a plane using a Boolean model from stochastic geometry. Renders at any resolution with individual grains visible at zoom. This is the academic gold standard for film grain rendering.

**Rejected because:** Computationally infeasible for a photo editor. A darktable contributor [tested it](https://github.com/darktable-org/darktable/issues/4451) on a 1920x1280 image and it ran for 20+ hours without completing. Even with proposed optimizations (20x iteration reduction + OpenMP parallelization for ~100x speedup), it would still be orders of magnitude too slow for batch processing.

## Design

### Parameters

```rust
pub enum GrainType {
    Fine,
    Silver,
    Soft,
    Cubic,
    Tabular,
    Harsh,
}

pub struct GrainParams {
    pub grain_type: GrainType,  // default: Silver
    pub amount: f32,            // 0-100, default: 0 (neutral)
    pub size: f32,              // 0-100, default: 50
    pub chromatic: f32,         // 0-100, default: 0 (luminance-only)
}
```

- `amount = 0` is the neutral/identity value (no grain applied)
- `grain_type` controls the internal character: octave weights, contrast curve, and luminance falloff strength
- `size` maps to simplex noise frequency — lower values = finer grain, higher = coarser. Frequency is scaled relative to image dimensions so grain looks consistent across different resolutions.
- `chromatic` controls the strength of per-channel color variation. When 0, grain affects luminance only. When > 0, three independent noise fields (one per R/G/B channel) are generated with offset seeds and blended proportionally.

### Grain Types

The six grain types match Capture One's model. Each type is a named combination of internal noise parameters — users select a type and adjust amount/size/chromatic on top.

| Type | Character | Octave profile | Luminance falloff | Real-world analog |
|------|-----------|---------------|-------------------|-------------------|
| Fine | Subtle, minimal texture | Low amplitude, high frequency bias | Strong | Slow-speed fine-grain film (Ektar 100) |
| Silver | Classic analog, balanced | Even octave weights | Medium | General-purpose film stock |
| Soft | Gentle, smooth texture | Emphasis on lower frequencies | Strong (grain fades in shadows/highlights) | Portrait film (Portra) |
| Cubic | Irregular, clumpy | Higher contrast between octaves | Medium-weak | Traditional cubic crystal emulsions (Tri-X) |
| Tabular | Uniform, modern | Narrow amplitude range, even distribution | Medium | Modern T-grain films (T-Max, Ilford Delta) |
| Harsh | High contrast, gritty | Strong high-frequency octave | Weak (grain visible everywhere) | Pushed high-ISO film |

Internally, each type maps to a configuration struct controlling:

- Octave count and relative weights
- Contrast/amplitude curve applied to the raw noise
- Luminance falloff curve shape and strength

### Chromatic Grain

The `chromatic` parameter is a strength slider (0-100), not a boolean, and is orthogonal to grain type. This gives 6 types x continuous chromatic range for expressive control.

When `chromatic > 0`, three independent noise fields are generated (one per R/G/B channel) using offset seeds derived from the base seed. Each channel gets a slightly different grain pattern, blended at `chromatic / 100.0` strength. This mimics how real color film works — each emulsion layer (sensitive to R/G/B) is a physically separate layer of silver halide crystals with its own independent grain structure.

All color film exhibits chromatic grain to some degree. It's especially visible in consumer stocks (Fuji Superia, Kodak Gold) and pushed high-ISO film, and very subtle in fine-grain stocks (Ektar). Black and white film has no chromatic grain (single emulsion layer). The chromatic slider lets users dial this in as desired.

### Luminance-Aware Blending

All grain types apply luminance-aware falloff — grain is strongest in midtones and reduced in deep shadows and blown highlights. This matches real film behavior: unexposed silver halide crystals don't produce visible grain in pure black areas, and fully saturated areas have uniform density that masks grain.

The falloff is baked in (not user-configurable) but varies per grain type:

- Types like `soft` and `fine` have strong falloff — grain fades significantly in shadows and highlights
- Types like `harsh` and `cubic` have weak falloff — grain remains visible across the full tonal range
- This is controlled by the internal per-type configuration, not exposed to users

**Blending math:** When `chromatic = 0`, a single noise value is computed from the pixel's (x, y) coordinates and added equally to all three RGB channels. This is a luminance-only effect in the sense that the same noise value shifts R, G, and B by the same amount. The noise value is scaled by both the `amount` parameter and a luminance-dependent weight derived from `Rec. 709 luma = 0.2126*R + 0.7152*G + 0.0722*B`. When `chromatic > 0`, the blend interpolates between the single shared noise value and three independent per-channel noise values, proportional to `chromatic / 100.0`.

Output values are clamped to [0.0, 1.0] after grain application to prevent out-of-range values from noise added to pixels near 0.0 or 1.0.

### Validation

All numeric parameters are validated in the preset module:

- `amount`: must be in range 0.0-100.0
- `size`: must be in range 0.0-100.0
- `chromatic`: must be in range 0.0-100.0
- `grain_type`: must be a valid enum variant (validated by serde deserialization)

Out-of-range values produce a preset validation error, consistent with the `validate_*_params` pattern used by other adjustments (dehaze, noise reduction, detail).

### Simplex Noise Implementation

Self-contained 2D simplex noise (~100 lines), no external crate dependency. The implementation uses:

- Standard simplex noise algorithm with gradient table and permutation table
- Permutation table seeded from a `u64` seed for deterministic-per-seed output
- Multi-octave layering: 2-3 octaves of noise at increasing frequencies, weighted by the grain type's octave profile
- Frequency scaling relative to image dimensions for resolution-independent grain appearance

### Determinism and Seeding

**User-facing behavior:** Non-deterministic. Each render generates a fresh random seed, producing a different grain pattern. This matches Lightroom and Capture One, where the grain pattern varies between exports. The visual character (size, roughness, intensity) is identical — only the specific spatial pattern differs.

**Internal API:** `apply_grain` takes an explicit `seed: u64` parameter. The engine generates a random seed at render time via `rand::thread_rng().gen::<u64>()`. Test code passes fixed seeds for reproducible assertions and golden file comparison.

**Why non-deterministic:** Real film grain genuinely varies frame to frame. Users never compare two exports pixel-by-pixel. Industry standard editors (LR, C1) are also non-deterministic — they simply don't pin the seed. If deterministic output were needed in the future, a seed parameter could be exposed in presets.

### Pipeline Position

The current AgX pipeline order in the per-pixel render loop is:

```
WB -> exposure -> dehaze (buffer) -> NR (buffer) -> sRGB conversion ->
contrast -> highlights -> shadows -> whites -> blacks -> tone curves -> HSL ->
color grading -> LUT -> vignette -> (buffer) detail pass -> linear conversion
```

Grain is inserted **after the detail pass buffer and before vignette**, within the per-pixel loop over the detail pass output. The updated end of the pipeline becomes:

```
... -> LUT -> vignette -> detail pass (buffer) -> grain -> linear conversion
```

Wait — to achieve grain-before-vignette (industry standard), we need to adjust the current ordering. Currently vignette is applied per-pixel *before* the detail buffer pass. To place grain after detail but before vignette, vignette must move to after the detail pass as well. The updated pipeline tail becomes:

```
... -> LUT -> detail pass (buffer) -> grain -> vignette -> linear conversion
```

This moves vignette from its current position (per-pixel, before detail buffer) to after the detail buffer pass. This is a minor pipeline reorder — vignette is a position-dependent darkening/brightening effect that is order-independent with respect to detail (sharpening doesn't interact with vignette). The reorder is justified by the grain placement requirement and aligns with darktable's ordering (sharpen → grain → vignette).

Grain is applied:

- **After detail pass (sharpening/clarity):** You don't want sharpening to amplify grain. Both Lightroom and darktable confirm this ordering — Lightroom explicitly documents that "grain is overlaid over the sharpened image." Darktable's default pipeline order is: sharpen → grain → soften → vignette.
- **Before vignette:** This is the industry standard (both darktable and Lightroom). Vignette darkening naturally reduces grain visibility in corners. Requires moving vignette after the detail buffer pass (see above).
- **In sRGB gamma space:** Grain is a perceptual effect applied after tonal adjustments. The luminance-aware falloff math is simpler in gamma space where perceptual brightness is roughly linear.

#### Grain Before Vignette: Physics vs. Aesthetics

The choice to apply grain before vignette is an aesthetic/industry-standard decision, not a physics-accurate one. The physics is nuanced:

- **Optical vignette (lens-caused):** Less light reaches the film edges → less exposure → silver halide crystals are less saturated → grain is actually *more visible* in vignetted areas. Physics would suggest amplifying grain in dark corners, the opposite of what grain-before-vignette achieves.
- **Darkroom vignette (dodge/burn during printing):** This happens after the film is developed. The grain pattern is baked into the negative. Dodging/burning changes print brightness but doesn't change the grain structure — grain is independent of darkroom vignette.

Neither physical scenario matches "grain before vignette" where vignette suppresses grain. However, AgX's vignette is a creative effect, not a lens simulation. The industry applies grain before vignette because it looks clean aesthetically — heavy grain in dark corners can be distracting. The luminance-aware falloff within the grain algorithm itself already provides natural grain reduction in very dark areas, which approximates the darkroom printing experience.

### Architecture

**New file:** `crates/agx/src/adjust/grain.rs`

Following the same pattern as other adjust functions. Pure pixel math, no I/O. Contains:

- `GrainType` enum with serde support
- `GrainParams` struct with `is_neutral()` (true when `amount == 0`)
- Internal `GrainTypeConfig` struct mapping each type to octave weights, contrast curve, and luminance falloff
- `simplex_noise_2d(x, y, perm)` — core noise function
- `GrainPrecomputed::new(params, seed, width, height)` — precomputed struct holding the seeded permutation table and per-type config
- `apply_grain_pixel(r, g, b, x, y, pre) -> (f32, f32, f32)` — per-pixel function applied inline in the render loop (like vignette), not a buffer-level pass. Output is clamped to [0.0, 1.0].

**Module dependencies:** Adds `rand` crate to `crates/agx/Cargo.toml` for seed generation (`rand::thread_rng().gen::<u64>()`). This is the only new external dependency.

**Engine integration:**

- `grain` field on `Parameters` with `Default` returning neutral (amount=0)
- `PartialGrainParams` following the merge/materialize/From pattern for preset composability
- Applied per-pixel in the post-detail-pass loop, after detail and before vignette (see Pipeline Position for the vignette reorder)
- `GrainPrecomputed` constructed at the top of `render()` when grain is active, with a random seed from `rand`

**Preset support:**

```toml
[grain]
type = "silver"
amount = 50
size = 50
chromatic = 25
```

All fields optional with serde defaults. Missing `[grain]` section = no grain.

**CLI flags:** `--grain-type`, `--grain-amount`, `--grain-size`, `--grain-chromatic`

**Re-exports:** `GrainType` and `GrainParams` from `crate::adjust`, `PartialGrainParams` from `crate::engine`.

### Testing Strategy

**Unit tests (fixed seed):**

- Default params are neutral / `is_neutral()` checks
- Simplex noise spatial coherence — neighboring pixels produce correlated values
- Grain type differentiation — different types produce measurably different output (variance, frequency)
- Luminance-aware falloff — grain reduced at luminance extremes (0.0, 1.0), strongest at midtones
- Chromatic mode — `chromatic > 0` produces different noise per R/G/B channel; `chromatic = 0` produces identical noise across channels
- Amount=0 is identity; higher amount = more pixel variance
- Size scaling — different size values produce different spatial frequencies
- Resolution awareness — grain character consistent across different image dimensions

**E2E golden tests (multiple fixed seeds):**

- Each grain e2e preset specifies multiple fixed seeds (3-5) in its TOML configuration
- The e2e harness renders one golden per (image, look, seed) combination and validates against all of them
- This requires extending the e2e framework: the `run_image_matrix` helper needs to iterate over a preset's seed list, generating and comparing a golden for each seed. Golden file naming adds a seed suffix, e.g. `temple_blossoms_grain_silver_seed1.png`, `temple_blossoms_grain_silver_seed2.png`.
- Two look presets: `grain_silver` (silver, amount=40, size=50) and `grain_harsh_chromatic` (harsh, amount=70, size=60, chromatic=40)
- Standard golden comparison across all test images (JPEG + RAW matrix)
- With 3 seeds x 2 presets x 6 images = 36 additional golden files (~20MB of PNGs). This is acceptable for the coverage it provides — multiple seeds verify that the algorithm produces valid grain across different random states, not just one lucky seed.

**Internal API design for testability:** `apply_grain_pixel` takes coordinates and a `GrainPrecomputed` struct (which holds the seeded permutation table). The engine generates a random seed at render time. Test code and e2e fixtures construct `GrainPrecomputed` with fixed seeds. This separates the non-deterministic user behavior from deterministic test infrastructure.

**Note on e2e seed mechanism:** The e2e look presets include a `seeds` array field (e.g. `seeds = [42, 137, 9001]`). The e2e harness iterates this array, rendering and comparing one golden per seed. Production presets and CLI usage do not use this field — the engine always generates a random seed. The `seeds` field is exclusively for e2e testing.

## References

- [Darktable grain module](https://docs.darktable.org/usermanual/4.8/en/module-reference/processing-modules/grain/) — simplex noise on L channel, coarseness + strength controls
- [Darktable default module order](https://docs.darktable.org/usermanual/3.8/en/special-topics/module-order/) — sharpen → grain → soften → vignette → dither
- [Darktable issue #4451: physically-realistic grain](https://github.com/darktable-org/darktable/issues/4451) — discussion of Monte Carlo replacement, found impractical (20+ hours for single image)
- [Newson et al. 2017: Realistic Film Grain Rendering (IPOL)](https://www.ipol.im/pub/art/2017/192/) — physics-based Boolean model, academic gold standard, computationally infeasible for interactive editing
- [Newson et al. 2017: Stochastic Film Grain Model (CGF)](https://onlinelibrary.wiley.com/doi/abs/10.1111/cgf.13159) — resolution-independent rendering variant
- [Film Grain Rendering and Parameter Estimation (ACM 2023)](https://dl.acm.org/doi/10.1145/3592127) — parameter estimation from scanned film
- [Film grain simulation rabbit hole (36Exp)](https://teaandtechtime.com/down-the-film-grain-simulation-rabbit-hole-for-36exp/) — practical developer experience with various approaches
