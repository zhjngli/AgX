# engine/gpu

## Purpose

This directory hosts the GPU render pipeline for AgX. It owns the wgpu runtime,
shader compilation, parameter upload layout, and stage dispatch code that runs
WGSL compute shaders over GPU-side image buffers.

The CPU adjustment path in `../../adjust/` remains the canonical source of
truth for output correctness. The CPU pipeline in `../pipeline.rs` and
`../stages/` orchestrates that math in the fixed render order. GPU rendering is
an opt-in acceleration path via `Engine::new_gpu_auto()`, `Engine::new_gpu()`,
or `agx-cli --gpu`; it is not the default pipeline. Any GPU result that differs
materially from the CPU result is a GPU bug unless the CPU behavior is
deliberately changed first.

The GPU pipeline follows the same fixed stage order as the CPU pipeline:

1. Linear adjustments: white balance and exposure.
2. Dehaze.
3. Denoise.
4. Linear to sRGB conversion.
5. Gamma-space adjustments: contrast, tone ranges, tone curves, HSL, color
   grading, and LUT.
6. Detail.
7. Grain.
8. Vignette.
9. sRGB to linear conversion.

## Dual-path principle

Algorithm math is documented once in the `adjust/` sibling `.md` files. The CPU
Rust functions and GPU WGSL shaders are two implementations of the same math,
not two independent definitions of behavior.

When an algorithm changes:

- Update the canonical explanation next to the CPU adjustment module.
- Update the CPU implementation first.
- Update the matching WGSL implementation to preserve the same math and stage
  order.
- Add or update cross-path consistency coverage in
  `crates/agx/tests/gpu_consistency.rs`.

The GPU code may use different execution structure for performance, especially
for separable filters and multi-pass reductions. It must still produce output
within the consistency tolerance for the same `Parameters`, optional LUT, and
input image.

## Related algorithm explanations

- [Exposure](../../adjust/exposure.md)
- [White balance](../../adjust/white_balance.md)
- [Basic tone](../../adjust/basic_tone.md)
- [Tone curves](../../adjust/tone_curves.md)
- [HSL](../../adjust/hsl.md)
- [Color grading](../../adjust/color_grading.md)
- [Vignette](../../adjust/vignette.md)
- [Dehaze](../../adjust/dehaze.md)
- [Denoise](../../adjust/denoise.md)
- [Detail](../../adjust/detail.md)
- [Grain](../../adjust/grain.md)

## Parameter layout

`GpuParameters` in `params.rs` is the flat `#[repr(C)]` Rust struct uploaded to
the GPU. WGSL shaders declare a matching `Params` struct and bind it as storage
data. The layout is intentionally plain: `f32`, fixed-size arrays, explicit
padding, no enums, no `Option`, and no pointers.

The current full layout is 400 bytes. Offsets below are byte offsets from the
start of `GpuParameters`; every group starts on a 16-byte boundary unless noted.
WGSL definitions may stop after the last field a shader reads, but any field
they include must preserve this prefix exactly.

| Field group | Rust fields | WGSL fields | Offset bytes | Layout notes |
|---|---|---|---:|---|
| Linear adjustments | `exposure`, `temperature`, `tint`, `_pad0` | same | 0 | One 16-byte lane. `_pad0` keeps following groups aligned. |
| Gamma adjustments | `contrast`, `highlights`, `shadows`, `whites`, `blacks`, `_pad1` | same | 16 | `blacks` starts the next lane at byte 32; `_pad1` fills bytes 36-47. |
| HSL | `hue_shifts`, `sat_shifts`, `lum_shifts` | same | 48 | Three `array<f32, 8>` blocks, 32 bytes each. `hsl_active` lives with vignette flags at byte 232. |
| Color grading | `cg_shadow_tint`, `cg_midtone_tint`, `cg_highlight_tint`, `cg_global_tint`, `cg_balance_factor`, `cg_balance_active`, `cg_active`, `_pad2` | `vec4f` wheels plus scalar flags | 144 | Rust `[f32; 4]` maps to WGSL `vec4f`. Four wheels occupy bytes 144-207; flags occupy 208-223. |
| Vignette | `vignette_amount`, `vignette_shape`, `hsl_active`, `_pad3` | same | 224 | `vignette_shape`: `0.0` elliptical, `1.0` circular. `hsl_active` is here to reuse the same lane. |
| Dehaze amount | `dehaze_amount`, `_pad4` | same | 240 | User-facing amount lane. Multi-pass dehaze scratch fields are later. |
| Grain | `grain_amount`, `grain_size`, `grain_type`, `grain_seed` | same | 256 | `grain_type`: `0.0` fine, `1.0` silver, `2.0` harsh. Seed is set during pipeline execution. |
| Tone curves and LUT | `tc_rgb_active`, `tc_luma_active`, `tc_red_active`, `tc_green_active`, `tc_blue_active`, `lut_active`, `_pad_tc` | same | 272 | Active flags are scalar floats. Curve data is not in `Params`; it is uploaded to `tone_curve_buffer` as `5 * 256` floats. |
| Image dimensions | `width`, `height`, `_pad5` | same | 304 | Dimensions are stored as `f32` because the shared struct is float-only. Convert in WGSL where integer indexing is needed. |
| Detail | `detail_strength`, `detail_threshold`, `detail_masking`, `kernel_size` | same | 320 | Set per dispatch by `gpu/stages/detail.rs`; kernel weights live in `kernel_buffer`. |
| Denoise | `nr_luminance`, `nr_color`, `nr_detail`, `nr_channel`, `nr_gap`, `nr_threshold`, `nr_is_luma`, `_pad_nr` | same | 336 | Per-dispatch fields. `nr_channel`, `nr_gap`, `nr_threshold`, and `nr_is_luma` change across wavelet/channel passes. |
| Dehaze multi-pass | `dehaze_airlight_r`, `dehaze_airlight_g`, `dehaze_airlight_b`, `dehaze_omega`, `dehaze_filter_radius`, `dehaze_mode`, `_pad_dh` | same | 368 | Per-dispatch fields. `dehaze_mode` is intentionally multi-purpose for min filters, box filters, and final recovery/fog passes. |

