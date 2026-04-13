# GPU Acceleration via wgpu + WGSL Compute Shaders

**Date:** 2026-04-13
**Status:** Approved
**Backlog:** [Performance Optimizations — P7](../backlog/performance.md)

## Problem

AgX's render pipeline is CPU-bound. Parallelization with rayon (P1–P5) delivered significant speedups, but the remaining bottleneck is fundamental: CPU cores max out at ~8–16 parallel threads, while photo editing math — particularly per-pixel adjustments and separable filters — is embarrassingly parallel across millions of pixels. GPUs offer thousands of execution units purpose-built for this workload.

## Goal

Port all adjustment math from Rust (CPU) to WGSL compute shaders (GPU) via the wgpu library, while keeping the existing CPU path as a feature-gated fallback. Same public API, near-identical output (sub-pixel tolerance), dramatically faster renders on GPU-equipped hardware.

## Non-goals

- Dropping the CPU path (deferred to follow-up profiling work)
- Automatic GPU→CPU fallback at runtime (deferred — compile-time feature gate for now)
- Golden file regeneration (deferred to follow-up)
- Documentation strategy for WGSL code (deferred to follow-up)
- Browser/WebGPU target (wgpu supports it, but not a goal of this design)

## Approach

**Composable shader modules, dispatched per stage.** WGSL shaders are organized as shared utility functions (`common/color.wgsl`, `common/tone.wgsl`) composed into per-stage shaders. Each stage maps to one or more GPU dispatches. Utility functions are imported via `naga_oil`, which provides a `#import` mechanism for WGSL.

This approach was chosen over:
- **Monolithic shaders** (one huge `.wgsl` per stage) — harder to maintain and test.
- **One shader per adjustment** (separate `contrast.wgsl`, `hsl.wgsl`, etc.) — too many dispatches, unnecessary overhead.

## Architecture

### Two pipeline executors

The engine selects between two pipeline executors at compile time via a Cargo feature gate.

```
                        Engine
                          |
                +---------+---------+
                |                   |
          CpuPipeline          GpuPipeline
        (existing rayon)      (new, wgpu)
                |                   |
        Stage::process()     GpuStage::dispatch()
        mutates Vec<[f32;3]>  runs WGSL on GPU buffer
                |                   |
                +---------+---------+
                          |
                    RenderResult
                  (same either way)
```

- `CpuPipeline`: today's `Pipeline`, renamed. All existing code unchanged.
- `GpuPipeline`: new executor. Uploads pixels to GPU once, dispatches all stages as compute shaders, downloads result once.

The `Engine` public API is unchanged. Callers (`agx-cli`, `agx-e2e`) do not know which path runs.

### Feature gate

```toml
# crates/agx/Cargo.toml
[features]
default = ["gpu"]
gpu = ["dep:wgpu", "dep:naga_oil", "dep:bytemuck"]
```

- `gpu` enabled (default): `Engine::new()` creates `GpuPipeline`.
- `gpu` disabled (`--no-default-features`): `Engine::new()` creates `CpuPipeline`.

### GPU runtime

`GpuRuntime` manages all wgpu resources, created once per `Engine` and reused across renders.

```
GpuRuntime
  device: wgpu::Device           -- GPU handle
  queue: wgpu::Queue             -- command submission
  pixel_buffer: wgpu::Buffer     -- GPU-side pixel data (read/write storage)
  params_buffer: wgpu::Buffer    -- GPU-side uniform (Parameters)
  staging_buffer: wgpu::Buffer   -- for reading pixels back to CPU
  compiled_shaders: HashMap<&str, wgpu::ComputePipeline>
```

**Render lifecycle:**

1. **Upload:** CPU `Vec<[f32; 3]>` → `pixel_buffer`; `Parameters` → `params_buffer`; optional `Lut3D` → 3D texture.
2. **Dispatch:** For each active stage, submit a compute pass. Pixels stay on GPU between stages.
3. **Download:** `pixel_buffer` → `staging_buffer` → CPU `Vec<[f32; 3]>`.

Buffer sizes are determined by image dimensions at engine construction. For a 26MP image: `width * height * 3 * 4 bytes` = ~300MB for the pixel buffer.

Shaders are compiled once at `GpuRuntime` construction (or lazily on first render) and cached as `wgpu::ComputePipeline` objects. No per-render compilation cost.

