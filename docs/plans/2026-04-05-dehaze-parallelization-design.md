# Dehaze Parallelization Design (P5)

## Problem

After P1-P4 parallelized per-pixel adjustments, Gaussian blur, denoise, and grain, the dehaze stage is the single biggest remaining bottleneck at ~4.6-4.9s for 26MP images. Dehaze has its own sequential implementations of `dark_channel` (separable min filter), `box_filter_2d` (separable box filter), and `guided_filter` (6x box filter + pixel loops) in `adjust/dehaze.rs` that were not touched by the earlier parallelization work.

## Approach

Parallelize all independent operations in `adjust/dehaze.rs` using rayon. Same pattern as P2 (Gaussian blur): row-parallel horizontal passes, column-parallel vertical passes, `par_chunks_mut` for pixel loops. No algorithmic changes, no new abstractions, no API changes.

## What Gets Parallelized

### Separable filter passes

Both `dark_channel` and `box_filter_2d` use a separable 2-pass strategy: horizontal 1D filter per row, then vertical 1D filter per column.

- **Horizontal pass**: `par_chunks_mut(width)` over the output buffer so each row is processed independently by a rayon thread.
- **Vertical pass**: parallel iteration over column indices. Each thread allocates a local `col_buf`, gathers the column from the horizontal output, runs the 1D filter, and scatters results back.

`dark_channel` is called twice per dehaze (once on the original, once on the normalized image). `box_filter_2d` is called 6 times inside `guided_filter`. This accounts for the bulk of the ~4.7s.

### Per-pixel loops

The following loops are embarrassingly parallel and use `par_chunks_mut(1024)` (or `par_chunks` for read-only input):

- `dark_channel` step 1: per-pixel `min(R, G, B)`
- `guided_filter`: `gp[i] = guide[i] * input[i]`, `gg[i] = guide[i] * guide[i]`, covariance/variance → `a[i]`/`b[i]`, final output `mean_a[i] * guide[i] + mean_b[i]`
- `apply_dehaze`: normalize by airlight, `t_raw` computation, guide (luma) computation, scene recovery, negative-haze blend

### Left sequential

- **`estimate_airlight`**: partial sort (`select_nth_unstable_by`) + linear scan over a tiny subset (~0.1% of pixels). Not worth parallelizing.
- **`min_filter_1d` / `box_filter_1d`**: 1D kernels called per row/column. Parallelism is at the row/column dispatch level, not inside the kernel. Each kernel is O(n) with a sliding window — already fast for a single row.

## Memory

No meaningful peak memory increase. The vertical passes allocate a per-thread `col_buf` (~100KB each at 26MP) instead of one shared buffer. With 8-16 threads this is ~0.8-1.6MB — negligible compared to the ~300MB image buffers.

## Testing

Existing unit tests in `dehaze.rs` (13 tests) verify algorithm correctness. E2e golden file comparisons verify bit-identical output. No new tests needed — parallelization must not change results.

## Expected Speedup

Based on P2 results (3-5x on similar separable filter passes), expect dehaze to drop from ~4.7s to ~1-1.5s at 26MP. The guided filter's 6 `box_filter_2d` calls dominate the runtime.

## Scope

- **In scope**: Parallelize all operations in `adjust/dehaze.rs`, update backlog and architecture docs.
- **Out of scope**: Shared filter infrastructure with `detail.rs` (Approach 3 from brainstorming — deferred as unnecessary coupling). Algorithmic changes. Transpose-based vertical pass optimization (measured as unlikely to help given existing gather-into-temp-buffer pattern).

## Files

| File | Change |
|------|--------|
| `crates/agx/src/adjust/dehaze.rs` | Add rayon parallelism to all filter passes and pixel loops |
| `docs/backlog/performance.md` | Check off P5 |
| `ARCHITECTURE.md` | Link this design doc |
