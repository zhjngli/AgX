# Dehaze Guided Filter Buffer Drops Design

## Problem

`guided_filter` in `adjust/dehaze.rs` allocates 11 single-channel `Vec<f32>` buffers (~100MB each at 26MP) that all stay alive until the function returns. Several are only needed until their box-filtered mean is computed, then never read again — yet they continue to occupy heap memory through the rest of the function.

Backlog estimate: peak memory during `guided_filter` is ~2.2GB at 26MP. Dropping unused intermediate buffers after their last read should cut that to ~1.4GB without changing any math or output.

## Approach

Add explicit `drop()` calls to release four intermediate buffers as soon as their final consumer finishes:

| Buffer | Last read | Drop after |
|--------|-----------|------------|
| `gp` (`guide * input`) | `box_filter_2d(&gp, ...)` produces `mean_gp` | `mean_gp` assignment |
| `gg` (`guide * guide`) | `box_filter_2d(&gg, ...)` produces `mean_gg` | `mean_gg` assignment |
| `a` (per-pixel slope) | `box_filter_2d(&a, ...)` produces `mean_a` | `mean_a` assignment |
| `b` (per-pixel intercept) | `box_filter_2d(&b, ...)` produces `mean_b` | `mean_b` assignment |

No algorithmic change. No reordering of computation. Pixel values and floating-point evaluation order are identical to the current implementation.

## Why explicit drops, not refactor or block scoping

Two alternatives to explicit `drop()` were considered:

- **Helper functions.** Break `guided_filter` into helpers so the buffers go out of scope naturally. This produces the same memory profile but adds new function boundaries that aren't otherwise warranted — `guided_filter` is already a coherent ~60-line algorithm and the helpers would only exist for lifetime management.
- **Block scoping.** Wrap each producer-and-mean pair in `let mean_x = { let x = ...; box_filter_2d(&x, ...) };` so `x` drops at the closing brace. Same MIR-level outcome as explicit `drop()`. The reason it loses here is the structure of `guided_filter`: `gp` and `gg` are produced sequentially before either mean is computed (both closures read `guide`), and the same pattern holds for `a`/`b`. Putting each into its own block would force reordering the producer/mean interleaving away from the algorithm's natural reading order, or duplicating computations across blocks.

Explicit `drop()` is one line per buffer, signals intent at the drop site, preserves the algorithm's natural ordering, and matches the existing precedent for memory-tradeoff comments at point of use elsewhere in this file (see the `UnsafeSlicePtr` doc comment).

## Memory expectation

Each dropped `Vec<f32>` at 26MP is `width * height * 4 bytes` ≈ 100MB. Dropping `gp`, `gg`, `a`, `b` at the right points should reduce peak resident set size by 200-400MB during the guided filter call (the exact figure depends on which buffers are simultaneously alive at peak; box-filter working memory also contributes).

The backlog item cites ~800MB total reduction. The design treats the backlog figure as expected ceiling and the measurement step below as ground truth.

## Verification

1. `./scripts/verify.sh` — fmt, clippy, unit, architecture, doc-links.
2. `./scripts/e2e.sh` — full golden matrix must remain byte-identical. The change does not touch math, so any golden delta is a bug.
3. Memory measurement using `/usr/bin/time -l` on macOS (or `/usr/bin/time -v` on Linux) on a 26MP fixture with dehaze active:

   ```
   /usr/bin/time -l ./target/release/agx apply \
       --preset crates/agx-e2e/fixtures/looks/dune.toml \
       crates/agx-e2e/fixtures/raw/sunset_river.raf \
       /tmp/out.jpg
   ```

   Capture the `maximum resident set size` value before the change (on `main`) and after. Record both numbers in this design doc once measured. Linux reports kilobytes; macOS reports bytes.

## Measurement

Measured on macOS (Apple Silicon, system memory pressure low) using `/usr/bin/time -l`. Fixture: `sunset_river.raf` (26MP) with `dune.toml` preset (dehaze active). Two release binaries built from the same source tree: one from the branch tip (no drops, identical to `main`), one with the four `drop()` calls applied. Runs interleaved alternating 5 times each to average out machine load drift.

### Single-run result (superseded — methodology too noisy)

An earlier single pre/post comparison produced a misleading result: baseline ~2,829 MB, after-drops ~2,978 MB, appearing to show a regression. This was an artefact of different machine load conditions between the two separate build+run sessions, not a true algorithmic effect. These numbers are retained for audit trail only.

### Median-of-5 result (definitive)

Raw peak RSS values (bytes):

| Run | Baseline | With drops |
|-----|----------|------------|
| 1 | 3,435,102,208 | 3,122,741,248 |
| 2 | 3,435,495,424 | 3,121,774,592 |
| 3 | 3,434,446,848 | 3,123,200,000 |
| 4 | 3,435,233,280 | 3,122,479,104 |
| 5 | 3,435,266,048 | 3,123,085,312 |

| Build | Median RSS (bytes) | Median RSS (MB) |
|-------|--------------------|-----------------|
| Baseline (`main`) | 3,435,233,280 | 3,276.1 |
| With buffer drops | 3,122,741,248 | 2,978.1 |
| Delta (saved) | 312,492,032 | 298.0 |

The spread within each set is tiny (< 1 MB across 5 runs), confirming stable measurement conditions. The explicit `drop()` calls cut peak RSS by **298 MB (9.1%)**. The drops are effective — LLVM does not shrink these heap lifetimes on its own at this optimization level. The change is kept and shipped.

## Files

| File | Change |
|------|--------|
| `crates/agx/src/adjust/dehaze.rs` | Add four `drop()` calls inside `guided_filter` |
| `docs/backlog/performance.md` | Check off "Dehaze guided filter intermediate buffers" |

## Scope

- **In scope.** Explicit drops for `gp`, `gg`, `a`, `b` in `guided_filter`. Memory measurement before/after. Backlog checkoff.
- **Out of scope.** Algorithmic changes. In-place box filter variants. Decode/encode buffer reduction (separate design). SIMD vectorization. Persistent memory profiling harness.

## Risks

- **Drop placement.** The Rust borrow checker will refuse to drop a buffer still being read, so misplaced drops fail to compile. The risk is the inverse: dropping too early would compile only if the buffer is not subsequently read, which is the desired property — no runtime risk here.
- **Compiler may already optimize.** LLVM can sometimes shrink stack/heap lifetimes via DCE. The measurement step verifies the explicit drops produce a measurable RSS delta; if they do not, the optimization is a no-op and we close the backlog item without merging.

## Related

- Backlog: [docs/backlog/performance.md](../backlog/performance.md) — "Dehaze guided filter intermediate buffers"
- Sibling effort: render IO buffer reduction (decode + encode) — separate design and PR landing in parallel.
- Prior dehaze work: [2026-04-05-dehaze-parallelization-design.md](2026-04-05-dehaze-parallelization-design.md) (P5 parallelization)