### Render flow

```rust
impl GpuPipeline {
    fn render(&self, original: &Rgb32FImage, params: &Parameters, lut: Option<&Lut3D>)
        -> Result<RenderResult, AgxError>
    {
        self.runtime.upload_pixels(original);
        self.runtime.upload_params(params);
        if let Some(lut) = lut {
            self.runtime.upload_lut(lut);
        }

        // Fixed stage order — same as CPU pipeline
        self.stages.linear_adjustments.dispatch(&self.runtime, params);
        self.stages.dehaze.dispatch(&self.runtime, params);
        self.stages.denoise.dispatch(&self.runtime, params);
        self.stages.linear_to_srgb.dispatch(&self.runtime);
        self.stages.gamma_adjustments.dispatch(&self.runtime, params);
        self.stages.detail.dispatch(&self.runtime, params);
        self.stages.grain.dispatch(&self.runtime, params);
        self.stages.vignette.dispatch(&self.runtime, params);
        self.stages.srgb_to_linear.dispatch(&self.runtime);

        let pixels = self.runtime.download_pixels();
        Ok(RenderResult { image: pixels_to_rgb32f(pixels) })
    }
}
```

Each stage checks `is_active` on the Rust side before dispatching — no empty GPU dispatches for inactive stages.

## WGSL shader structure

```
crates/agx/src/shaders/
  common/
    color.wgsl              -- linear<>srgb, luminance, HSL<>RGB conversions
    tone.wgsl               -- smoothstep, tone curve eval, highlights/shadows math
    math.wgsl               -- clamp helpers, interpolation utilities
    blur.wgsl               -- shared separable Gaussian blur (used by detail, grain)
  linear_adjustments.wgsl   -- white balance + exposure (linear sRGB space)
  gamma_adjustments.wgsl    -- contrast, H/S/W/B, tone curves, HSL, color grading, LUT (sRGB gamma space)
  dehaze.wgsl               -- dark channel, guided filter passes (linear sRGB)
  denoise.wgsl              -- a-trous wavelet decomposition (linear sRGB)
  detail.wgsl               -- Gaussian blur + sharpen/clarity/texture (sRGB gamma)
  grain.wgsl                -- noise generation + blur + luminance-modulated apply (sRGB gamma)
  vignette.wgsl             -- radial falloff (sRGB gamma)
  linear_to_srgb.wgsl       -- color space conversion
  srgb_to_linear.wgsl       -- color space conversion
```

### Stage naming

Stages are grouped by color space, not by photographic category:

- **`linear_adjustments`**: all operations in linear sRGB (white balance, exposure). Replaces `WhiteBalanceExposure`.
- **`gamma_adjustments`**: all per-pixel operations in sRGB gamma space (contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading, LUT). Replaces `PerPixelAdjustments`.

Both are per-pixel stages. The filter stages (dehaze, denoise, detail, grain) read neighboring pixels and require multi-dispatch patterns.

### Per-stage dispatch patterns

| Pipeline order | Stage | Shader | Dispatches | Pattern |
|---|---|---|---|---|
| 1 | linear_adjustments | `linear_adjustments.wgsl` | 1 | Per-pixel |
| 2 | Dehaze | `dehaze.wgsl` | ~6 | Multi-pass: dark channel (separable min filter), guided filter (box filters + per-pixel) |
| 3 | Denoise | `denoise.wgsl` | ~4-6 | Multi-pass: one dispatch per wavelet scale |
| 4 | LinearToSrgb | `linear_to_srgb.wgsl` | 1 | Per-pixel |
| 5 | gamma_adjustments | `gamma_adjustments.wgsl` | 1 | Per-pixel |
| 6 | Detail | `detail.wgsl` | 3 | Blur H + blur V + apply |
| 7 | Grain | `grain.wgsl` | ~4 | Noise gen + blur H + blur V + apply |
| 8 | Vignette | `vignette.wgsl` | 1 | Per-pixel |
| 9 | SrgbToLinear | `srgb_to_linear.wgsl` | 1 | Per-pixel |

Filter stages that read neighboring pixels use workgroup shared memory to cache pixel tiles, avoiding redundant global memory reads. The separable blur pattern (horizontal pass, then vertical pass) is shared between detail and grain via `common/blur.wgsl`.

