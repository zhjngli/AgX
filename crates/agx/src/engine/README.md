# engine

## Purpose

Hold the immutable original image and mutable parameters, and render the final output by executing a fixed pipeline of stages.

## Architecture

The engine has two render pipelines that execute the same stages in the same order:

- **CPU pipeline** (`pipeline.rs`) — Rust + rayon. Each stage implements the `Stage` trait and processes a shared pixel buffer in-place.
- **GPU pipeline** (`gpu/`) — wgpu + WGSL compute shaders. Each stage dispatches compute passes on GPU-side buffers. Enabled by the `gpu` feature (on by default).

`Engine::new()` always uses the CPU pipeline — this is the canonical path for deterministic output across all platforms. `Engine::new_gpu_auto()` tries GPU first and falls back to CPU (opt-in via `--gpu` CLI flag). `Engine::new_gpu()` forces GPU-only and returns `Err` if unavailable (useful for profiling and testing).

### Working space contract

The engine's working space is linear Rec.2020. Decode delivers buffers in linear Rec.2020; encode expects them in linear Rec.2020 and converts to the chosen output gamut. The buffer always enters the pipeline as linear Rec.2020, and the executor converts it back to linear Rec.2020 after the last active stage if needed.

Color-space conversions between stages are **auto-inserted by the CPU executor** based on each stage's declared `input_color_space` / `output_color_space`. No hand-placed conversion stages exist in the stage list. The single conversion primitive is `crate::color_space::convert_buffer(buf, from, to)`, which routes through the linear Rec.2020 hub; all stage-to-stage conversions go through it. Never hand-place a conversion call inside a stage's `process` method.

Wide-gamut inputs (Display P3 HEIC) survive end-to-end; the final clamp to display gamut happens only at encode.

### Pipeline Order (fixed, not configurable)

The stage list contains 8 real stages. Color-space conversions are auto-inserted between them by the CPU executor as needed; the GPU pipeline uses a fused hand-ordered equivalent (see CPU/GPU asymmetry below).

1. WhiteBalanceExposure (linear Rec.2020)
2. Dehaze (linear Rec.2020)
3. Denoise (linear Rec.2020)
4. PerPixelAdjustments (gamma Rec.2020) — contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading
5. LutStage (color space declared by the LUT's encoding field: `SrgbGamma` for `Srgb` LUTs, `LinearSrgb` for `Linear` LUTs; inactive when no LUT is set)
6. Detail (gamma Rec.2020)
7. Grain (gamma Rec.2020)
8. Vignette (gamma Rec.2020)

After the last active stage, the executor converts back to linear Rec.2020 if the buffer is not already there. Engine output is always linear Rec.2020.

### CPU/GPU Asymmetry

The CPU executor auto-inserts conversions from declared stage color spaces. The GPU pipeline uses a fused, hand-ordered LUT bracket (encoding-aware) instead of the executor/auto-insert approach — it does not share the CPU stage list or executor. This asymmetry is intentional: the CPU path is the pluggable reference implementation; the GPU path is an optimized mirror that matches pixel output but maintains its own sequencing.

### CPU Stage Trait

```rust
/// Read-only inputs a stage may consult to decide activity and color space.
pub struct StageInputs<'a> {
    pub params: &'a Parameters,
    pub lut: Option<&'a crate::lut::Lut3D>,
}

pub trait Stage: Send + Sync {
    fn name(&self) -> &'static str;
    fn input_color_space(&self, inp: &StageInputs) -> ColorSpace;
    fn output_color_space(&self, inp: &StageInputs) -> ColorSpace;
    fn is_active(&self, inp: &StageInputs) -> bool;
    fn prepare(&mut self, inp: &StageInputs);
    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError>;
}
```

All four query methods (`input_color_space`, `output_color_space`, `is_active`, `prepare`) take `&StageInputs` so a stage can depend on both parameters and the LUT. The executor constructs a `StageInputs` once per render and passes it to every query call. `prepare` precomputes loop-invariant data; `process` operates on the buffer in place.

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
- `ColorSpace` -- enum: `LinearRec2020`, `GammaRec2020`, `LinearSrgb`, `SrgbGamma` (the first two are the pipeline working spaces; `LinearSrgb` is the space for `Linear`-encoded LUTs and encode-side intermediates; `SrgbGamma` is the space for `Srgb`-encoded LUTs and the sRGB final output step)
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

1. Create `crates/agx/src/engine/stages/my_stage.rs` implementing the `Stage` trait. Declare the stage's working color space via `input_color_space` and `output_color_space` — the executor auto-inserts conversions, so no hand-placed conversion calls are needed in `process`.
2. Add the stage's pixel math as a buffer-level function in the `adjust` module.
3. Add the stage to the fixed list in `CpuPipeline::new()` at the correct position.
4. Re-export from `stages/mod.rs`.
5. Write a WGSL compute shader in `src/shaders/` and a dispatcher in `gpu/stages/`.
6. Add the stage dispatch to `GpuPipeline::execute()` at the matching position, including any conversion brackets the GPU path needs explicitly.
7. Add a cross-path consistency test in `tests/gpu_consistency.rs`.

**LUT encoding note.** The `LutStage` declares its color space from the LUT's `encoding` field: `SrgbGamma` for the `Srgb` variant (the default for `.cube` files authored in the sRGB-gamma domain), `LinearSrgb` for the `Linear` variant. The executor auto-inserts the conversion bracket. If a LUT format with a different primary set ever needs to be supported, add a new `ColorSpace` variant and extend `convert_buffer` with its hub hops.

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
