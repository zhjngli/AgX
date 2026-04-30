# Render Performance Analysis

**Date:** 2026-04-01
**Branch:** `perf/render-optimization`

## Methodology

Profiled the AgX render pipeline using feature-gated `std::time::Instant` instrumentation. Each image x preset combination was run 3 times in release mode; median timings reported below. Profiling overhead is negligible (nanosecond-resolution timer reads between pipeline stages).

**Test matrix:** 5 images (3 RAW 6246x4170, 1 JPEG 6240x4160, 1 PNG 1024x684) x 6 presets + 2 noop baselines = 32 combinations x 3 repetitions = 96 total renders.

**Hardware:** Apple Silicon (local dev machine), single-threaded render pipeline.

## Summary of Findings

### Full-resolution images (~26MP, 6246x4170)

| Preset type | Total render time | Dominant stages |
|-------------|-------------------|-----------------|
| Heavy (dune: dehaze+detail+grain) | 15-17.5s | dehaze 27%, per_pixel 29%, grain 17-19% |
| Detail-heavy (blade_runner: detail+grain) | 13-16.5s | detail 33-41%, per_pixel 22-28%, grain 17-21% |
| Denoise (portra_400, cinema_warm) | 11-15s | denoise 24-32%, per_pixel 26-33%, grain 19-24% |
| Light (neo_noir: grain only) | 9-11s | per_pixel 31-44%, grain 27-33%, decode 24-25% |
| Noop (decode+encode only) | 4s | decode 64%, encode 15% |

### Small images (1024x684 PNG)

| Preset type | Total render time |
|-------------|-------------------|
| Heavy (dune) | 322ms |
| Detail-heavy (blade_runner) | 323ms |
| Light (neo_noir) | 172ms |
| Noop | 49ms |

## Pipeline Stage Breakdown

### Top bottlenecks by frequency

| Stage | Appears as >15% in | Avg % when bottleneck | Notes |
|-------|--------------------|-----------------------|-------|
| **linear_to_srgb_and_per_pixel** | 27/32 combos | 31% | Always significant. Single loop doing gamma conversion + contrast/highlights/shadows/whites/blacks + tone curves + HSL + color grading + LUT |
| **grain** | 28/32 combos | 22% | Active in all presets except noop. Includes noise generation, Gaussian blur, per-pixel application |
| **decode** | 19/32 combos | 22% | Dominant for RAW (2.5-2.7s). Negligible for JPEG/PNG |
| **detail** | 10/32 combos | 41% | When active, it's the single biggest stage. Includes Gaussian blur + unsharp mask for sharpening/clarity/texture |
| **denoise** | 10/32 combos | 28% | A trous wavelet decomposition — expensive buffer-level pass |
| **dehaze** | 5/32 combos | 29% | Dark channel + guided filter — expensive buffer-level pass |

### Per-stage timing at 26MP

| Stage | Typical time | Notes |
|-------|-------------|-------|
| decode (RAW) | 2.5-2.7s | LibRaw decompression, fixed cost per image |
| decode (JPEG) | 0.6s | Standard JPEG decompression |
| decode (PNG) | 14ms | Already decompressed, just pixel format conversion |
| white_balance_exposure | 73-83ms | Simple channel multipliers, fast |
| dehaze | 4.5-4.8s | Dark channel prior + guided filter, O(n) but large constant |
| denoise | 3.5-3.7s | A trous wavelet, 4-5 iterations with buffer copies |
| linear_to_srgb_and_per_pixel | 1.2-5.1s | Varies with number of active adjustments. Includes gamma conversion, all per-pixel adjustments, and LUT lookup |
| detail | 0.4-5.8s | Varies with sharpening/clarity/texture amounts. Multiple Gaussian blur passes |
| grain | 2.7-3.3s | White noise generation + Gaussian blur + per-pixel application. 4 buffers (shared + 3 per-channel) |
| vignette_and_srgb_to_linear | 0.2-0.5s | Position-dependent darkening + gamma conversion back |
| encode (PNG) | 0.6-0.7s | PNG compression + metadata embedding |

## Key Observations

### 1. The per-pixel loop is always expensive

`linear_to_srgb_and_per_pixel` is a bottleneck in 27/32 combos. It combines many operations in a single loop: sRGB gamma conversion (pow/cbrt), contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading, and LUT. Even for the simplest presets, this loop touches every pixel with at least the gamma conversion + one or two adjustments.

**Why it's slow:** Each pixel goes through 10+ conditional branches and multiple floating-point operations. The loop is purely sequential — no SIMD, no parallelism.

### 2. Buffer-level passes dominate when active

Detail, dehaze, denoise, and grain all use buffer-level passes with Gaussian blur. When multiple are active (e.g., dune preset: dehaze + detail + grain), buffer operations account for 50-65% of total render time.

**Why they're slow:** Each Gaussian blur is a separable two-pass operation over the entire image. Detail does multiple blurs (one per sharpening component). Grain blurs 4 noise buffers (shared + 3 per-channel). Denoise does 4-5 iterations of wavelet decomposition.

