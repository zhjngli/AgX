# adjust

## Purpose

Pure per-pixel math functions for photo adjustments, operating on individual `f32` channel values.

## Working space

The engine runs stages 1–3 in linear Rec.2020 and stages 5–8 in gamma Rec.2020 (the sRGB transfer curve applied to Rec.2020 linear values). Per-channel adjustments here are gamut-agnostic — they operate at fixed perceptual anchors (0.5 midpoint, 0.25/0.75 splits) whose meaning is preserved across the move from sRGB to Rec.2020 because the same sRGB transfer curve shape is reused. Callers (the engine) are responsible for converting at the right pipeline slot.

## Public API

- `linear_to_srgb(r, g, b)` / `srgb_to_linear(r, g, b)` -- color space conversion via `palette`
- `exposure_factor(stops)` -- compute multiplier (2^stops)
- `apply_exposure(value, factor)` -- multiply a linear channel value
- `apply_white_balance(r, g, b, temperature, tint)` -- channel multipliers in linear space, normalized to preserve brightness
- `apply_contrast(value, contrast)` -- S-curve around 0.5 midpoint (gamma Rec.2020 working space)
- `apply_highlights(value, highlights)` -- targets pixels > 0.5 (gamma Rec.2020 working space)
- `apply_shadows(value, shadows)` -- targets pixels < 0.5 (gamma Rec.2020 working space)
- `apply_whites(value, whites)` -- targets pixels > 0.75 (gamma Rec.2020 working space)
- `apply_blacks(value, blacks)` -- targets pixels < 0.25 (gamma Rec.2020 working space)

All tone functions operate on a single channel and return an `f32`. Aesthetic output clamps were removed in the wide-working-space migration so wide-gamut headroom survives stage chaining; the final clamp to display gamut happens at encode. Domain-safety clamps (e.g., LUT-index, denoise weight) are kept and noted at their call sites.

### HSL Adjustments

- `hue_distance(a, b)` -- shortest angular distance between two hue angles in degrees
- `cosine_weight(hue_dist, half_width)` -- cosine falloff weight function for HSL channel targeting
- `apply_hsl(r, g, b, hue_shifts, saturation_shifts, luminance_shifts, weight_fn)` -- per-channel HSL adjustment in gamma Rec.2020 working space
- `WeightFn` -- type alias for pluggable weight functions: `fn(f32, f32) -> f32`

### Vignette

- `VignetteShape` -- enum: `Elliptical` (default, aspect-matched) or `Circular` (image-circle)
- `apply_vignette(r, g, b, amount, shape, x, y, w, h)` -- position-dependent edge darkening/brightening using power-curve falloff (gamma Rec.2020 working space)

### Noise Reduction

- `NoiseReductionParams` -- luminance (0-100), color (0-100), detail (0-100) noise reduction parameters
- `apply_noise_reduction(pixels, width, height, params)` -- à trous wavelet denoising in YCbCr space (linear buffer-level pass)

### Grain

- `GrainType` -- enum: `Fine`, `Silver`, `Harsh`
- `GrainParams` -- grain_type, amount (0-100), size (0-100), optional seed
- `apply_grain_buffer(buf, width, height, params, seed)` -- blur-based grain applied to gamma Rec.2020 buffer (buffer-level when size >= threshold, per-pixel otherwise)

### Buffer-Level Functions

- `apply_white_balance_exposure_buffer(buf, temperature, tint, exposure)` -- WB + exposure on a linear buffer in-place
- `PerPixelParams` -- struct holding all per-pixel adjustment parameters for the gamma Rec.2020 pass
- `apply_per_pixel_adjustments(buf, params)` -- all gamma Rec.2020 per-pixel adjustments (contrast through LUT) on a buffer in-place
- `apply_vignette_buffer(buf, width, height, precomputed)` -- position-dependent vignette on a gamma Rec.2020 buffer in-place

## Extension Guide

1. Add a new `pub fn apply_foo(value: f32, amount: f32) -> f32` here.
2. Add a `foo` field to `Parameters` in `engine/mod.rs`.
3. Call `apply_foo` at the correct pipeline position in `Engine::render()`.
4. Add the field to the preset TOML section structs and mapping in `preset/mod.rs`.

## Does NOT

- Perform file I/O.
- Know about presets, the engine, or the rendering pipeline order.

**Note:** Some submodules (`detail`, `dehaze`, `denoise`, `grain`) operate on full image buffers rather than individual pixel values. These are still pure math with no I/O or pipeline awareness. Buffer-level functions (`apply_per_pixel_adjustments`, `gaussian_blur`, `convolve_horizontal`, `convolve_vertical`, `apply_grain_buffer`, `apply_dehaze`) use rayon for data-parallel processing. Denoise additionally parallelizes across Y/Cb/Cr channels.

## Key Decisions

- **Stateless functions, not methods.** Each function takes scalar inputs and returns scalar outputs. The engine decides iteration order and pipeline sequencing.
- **Color space documented per function.** Exposure and white balance operate in linear space; contrast through blacks operate in gamma Rec.2020 working space. Callers (the engine) are responsible for converting at the right point.