### Composable shader example

```wgsl
// gamma_adjustments.wgsl
#import common::color
#import common::tone

struct Params {
    exposure: f32,
    temperature: f32,
    tint: f32,
    contrast: f32,
    highlights: f32,
    shadows: f32,
    whites: f32,
    blacks: f32,
    // ... remaining fields
}

@group(0) @binding(0) var<storage, read_write> pixels: array<vec3f>;
@group(0) @binding(1) var<uniform> params: Params;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) id: vec3u) {
    let idx = id.x;
    var rgb = pixels[idx];
    rgb = tone::apply_contrast(rgb, params.contrast);
    rgb = tone::apply_highlights_shadows(rgb, params.highlights, params.shadows);
    rgb = tone::apply_whites_blacks(rgb, params.whites, params.blacks);
    rgb = tone::apply_tone_curve(rgb, /* curve data */);
    rgb = color::apply_hsl(rgb, /* hsl params */);
    rgb = color::apply_color_grading(rgb, /* grading params */);
    // LUT applied via textureSample if active
    pixels[idx] = rgb;
}
```

Each adjustment is its own function, composable and testable. All run in a single dispatch — one pass over all pixels.

## Parameter and LUT transfer

### Parameters to GPU

`Parameters` is a Rust struct with floats, enums, and arrays. GPU needs flat bytes. Two-part solution:

1. **`GpuParameters`** — a `#[repr(C)]` struct with GPU-friendly layout (no enums, no Options, just `f32` and fixed-size arrays). Derives `bytemuck::Pod` and `bytemuck::Zeroable`.
2. **`From<&Parameters> for GpuParameters`** — field-by-field conversion, runs on CPU before upload.

The WGSL side declares a matching `Params` struct. Layout correspondence is enforced by either:
- **`wgsl_to_wgpu`** (preferred): auto-generates the Rust `GpuParameters` struct from the WGSL `Params` definition. WGSL becomes the source of truth for struct layout. Good foundation for future single-source documentation.
- **Manual + test**: maintain both structs, with a unit test asserting `std::mem::size_of::<GpuParameters>()` matches the expected WGSL layout size.

Evaluate `wgsl_to_wgpu` during implementation; fall back to manual if too brittle.

### LUT to 3D texture

The current `Lut3D` (`Vec<[f32; 3]>`, typically 33x33x33) maps to a `wgpu::Texture` with dimension `D3` and format `Rgba32Float` (padded to 4 channels — GPU prefers aligned reads). WGSL reads it with hardware-accelerated trilinear interpolation via `textureSampleLevel`, replacing the manual trilinear lookup in the CPU path.

```wgsl
@group(0) @binding(2) var lut_texture: texture_3d<f32>;
@group(0) @binding(3) var lut_sampler: sampler;

fn apply_lut(rgb: vec3f) -> vec3f {
    return textureSampleLevel(lut_texture, lut_sampler, rgb, 0.0).rgb;
}
```

## File layout

### New files

```
crates/agx/src/
  engine/
    gpu/                            -- NEW (all #[cfg(feature = "gpu")])
      mod.rs                        -- GpuPipeline executor
      runtime.rs                    -- GpuRuntime (device, queue, buffers)
      shaders.rs                    -- shader compilation + caching
      stages/
        mod.rs
        linear_adjustments.rs
        gamma_adjustments.rs
        dehaze.rs
        denoise.rs
        detail.rs
        grain.rs
        vignette.rs
  shaders/                          -- NEW (WGSL source files)
    common/
      color.wgsl
      tone.wgsl
      math.wgsl
      blur.wgsl
    linear_adjustments.wgsl
    gamma_adjustments.wgsl
    dehaze.wgsl
    denoise.wgsl
    detail.wgsl
    grain.wgsl
    vignette.wgsl
    linear_to_srgb.wgsl
    srgb_to_linear.wgsl
```

### Changed files

- `crates/agx/Cargo.toml` — add `gpu` feature and optional dependencies
- `crates/agx/src/engine/mod.rs` — delegate to pipeline via feature gate
- `crates/agx/src/engine/pipeline.rs` — renamed from current pipeline code
- `crates/agx/src/error.rs` — add `GpuInitFailed` error variant

### Unchanged

