# Render Performance Profiling Infrastructure

**Date:** 2026-03-31
**Status:** Draft
**Branch:** `perf/render-optimization`

## Problem

AgX has no performance measurement infrastructure. The render pipeline — decode, adjustments, buffer-level passes, encode — runs entirely single-threaded within a single image. Before optimizing, we need to understand where time is actually spent.

## Goals

- Add feature-gated profiling instrumentation to the render pipeline with zero overhead when disabled
- Measure decode, render stage, and encode times across a representative set of images and presets
- Document findings and produce a prioritized optimization roadmap based on data

## Non-Goals

- Implementing optimizations (parallelization, SIMD, GPU) — this effort is measurement only
- Micro-benchmarking individual functions (Criterion) — we want end-to-end pipeline stage timing
- Profiling batch processing throughput — focus is single-image render performance

## Design

### Feature flag

A `profiling` feature is added to `crates/agx/Cargo.toml`. When enabled, `Engine::render()` records `std::time::Instant` timestamps between each pipeline stage. When disabled, no timing code is compiled — zero overhead in production builds.

### Timing data

```rust
#[cfg(feature = "profiling")]
pub struct RenderProfile {
    pub stages: Vec<(String, std::time::Duration)>,
    pub total: std::time::Duration,
}
```

`Engine::render()` returns a `RenderResult` containing the image and an optional `RenderProfile` (present only when compiled with the `profiling` feature). Decode and encode timing are measured in the CLI layer since they happen outside the engine.

### Pipeline stages measured

| Stage | Location | What it covers |
|-------|----------|----------------|
| decode | CLI | File I/O + format decompression + sRGB-to-linear conversion |
| white_balance_exposure | Engine | White balance channel multipliers + exposure factor |
| dehaze | Engine | Dark channel, guided filter, transmission map (when active) |
| denoise | Engine | A trous wavelet decomposition, thresholding, reconstruction (when active) |
| linear_to_srgb | Engine | Linear-to-sRGB gamma encoding |
| per_pixel_adjustments | Engine | Contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading, LUT (single loop) |
| detail | Engine | Texture, clarity, sharpening buffer passes (when active) |
| grain | Engine | Noise generation, blur, per-pixel application (when active) |
| vignette | Engine | Position-dependent darkening/brightening (when active) |
| srgb_to_linear | Engine | sRGB-to-linear gamma decoding |
| encode | CLI | Linear-to-sRGB + f32-to-u8 + format compression + metadata + file write |

Per-pixel adjustments (contrast through LUT) are grouped because they execute in a single loop — inserting per-pixel timing would distort results.

### CLI interface

```
cargo build --release --features profiling -p agx-cli
./target/release/agx-cli edit --input photo.raw --output out.png --profile-output timings.json
```

The `--profile-output <path>` flag is gated behind the `profiling` feature. When specified, the CLI writes a JSON file containing timing data for the rendered image. For batch operations, each image's profile is appended to an array in the output file.

### JSON output format

```json
[
  {
    "image": "sunset_river_noop.png",
    "preset": "blade_runner",
    "dimensions": [1024, 684],
    "stages": {
      "decode": 200.5,
      "white_balance_exposure": 5.2,
      "dehaze": 0.0,
      "denoise": 0.0,
      "linear_to_srgb": 15.1,
      "per_pixel_adjustments": 45.3,
      "detail": 180.7,
      "grain": 120.4,
      "vignette": 10.2,
      "srgb_to_linear": 14.8,
      "encode": 300.1
    },
    "total_ms": 892.3
  }
]
```

All times in milliseconds (f64). Stages that are skipped (neutral parameters) report 0.0.

### Profiling test matrix

**Images** — cover different decode paths and sizes:

- RAW test images (exercises LibRaw decode)
- PNG test images from e2e fixtures (standard decode, known dimensions)
- JPEG test images from e2e fixtures

**Presets** — exercise different pipeline stages:

- `noop` — baseline (decode + encode only)
- Light presets (exposure/WB/contrast, no heavy buffer ops)
- Heavy presets (grain + LUT + multiple adjustments)
- Presets with dehaze/denoise if available

Start with a representative subset and expand if data suggests gaps. The infrastructure supports any number of images and presets.

**Repetitions** — each combination run 3 times, take the median to reduce variance from I/O and system noise.

### Profiling scripts

**`scripts/profile.sh`** — runs the image x preset x repetition matrix:

1. Builds `agx-cli` with `--features profiling` in release mode
2. Iterates over selected images and presets
3. Runs each combo 3 times with `--profile-output`
4. Collects all JSON into a single results file

**`scripts/profile_summary.sh`** — reads the JSON results and prints a human-readable summary:

- Median time per stage per image/preset combo
- Percentage of total time per stage
- Sorted by total render time (slowest first)

### Flamegraph follow-up

After structured profiling identifies the heaviest stages, run `cargo flamegraph` on the slowest combo to drill into function-level hotspots within those stages (e.g., is Gaussian blur the bottleneck in detail, or is it the unsharp mask blend?).

## Files Changed

| File | Change |
|------|--------|
| `crates/agx/Cargo.toml` | Add `profiling` feature flag |
| `crates/agx/src/engine/mod.rs` | Add `RenderProfile` struct, instrument `render()` with timing gates |
| `crates/agx-cli/Cargo.toml` | Add `profiling` feature that enables `agx/profiling` |
| `crates/agx-cli/src/main.rs` | Add `--profile-output` flag, measure decode/encode, write JSON |
| `scripts/profile.sh` | Profiling runner script |
| `scripts/profile_summary.sh` | Results summary script |

## Deliverables

1. Feature-gated profiling infrastructure (zero overhead when disabled)
2. CLI `--profile-output` flag for JSON timing output
3. Profiling and summary scripts
4. Analysis document (`docs/plans/YYYY-MM-DD-render-performance-analysis.md`) with findings, bottleneck identification, and prioritized optimization roadmap
5. Flamegraph for the heaviest image/preset combination

## References

- [`docs/backlog/performance.md`](../backlog/performance.md) — performance backlog with speculative optimization ideas
- [`crates/agx/src/engine/mod.rs`](../../crates/agx/src/engine/mod.rs) — render pipeline implementation
- [`docs/reference/grain-algorithm.md`](../reference/grain-algorithm.md) — grain algorithm (one of the heavier pipeline stages)
