# engine

## Purpose
Hold the immutable original image and mutable parameters, and render the final output by executing a fixed pipeline of stages.

## Architecture

The engine uses a stage-based pipeline. Each stage implements the `Stage` trait and processes a shared pixel buffer (`RenderContext`) in-place. The `Pipeline` executor runs stages in a fixed order, auto-inserts color space conversions between stages that disagree, and times each stage for profiling when the `profiling` feature is enabled.

### Pipeline Order (fixed, not configurable)

1. WhiteBalanceExposure (linear sRGB)
2. Dehaze (linear sRGB)
3. Denoise (linear sRGB)
4. LinearToSrgb (conversion)
5. PerPixelAdjustments (sRGB gamma) — contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading, LUT
6. Detail (sRGB gamma)
7. Grain (sRGB gamma)
8. Vignette (sRGB gamma)
9. SrgbToLinear (conversion)

### Stage Trait

```rust
pub trait Stage: Send + Sync {
    fn name(&self) -> &'static str;
    fn input_color_space(&self) -> ColorSpace;
    fn output_color_space(&self) -> ColorSpace;
    fn is_active(&self, params: &Parameters) -> bool;
    fn prepare(&mut self, params: &Parameters);
    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError>;
}
```

Stages declare their working color space. The executor skips inactive stages (where `is_active` returns false). `prepare` precomputes loop-invariant data; `process` operates on the buffer.

## Public API
- `Parameters` -- all adjustment fields
- `VignetteParams` -- vignette parameters: `amount` (f32) and `shape` (`VignetteShape`)
- `PartialParameters` -- partial parameter set for preset composability
- `ColorSpace` -- enum: `LinearSrgb`, `SrgbGamma`
- `Engine::new(image)` -- create engine with a linear sRGB `Rgb32FImage` and neutral parameters
- `Engine::original()` -- reference to the unmodified source image
- `Engine::params()` / `Engine::params_mut()` -- read/write current parameters
- `Engine::set_params(params)` -- replace all parameters
- `Engine::lut()` / `Engine::set_lut(lut)` -- read/write the optional 3D LUT
- `Engine::apply_preset(preset)` -- replace parameters and LUT from a `Preset`
- `Engine::layer_preset(preset)` -- layer a preset on top of current parameters
- `Engine::render()` -- execute the pipeline, returning `RenderResult`

## Extension Guide
To add a new pipeline stage:
1. Create `crates/agx/src/engine/stages/my_stage.rs` implementing the `Stage` trait.
2. Add the stage's pixel math as a buffer-level function in the `adjust` module.
3. Add the stage to the fixed list in `Pipeline::new()` at the correct position.
4. Re-export from `stages/mod.rs`.

To add a new per-pixel adjustment (within the existing PerPixelAdjustments stage):
1. Add the adjustment function in `adjust/mod.rs`.
2. Add a field to `Parameters`.
3. Add the call in `adjust::apply_per_pixel_adjustments()` at the correct position.
4. Add the field to preset TOML structs in `preset/mod.rs`.

## Does NOT
- Perform file I/O (decoding or encoding).
- Define adjustment algorithms (delegates to `adjust` module).
- Allow pipeline reordering — the fixed order is an invariant that preserves preset compatibility.

## Key Decisions
- **Always re-render from original.** `render()` starts from `self.original` every time.
- **Fixed internal pipeline order.** The render order is hardcoded in `Pipeline::new()`. Consumers cannot reorder stages.
- **Output is linear sRGB.** The rendered image is returned in linear space.
- **Stages delegate to adjust.** Stages own orchestration; `adjust` owns the math.
- **Profiling is built into the executor.** Each stage is automatically timed when the `profiling` feature is enabled. No per-stage instrumentation needed.