- `crates/agx/src/adjust/` — all CPU math stays as-is
- `crates/agx/src/engine/stages/` — all CPU stage implementations stay
- `crates/agx-cli/`, `crates/agx-e2e/`, `crates/agx-docgen/`, `crates/agx-lut-gen/` — no changes

## Testing

### Unit tests (per GPU stage)

Each `engine/gpu/stages/*.rs` file gets tests that create a small pixel buffer, upload, dispatch one stage, download, and assert against expected values. Gated on `#[cfg(feature = "gpu")]`.

### GPU availability detection

GPU tests use runtime detection to skip gracefully when no GPU adapter is available:

```rust
fn gpu_available() -> bool {
    pollster::block_on(async {
        let instance = wgpu::Instance::new(Default::default());
        instance.request_adapter(&Default::default()).await.is_some()
    })
}
```

Tests that require a GPU check `gpu_available()` and return early with a message if no adapter is found. This works in any CI environment without assumptions about hardware. Enabling a GPU-capable CI runner is recommended but not required.

### Cross-path consistency tests

A dedicated test module runs the same `Parameters` + input image through both `CpuPipeline` and `GpuPipeline`, asserting near-identical results. Per-channel tolerance: +-1/255 after conversion to u8. This is the primary correctness check.

### E2e tests

No changes to e2e test code or golden files. E2e runs whichever pipeline the feature gate selects. Golden file regeneration is deferred to follow-up work.

## Error handling

`GpuPipeline::new()` can fail if no GPU adapter or device is available. This propagates as `AgxError::GpuInitFailed(String)` with a description of what failed (adapter request, device creation, etc.). There is no automatic fallback to the CPU path — the user selects the path at compile time via the feature gate. Automatic fallback is a potential follow-up after profiling.

## New dependencies

| Crate | Version | Purpose |
|---|---|---|
| `wgpu` | 24 | GPU runtime (Vulkan, Metal, DX12, WebGPU backends) |
| `naga_oil` | 0.16 | WGSL `#import` composition |
| `bytemuck` | 1 | Safe cast of `#[repr(C)]` parameter structs to byte slices |
| `pollster` | 0.4 | Block on async wgpu calls (adapter/device creation) |

All optional, gated behind the `gpu` feature.

## Follow-up work

Each item below becomes its own brainstorm/design/implement cycle after this work lands.

### F1: GPU vs CPU profiling

Profile both paths on real images across representative hardware (M1 Pro, discrete GPU, integrated GPU). Measure per-stage timings, total render time, and upload/download overhead. Compare against the rayon CPU baseline. This data drives all subsequent decisions.

### F2: CPU path decision

Based on F1 profiling data, decide whether to keep both paths or drop the CPU path. If keeping both: evaluate automatic runtime fallback (GPU init fails -> fall back to CPU). If dropping CPU: remove the `adjust/` math duplication and make WGSL the sole source of truth for adjustment algorithms.

### F3: Documentation strategy

Depends on F2. If both paths are kept: document both, with shared concepts in `common/` WGSL files. If GPU-only: evaluate `wgsl_to_wgpu` as the documentation source of truth, update documentation initiative sub-projects 4+ to account for WGSL source files, and determine whether WGSL files need the same sibling `.md` pattern that `adjust/*.rs` uses today.

### F4: Golden file regeneration

Regenerate e2e golden files from the GPU path (or designate one path as canonical). Define tolerance policy: are GPU goldens the new baseline, or maintain CPU goldens and validate GPU output within tolerance?

### F5: Architecture and backlog updates

- Check off P7 in `docs/backlog/performance.md`
- Update `ARCHITECTURE.md` dependency graph and rules for `engine/gpu/` and `shaders/`
- Update engine `README.md` with GPU pipeline documentation

## Related

- [Parallel Render Design (P1+P2)](2026-04-02-parallel-render-design.md)
- [Parallel Render P3+P4 Design](2026-04-03-parallel-render-p3-p4-design.md)
- [Dehaze Parallelization Design](2026-04-05-dehaze-parallelization-design.md)
- [Pluggable Pipeline Design](2026-04-01-pluggable-pipeline-design.md)
- [Performance Optimizations Backlog](../backlog/performance.md)
- [Documentation Initiative Design](2026-04-06-documentation-initiative-design.md)
