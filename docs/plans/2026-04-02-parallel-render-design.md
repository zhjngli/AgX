# Parallel Render Pipeline (P1 + P2)

**Date:** 2026-04-02
**Status:** Approved
**Backlog:** [Performance Optimizations](../backlog/performance.md)

## Problem

Profiling shows that two stages dominate 26MP render times:

1. **Per-pixel adjustments** — #1 bottleneck, avg 31% of total (27/32 combos), ~3.7s on heavy presets
2. **Gaussian blur** — shared by detail, grain, and dehaze stages, 2-5s depending on active stages

Both are embarrassingly parallel but currently single-threaded.

## Design

Add `rayon` as a hard dependency to the `agx` crate and parallelize the two hot loops directly in the `adjust` module. No feature flag — rayon is already in the workspace (via `agx-cli`), and all current consumers benefit from parallelism. If a `no_std` or WASM target materializes, we can gate it then.

### P1: Parallelize per-pixel adjustment loop

**File:** `crates/agx/src/adjust/mod.rs` — `apply_per_pixel_adjustments`

The current loop iterates `buf.iter_mut()` sequentially. Each pixel is fully independent: no shared mutable state, no cross-pixel reads. All parameters are read-only (`&PerPixelParams`).

Change to `buf.par_chunks_mut(1024)` with an inner sequential loop per chunk. Chunk size of 1024 pixels balances rayon scheduling overhead against cache locality. Each chunk processes its pixels identically to the current code.

The `PerPixelParams` struct contains only `Copy` types and shared references (`&ToneCurvePrecomputed`, `&ColorGradingPrecomputed`, `&dyn Fn`). The `lut_fn` field is `Option<&dyn Fn(f32, f32, f32) -> (f32, f32, f32)>` — this is `Sync` since it's a shared reference to a closure over an `Arc<Lut3D>`. No changes needed to make `PerPixelParams` safe for parallel access.

### P2: Parallelize Gaussian blur

**File:** `crates/agx/src/adjust/detail.rs` — `gaussian_blur`

The separable Gaussian blur has two passes:

1. **Horizontal pass:** For each row, convolve with the 1D kernel reading from `input` (immutable) and writing to `temp`. Each row writes to a disjoint slice of `temp`. Parallelize by splitting `temp` into row-sized chunks via `par_chunks_mut(width)`, with each chunk computing one row's convolution.

2. **Vertical pass:** For each row of `output`, convolve vertically reading from `temp` (immutable after horizontal pass) and writing to `output`. Parallelize identically — `output.par_chunks_mut(width)`, each chunk computes one row by reading a column-stride pattern from `temp`.

Both passes read from a shared immutable buffer and write to disjoint regions, so no synchronization is needed.

### Supporting changes

- **`profile_summary.sh`:** Update the hardcoded `stage_order` list to match the new pluggable pipeline stage names (`per_pixel_adjustments`, `linear_to_srgb`, `srgb_to_linear`, `vignette` instead of the old combined names).
- **`docs/backlog/performance.md`:** Update stage names in the baseline table to match (cosmetic, no data change — we'll reprofile after this work).

## What doesn't change

- The `Stage` trait, pipeline executor, and all other stages are untouched.
- No new public API — these are internal implementation changes to existing functions.
- E2e golden files must be **bit-identical**. Floating-point addition is not associative, but the chunk boundaries don't change the per-pixel computation order (each pixel is processed identically). The blur parallelizes by rows, which also preserves computation order within each row. Output is deterministic.
- Batch parallelism (`agx-cli` uses rayon to process multiple images) coexists naturally with per-image parallelism. Rayon's work-stealing global thread pool handles nested parallelism — when batch parallelism saturates cores, inner parallelism runs on the calling thread with minimal overhead.

## Expected impact

- **P1:** 3-5x speedup on per-pixel stage → saves ~2.5-3s on 26MP heavy presets
- **P2:** 3-5x speedup on Gaussian blur → saves ~2-5s depending on active stages (detail, grain, dehaze all use it)
- **Combined:** 50-60% reduction in total 26MP render time (per backlog estimate)

## Verification

1. `./scripts/verify.sh` — format, clippy, unit tests, architecture tests
2. `./scripts/e2e-quick.sh` — golden file comparison (must be bit-identical)
3. Reprofile after implementation to measure actual gains and update baseline
