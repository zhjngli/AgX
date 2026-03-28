# Performance Optimizations

Potential performance improvements identified during the cleanup/simplify review (2026-03-18). None have been profiled yet — each should be benchmarked before implementation.

## Render Loop Parallelization (rayon)

**Impact:** Potentially 4-16x on multi-core machines for single-image rendering.

The per-pixel render loop in `Engine::render()` is embarrassingly parallel. Adding `rayon` to the core `agx` crate would allow `par_iter` over scanlines or pixel chunks.

**Trade-offs:**
- Adds `rayon` as a hard dependency to the library crate (currently only `agx-cli` depends on it for batch parallelism).
- Library consumers who don't want rayon would need a feature flag.
- Batch processing already parallelizes across images via rayon in the CLI, so the incremental benefit depends on the workload (many small images vs few large images).

## Inline sRGB Transfer Functions

**Impact:** Unclear without profiling. Likely small in release builds with LTO.

Replace `palette` crate's `Srgb`/`LinSrgb` type wrappers with inline IEC 61966-2-1 transfer functions for the per-pixel hot path.

**Trade-offs:**
- `palette` provides type safety (`LinSrgb` vs `Srgb` at compile time) and spec-correct conversions maintained upstream.
- In release mode, the compiler likely inlines palette's conversion functions already since they're small and marked `#[inline]`.
- Risk of introducing subtle color accuracy bugs if hand-rolling the spec.

## Decode Buffer Reduction

**Impact:** ~1 image buffer worth of memory saved during decode.

Currently decode allocates an intermediate sRGB f32 buffer, then converts pixel-by-pixel into a linear f32 buffer. Could convert in-place or use a single pass.

## Encode Buffer Reduction

**Impact:** ~1-2 image buffers worth of memory saved during encode.

`linear_to_srgb_dynamic` creates an intermediate `Rgb32FImage` in sRGB, which is then converted to `DynamicImage`, then to `Rgb8`. Could go directly from linear f32 to u8 sRGB in a single pass.

## Consolidate Dual BatchOpts

**Impact:** Code quality, not performance.

The CLI has `BatchOpts` (clap args struct) and `batch::BatchOpts` (internal struct). These could potentially be unified, though they serve different roles (user-facing args vs internal config).
