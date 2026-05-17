# engine

## Purpose

Hold the immutable original image and mutable parameters, and render the final output by executing a fixed pipeline of stages.

## Architecture

The engine has two render pipelines that execute the same stages in the same order:

- **CPU pipeline** (`pipeline.rs`) — Rust + rayon. Each stage implements the `Stage` trait and processes a shared pixel buffer in-place.
- **GPU pipeline** (`gpu/`) — wgpu + WGSL compute shaders. Each stage dispatches compute passes on GPU-side buffers. Enabled by the `gpu` feature (on by default).

`Engine::new()` always uses the CPU pipeline — this is the canonical path for deterministic output across all platforms. `Engine::new_gpu_auto()` tries GPU first and falls back to CPU (opt-in via `--gpu` CLI flag). `Engine::new_gpu()` forces GPU-only and returns `Err` if unavailable (useful for profiling and testing).

### Working space contract

The engine's working space is linear Rec.2020. Decode delivers buffers in linear Rec.2020; encode expects them in linear Rec.2020 and converts to 8-bit sRGB at output. Stages 1–3 run in linear Rec.2020; stages 5–8 run in gamma-encoded Rec.2020 (the sRGB transfer curve applied to Rec.2020 linear values), reached via the sign-preserving transfer stages at slots 4 and 9. The LUT lookup in stage 5 wraps its sample in a gamma-sRGB conversion bracket so existing sRGB-authored `.cube` LUTs remain portable. Wide-gamut inputs (Display P3 HEIC) survive end-to-end; the final clamp to display gamut happens only at encode.

### Pipeline Order (fixed, not configurable)

1. WhiteBalanceExposure / linear_adjustments (linear Rec.2020)
2. Dehaze (linear Rec.2020)
3. Denoise (linear Rec.2020)
4. LinearToGamma (conversion: linear Rec.2020 → gamma Rec.2020 via sign-preserving sRGB curve)
5. PerPixelAdjustments / gamma_adjustments (gamma Rec.2020) — contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading, LUT (sampled via sRGB-gamma bracket)
6. Detail (gamma Rec.2020)
7. Grain (gamma Rec.2020)
8. Vignette (gamma Rec.2020)
9. GammaToLinear (conversion: gamma Rec.2020 → linear Rec.2020)

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
- `ColorSpace` -- enum: `LinearRec2020`, `GammaRec2020`, `LinearSrgb`, `SrgbGamma` (the first two are the engine's working spaces; the latter two are retained for encode-side intermediates and the LUT-wrap conversion bracket)
- `Engine::new(image)` -- create engine (always CPU, canonical path)
- `Engine::new_gpu_auto(image)` -- try GPU, fall back to CPU (opt-in)
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
- **Output is linear Rec.2020.** The rendered image is returned in the linear Rec.2020 working space; encode performs the final conversion to display gamut.
- **CPU stages delegate to adjust.** CPU stages own orchestration; `adjust` owns the math.
- **GPU stages are self-contained WGSL.** GPU shaders reimplement the same algorithms in WGSL. The `adjust` module is not used by the GPU path.
- **CPU is canonical.** CPU pipeline is the default for deterministic output. GPU is opt-in via `new_gpu_auto()` or `--gpu` CLI flag.
- **Profiling is built into both executors.** Each stage is automatically timed when the `profiling` feature is enabled.
