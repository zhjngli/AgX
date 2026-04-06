# Parallel Render Pipeline (P3 + P4)

**Date:** 2026-04-03
**Status:** Approved
**Backlog:** [Performance Optimizations](../backlog/performance.md)
**Prior work:** [P1+P2 Parallel Render Design](2026-04-02-parallel-render-design.md)

## Problem

After P1+P2, the remaining render bottlenecks on 26MP images are:

1. **Denoise** — 40% avg when active (~3.6s), the single biggest stage bottleneck
2. **Grain** — 26% avg across all presets (~1.9s), the most frequent bottleneck (30/32 combos)

Both contain embarrassingly parallel work but are currently single-threaded.

## Design

Extend the rayon parallelism added in P1+P2 to the denoise and grain stages. No new dependencies — rayon is already in the `agx` crate.

### P3: Parallelize denoise wavelet passes

**File:** `crates/agx/src/adjust/denoise.rs`

Two levels of parallelism:

**Level 1 — Convolution passes:** `convolve_horizontal` and `convolve_vertical` use the same `par_chunks_mut(width)` pattern as the Gaussian blur (P2). Each row writes to a disjoint output slice, reads from shared immutable input. These are the inner hot loops — called 30 times per render (5 levels x 2 passes x 3 channels).

**Level 2 — Channel parallelism:** The three `denoise_channel` calls (Y, Cb, Cr) are fully independent. Run them in parallel using `rayon::join` (or a parallel iterator collecting results). This eliminates sequential channel processing.

Channel parallelism triples peak memory during wavelet decomposition. At 26MP, each channel's decomposition allocates ~5 detail buffers + residual + temp buffers for convolutions (~600MB per channel, ~1.8GB total vs ~600MB sequential). This is acceptable for the typical desktop/server use case. The memory impact should be profiled under batch load — tracked in the existing "Batch memory pressure" backlog item.

### P4: Parallelize grain noise generation and application

**File:** `crates/agx/src/adjust/grain.rs`

Three parallelism opportunities:

**1. Noise buffer generation:** The 4 `generate_white_noise_buffer` calls (shared + R/G/B) are independent — each uses its own seed and allocates its own buffer. Run in parallel.

**2. Noise buffer blurring:** The 4 `blur()` calls are independent. Run in parallel. Each calls `gaussian_blur` which is already internally parallelized (P2), but rayon's work-stealing handles nesting gracefully.

**3. Per-pixel application loop:** The `for idx in 0..buf.len()` loop reads from 4 immutable noise buffers and writes to `buf[idx]`. Change to `par_chunks_mut` with index offset arithmetic, same pattern as P1.

### Supporting changes

- **`docs/backlog/performance.md`:** Check off P3 and P4. Add note under memory section about parallel channel denoising memory impact.
- **`crates/agx/src/adjust/README.md`:** Note denoise and grain parallelism.
- **`ARCHITECTURE.md`:** Add this design doc link.

## What doesn't change

- E2e golden files must be **bit-identical**. Denoise convolutions parallelize by rows (same computation order within each row). Grain uses seeded PRNG — each buffer is generated independently with its own seed, so parallelism doesn't affect output.
- No public API changes.
- No new dependencies.

## Expected impact

- **P3:** Convolution parallelism: 3-5x speedup on the convolution portion. Channel parallelism: up to 3x on top (all 3 channels run simultaneously). Combined: denoise stage from ~3.6s to ~0.5-1s.
- **P4:** Noise generation + blur parallelism saves wall-clock time on buffer setup. Per-pixel loop parallelism: same 7-12x as P1. Combined: grain stage from ~1.9s to ~0.5-1s.
- **Overall:** Further 20-30% reduction in total 26MP render time on presets that use denoise or grain (most presets).

## Verification

1. `./scripts/verify.sh` — format, clippy, unit tests, architecture tests
2. `./scripts/e2e-quick.sh` — golden file comparison (must be bit-identical)
3. Reprofile after implementation to measure actual gains
