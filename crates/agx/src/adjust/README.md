# adjust

## Purpose
Pure per-pixel math functions for photo adjustments, operating on individual `f32` channel values.

## Public API
- `linear_to_srgb(r, g, b)` / `srgb_to_linear(r, g, b)` -- color space conversion via `palette`
- `exposure_factor(stops)` -- compute multiplier (2^stops)
- `apply_exposure(value, factor)` -- multiply a linear channel value
- `apply_white_balance(r, g, b, temperature, tint)` -- channel multipliers in linear space, normalized to preserve brightness
- `apply_contrast(value, contrast)` -- S-curve around 0.5 midpoint (sRGB gamma space)
- `apply_highlights(value, highlights)` -- targets pixels > 0.5 (sRGB gamma space)
- `apply_shadows(value, shadows)` -- targets pixels < 0.5 (sRGB gamma space)
- `apply_whites(value, whites)` -- targets pixels > 0.75 (sRGB gamma space)
- `apply_blacks(value, blacks)` -- targets pixels < 0.25 (sRGB gamma space)

All tone functions operate on a single channel and return a clamped `f32`.

### HSL Adjustments
- `hue_distance(a, b)` -- shortest angular distance between two hue angles in degrees
- `cosine_weight(hue_dist, half_width)` -- cosine falloff weight function for HSL channel targeting
- `apply_hsl(r, g, b, hue_shifts, saturation_shifts, luminance_shifts, weight_fn)` -- per-channel HSL adjustment in sRGB gamma space
- `WeightFn` -- type alias for pluggable weight functions: `fn(f32, f32) -> f32`

### Vignette
- `VignetteShape` -- enum: `Elliptical` (default, aspect-matched) or `Circular` (image-circle)
- `apply_vignette(r, g, b, amount, shape, x, y, w, h)` -- position-dependent edge darkening/brightening using power-curve falloff (sRGB gamma space)

### Noise Reduction
- `NoiseReductionParams` -- luminance (0-100), color (0-100), detail (0-100) noise reduction parameters
- `apply_noise_reduction(pixels, width, height, params)` -- à trous wavelet denoising in YCbCr space (linear buffer-level pass)

### Grain
- `GrainType` -- enum: `Fine`, `Silver`, `Harsh`
- `GrainParams` -- grain_type, amount (0-100), size (0-100), optional seed
- `apply_grain_buffer(buf, width, height, params, seed)` -- blur-based grain applied to sRGB gamma buffer (buffer-level when size >= threshold, per-pixel otherwise)

### Buffer-Level Functions
- `apply_white_balance_exposure_buffer(buf, temperature, tint, exposure)` -- WB + exposure on a linear buffer in-place
- `PerPixelParams` -- struct holding all per-pixel adjustment parameters for the sRGB gamma pass
- `apply_per_pixel_adjustments(buf, params)` -- all sRGB gamma-space per-pixel adjustments (contrast through LUT) on a buffer in-place
- `apply_vignette_buffer(buf, width, height, precomputed)` -- position-dependent vignette on an sRGB gamma buffer in-place

## Extension Guide
1. Add a new `pub fn apply_foo(value: f32, amount: f32) -> f32` here.
2. Add a `foo` field to `Parameters` in `engine/mod.rs`.
3. Call `apply_foo` at the correct pipeline position in `Engine::render()`.
4. Add the field to the preset TOML section structs and mapping in `preset/mod.rs`.

## Does NOT
- Perform file I/O.
- Know about presets, the engine, or the rendering pipeline order.

**Note:** Some submodules (`detail`, `dehaze`, `denoise`, `grain`) operate on full image buffers rather than individual pixel values. These are still pure math with no I/O or pipeline awareness. Buffer-level functions (`apply_per_pixel_adjustments`, `gaussian_blur`, `convolve_horizontal`, `convolve_vertical`, `apply_grain_buffer`) use rayon for data-parallel processing. Denoise additionally parallelizes across Y/Cb/Cr channels.

## Key Decisions
- **Stateless functions, not methods.** Each function takes scalar inputs and returns scalar outputs. The engine decides iteration order and pipeline sequencing.
- **Color space documented per function.** Exposure and white balance operate in linear space; contrast through blacks operate in sRGB gamma space. Callers (the engine) are responsible for converting at the right point.
