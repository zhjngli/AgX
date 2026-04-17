# engine

## Purpose
Hold the immutable original image and mutable parameters, and render the final output by executing a fixed pipeline of stages.

## Architecture

The engine has two render pipelines that execute the same stages in the same order:

- **CPU pipeline** (`pipeline.rs`) — Rust + rayon. Each stage implements the `Stage` trait and processes a shared pixel buffer in-place.
- **GPU pipeline** (`gpu/`) — wgpu + WGSL compute shaders. Each stage dispatches compute passes on GPU-side buffers. Enabled by the `gpu` feature (on by default).

`Engine::new()` tries GPU first and falls back to CPU if initialization fails (no adapter, buffer too large). This is transparent to consumers — `Engine::render()` returns the same `RenderResult` either way. `Engine::new_cpu()` and `Engine::new_gpu()` force a specific path for profiling or testing.

### Pipeline Order (fixed, not configurable)

1. WhiteBalanceExposure / linear_adjustments (linear sRGB)
2. Dehaze (linear sRGB)
3. Denoise (linear sRGB)
4. LinearToSrgb (conversion)
5. PerPixelAdjustments / gamma_adjustments (sRGB gamma) — contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading, LUT
6. Detail (sRGB gamma)
7. Grain (sRGB gamma)
8. Vignette (sRGB gamma)
9. SrgbToLinear (conversion)

### CPU Stage Trait

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

### GPU Pipeline

The GPU pipeline (`gpu/mod.rs`) owns a `GpuRuntime` (device, queue, buffers) and a `ShaderCache` (compiled WGSL compute pipelines). Each stage is a dispatcher function in `gpu/stages/` that creates bind groups and dispatches compute passes. Multi-pass stages (dehaze, denoise, detail, grain) manage their own sequencing internally.

Key GPU submodules:
- `gpu/runtime.rs` — wgpu device, queue, buffer allocation, upload/download
- `gpu/shaders.rs` — compile and cache WGSL compute pipelines via naga_oil
- `gpu/params.rs` — `GpuParameters` Pod struct mirroring `Parameters` for uniform upload
- `gpu/stages/` — per-stage compute dispatchers

## Public API
- `Parameters` -- all adjustment fields
- `VignetteParams` -- vignette parameters: `amount` (f32) and `shape` (`VignetteShape`)
- `PartialParameters` -- partial parameter set for preset composability
- `ColorSpace` -- enum: `LinearSrgb`, `SrgbGamma`
- `Engine::new(image)` -- create engine (tries GPU, falls back to CPU)
- `Engine::new_cpu(image)` -- force CPU pipeline
- `Engine::new_gpu(image)` -- force GPU pipeline (returns `Err` if unavailable)
- `Engine::pipeline_name()` -- returns `"gpu"` or `"cpu"`
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
3. Add the stage to the fixed list in `CpuPipeline::new()` at the correct position.
4. Re-export from `stages/mod.rs`.
5. Write a WGSL compute shader in `src/shaders/` and a dispatcher in `gpu/stages/`.
6. Add the stage dispatch to `GpuPipeline::execute()` at the matching position.
7. Add a cross-path consistency test in `tests/gpu_consistency.rs`.

To add a new per-pixel adjustment (within the existing PerPixelAdjustments stage):
1. Add the adjustment function in `adjust/mod.rs`.
2. Add a field to `Parameters` and `GpuParameters`.
3. Add the call in `adjust::apply_per_pixel_adjustments()` at the correct position.
4. Add the logic to the `gamma_adjustments.wgsl` shader.
5. Add the field to preset TOML structs in `preset/mod.rs`.

## Does NOT
- Perform file I/O (decoding or encoding).
- Define adjustment algorithms (delegates to `adjust` module).
- Allow pipeline reordering — the fixed order is an invariant that preserves preset compatibility.

## Key Decisions
- **Always re-render from original.** `render()` starts from `self.original` every time.
- **Fixed internal pipeline order.** The render order is hardcoded. Consumers cannot reorder stages.
- **Output is linear sRGB.** The rendered image is returned in linear space.
- **CPU stages delegate to adjust.** CPU stages own orchestration; `adjust` owns the math.
- **GPU stages are self-contained WGSL.** GPU shaders reimplement the same algorithms in WGSL. The `adjust` module is not used by the GPU path.
- **Runtime pipeline selection.** GPU is preferred; CPU is the automatic fallback. Transparent to consumers.
- **Profiling is built into both executors.** Each stage is automatically timed when the `profiling` feature is enabled.
