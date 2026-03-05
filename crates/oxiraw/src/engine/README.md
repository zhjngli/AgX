# engine

## Purpose
Hold the immutable original image and mutable parameters, and render the final output by applying all adjustments from scratch on each call.

## Public API
- `Parameters` -- all adjustment fields (`exposure`, `contrast`, `highlights`, `shadows`, `whites`, `blacks`, `temperature`, `tint`)
- `Engine::new(image)` -- create engine with a linear sRGB `Rgb32FImage` and neutral parameters
- `Engine::original()` -- reference to the unmodified source image
- `Engine::params()` / `Engine::params_mut()` -- read/write current parameters
- `Engine::set_params(params)` -- replace all parameters
- `Engine::lut()` / `Engine::set_lut(lut)` -- read/write the optional 3D LUT
- `Engine::apply_preset(preset)` -- replace parameters and LUT from a `Preset`
- `Engine::render()` -- apply the full pipeline, returning a new `Rgb32FImage`

## Extension Guide
To add a new adjustment:
1. Add a field to `Parameters` (with `Default` returning the neutral value).
2. Add the adjustment function in `adjust/mod.rs`.
3. Insert the call at the correct position in `Engine::render()`. The pipeline order is:
   white balance (linear) -> exposure (linear) -> sRGB conversion -> contrast -> highlights -> shadows -> whites -> blacks -> LUT -> linear conversion.

## Does NOT
- Perform file I/O (decoding or encoding).
- Define adjustment algorithms (delegates to `adjust` module).
- Own the pipeline across multiple renders -- each `render()` is independent.

## Key Decisions
- **Always re-render from original.** `render()` starts from `self.original` every time. This makes the system order-independent from the user's perspective and eliminates accumulated rounding errors.
- **Fixed internal pipeline order.** The render order is chosen for correctness (e.g., white balance and exposure in linear space, tone curves in gamma space, LUT last before final conversion). Users cannot reorder steps.
- **Output is linear sRGB.** The rendered image is returned in linear space; the encode module handles gamma conversion for output files.
