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
- `GrainType` -- enum: `Fine`, `Silver`, `Soft`, `Cubic`, `Tabular`, `Harsh`
- `GrainParams` -- grain_type, amount (0-100), size (0-100), chromatic (0-100), optional seed
- `GrainPrecomputed::new(params, seed, width, height)` -- precomputed struct for per-pixel grain
- `apply_grain_pixel(r, g, b, x, y, pre)` -- per-pixel grain via simplex noise (sRGB gamma space)

## Extension Guide
1. Add a new `pub fn apply_foo(value: f32, amount: f32) -> f32` here.
2. Add a `foo` field to `Parameters` in `engine/mod.rs`.
3. Call `apply_foo` at the correct pipeline position in `Engine::render()`.
4. Add the field to the preset TOML section structs and mapping in `preset/mod.rs`.

## Does NOT
- Hold or iterate over image buffers.
- Perform file I/O.
- Know about presets, the engine, or the rendering pipeline order.

## Key Decisions
- **Stateless functions, not methods.** Each function takes scalar inputs and returns scalar outputs. The engine decides iteration order and pipeline sequencing.
- **Color space documented per function.** Exposure and white balance operate in linear space; contrast through blacks operate in sRGB gamma space. Callers (the engine) are responsible for converting at the right point.