Layout guardrails:

- Keep `GpuParameters` `bytemuck::Pod`; if that derive fails, the layout is not
  safe to upload.
- Preserve the `std::mem::size_of::<GpuParameters>() % 16 == 0` invariant.
- Add padding explicitly rather than relying on implicit Rust or WGSL layout.
- Use the same field order in every WGSL `Params` prefix.
- Update every shader that declares fields after the insertion point.
- Prefer appending fields at the end of a 16-byte lane or adding a new lane over
  inserting into the middle of the struct.

## Adding a new adjustment to both paths

1. Implement the Rust-side adjustment in `../../adjust/<name>.rs` and add the
   sibling `../../adjust/<name>.md` as the canonical algorithm explanation.
2. Register the user-facing parameter in `engine::Parameters`, plus the matching
   partial parameter and preset materialization path if the adjustment is
   preset-addressable.
3. Add a mirror field in `GpuParameters`. Watch 16-byte alignment, update
   padding deliberately, and keep the struct `Pod`.
4. Add the matching field to every WGSL `Params` definition that needs the new
   field or lies after the insertion point.
5. Write WGSL shader code under `../../shaders/`. Non-common WGSL files must use
   the structured five-line header convention: `Algorithm`, `Canonical
   explanation`, `CPU equivalent`, `Bindings`, and `Entry points`.
6. Add the GPU pipeline stage or extend the existing stage dispatcher in
   `stages/`, including bind groups, buffer uploads, scratch buffers, and
   dispatch ordering.
7. Add a `gpu_consistency` test comparing CPU and GPU output within tolerance
   for focused parameters and at least one combined-pipeline case.
8. Update e2e presets, LUT coverage, and golden files according to the
   `feedback_e2e_with_features` policy whenever the new adjustment affects
   user-visible output or preset compatibility.

Do not document GPU-specific math separately from the adjustment explanation
unless the difference is an implementation detail needed to maintain or debug
the shader. The shared algorithm page explains what the operation means; this
README explains how to keep the two implementations synchronized.

## Debugging the GPU path

Use the opt-in GPU path explicitly:

```bash
RUST_LOG=agx::engine::gpu=debug agx --gpu apply --input in.jpg --preset look.toml --output out.png
```

The most useful places to inspect are:

- `runtime.rs`: adapter/device limits, buffer allocation, upload, and download.
- `shaders.rs`: naga_oil module composition and WGSL compilation failures.
- `stages/dispatch.rs`: generic bind group construction and workgroup counts.
- Stage dispatchers in `stages/`: per-pass parameter mutation and scratch-buffer
  sequencing.
- `crates/agx/tests/gpu_consistency.rs`: focused CPU-vs-GPU repro cases.

Buffer readback pointers:

- Full image readback is `GpuRuntime::download_pixels()`, which copies
  `pixel_buffer` into `staging_buffer`, maps the staging buffer, and casts bytes
  back to `Vec<[f32; 3]>`.
- Single-channel debug/readback is `GpuRuntime::download_single_channel()`. It is
  currently used by dehaze airlight estimation and denoise sigma estimation.
- Readbacks are synchronization points. They are acceptable for correctness or
  small intermediate reductions, but avoid adding them to hot paths without a
  measured reason.

llvmpipe and CI gotchas:

- Software adapters are much slower than hardware adapters and expose tighter
  buffer limits.
- Large images may exceed `max_buffer_size` or
  `max_storage_buffer_binding_size`; `GpuRuntime::new_inner()` checks the
  effective limit before allocating the pixel buffer.
- Keep GPU tests small. Existing consistency tests use 16x16 or 32x32 fixtures
  so they can run on local machines and software adapters when available.
- A passing CPU e2e run does not exercise the GPU path. Use `--gpu` or
  `cargo test -p agx --features gpu --test gpu_consistency` when validating GPU
  changes locally.

## Known limitations

- GPU consistency tests require an available adapter and return early when none
  exists. The CI coverage gap is tracked in the
  [performance backlog](../../../../../docs/backlog/performance.md).
- The GPU path is not the default pipeline. `Engine::new()` uses CPU rendering
  for deterministic output; callers must opt in with `Engine::new_gpu_auto()`,
  `Engine::new_gpu()`, or `agx-cli --gpu`.
- Some stages still use CPU readbacks for intermediate reductions. These are
  correctness-oriented synchronization points, not proof that the GPU path is
  fully GPU-resident.
- Floating-point differences are expected at very small magnitudes. Treat the
  CPU path as canonical and use `gpu_consistency` tolerances to decide whether a
  difference is acceptable.