### 3. RAW decode is a fixed overhead

RAW decode via LibRaw takes 2.5-2.7s regardless of preset. For heavy presets this is 15% of total; for light presets it's 25%. This is external C library code — optimization is limited to LibRaw configuration flags.

### 4. Grain is always present and always expensive

Grain appears as a bottleneck in 28/32 combos. At 26MP, it consistently takes 2.7-3.3s. The main cost is the 4 Gaussian blur passes (one per noise buffer channel). The per-pixel application loop is comparatively fast.

### 5. Encode is not a bottleneck

PNG encoding is consistently 0.6-0.7s (4-5% of total). Not worth optimizing.

## Prioritized Optimization Roadmap

### Priority 1: Parallelize per-pixel operations (estimated: 3-5x speedup for per_pixel stage)

**What:** Use rayon to parallelize the `linear_to_srgb_and_per_pixel` loop. Each pixel is independent — no data dependencies between pixels.

**Impact:** This stage is 25-44% of total time in most combos. A 4x speedup (on 4+ core machines) would reduce a 5s stage to ~1.3s, saving 3-4s on heavy presets.

**Complexity:** Low. Replace `for y in 0..h` with rayon `par_chunks_mut` or similar. All per-pixel functions are pure (no mutable shared state).

### Priority 2: Parallelize Gaussian blur (estimated: 3-5x speedup for detail/grain/dehaze blur)

**What:** The separable Gaussian blur in `detail.rs` is used by detail, grain, and dehaze. Parallelize the horizontal and vertical passes with rayon.

**Impact:** Gaussian blur is the core cost of detail (33-46% of total when active), grain (17-27%), and dehaze (27-34%). A 4x speedup on blur alone would reduce blade_runner from 15s to ~10s.

**Complexity:** Medium. The horizontal pass can be parallelized by rows. The vertical pass can be parallelized by columns. Need to ensure the temp buffer access pattern is safe.

### Priority 3: Parallelize denoise wavelet passes

**What:** The a trous wavelet decomposition in `denoise.rs` processes each pixel independently per iteration.

**Impact:** Denoise is 24-32% of total when active. Parallelizing the per-iteration pixel loop would cut denoise time by 3-5x.

**Complexity:** Medium. Each wavelet iteration reads from the previous buffer and writes to a new one — classic map pattern, parallelizable per-pixel.

### Priority 4: Parallelize grain noise generation and application

**What:** White noise generation is embarrassingly parallel (each pixel is independent). The per-pixel grain application loop is also independent.

**Impact:** Grain is 17-33% of total. Noise generation is fast; the blur is the bottleneck (covered in Priority 2). The application loop could save 0.3-0.5s.

**Complexity:** Low. Noise generation is pure; application loop reads from noise buffer and writes to sRGB buffer.

### Priority 5: SIMD for per-pixel adjustments

**What:** Vectorize the inner per-pixel loop with explicit SIMD (e.g., `std::arch::x86_64` SSE/AVX or portable SIMD via `std::simd` nightly).

**Impact:** Could provide an additional 2-4x on top of parallelization for the per-pixel loop. Would stack with Priority 1.

**Complexity:** High. Requires rewriting per-pixel math in SIMD intrinsics or using auto-vectorization-friendly patterns. sRGB gamma (pow) is the hardest part — would need a fast approximation.

### Priority 6: GPU acceleration (compute shaders)

**What:** Offload per-pixel and buffer operations to the GPU via wgpu compute shaders.

**Impact:** Could provide 10-100x speedup for embarrassingly parallel operations. Would reduce 26MP renders from seconds to milliseconds.

**Complexity:** Very high. Requires wgpu dependency, shader compilation, CPU-GPU data transfer management, fallback path for headless environments. Significant architectural change.

### Not recommended

- **Optimizing encode:** Only 4-5% of total, diminishing returns.
- **Optimizing RAW decode:** External library (LibRaw), limited control. Could explore multi-threaded LibRaw builds but marginal gain.
- **Algorithmic changes to dehaze/denoise:** These are well-known algorithms with inherent O(n) cost. Parallelization (Priorities 2-3) is the right approach.

## Recommended Next Steps

1. Start with **Priority 1** (parallelize per-pixel loop) — lowest complexity, highest frequency bottleneck, uses rayon which is already a dependency in agx-cli.
2. Follow with **Priority 2** (parallelize Gaussian blur) — addresses the core cost shared by detail, grain, and dehaze.
3. These two changes alone should reduce typical render times by 50-70% on multi-core machines.
4. Re-profile after implementing Priorities 1-2 to determine if further optimization is needed.

## References

- [Profiling design doc](2026-03-31-render-performance-profiling-design.md)
- [Detail pass implementation](../../crates/agx/src/adjust/detail.rs)
- [Grain algorithm explanation](../../docs/book/src/explanation/algorithms/grain.md)
- [Denoise implementation](../../crates/agx/src/adjust/denoise.rs)
- [Dehaze implementation](../../crates/agx/src/adjust/dehaze.rs)
