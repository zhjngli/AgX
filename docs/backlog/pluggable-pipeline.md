# Pluggable Pipeline

Refactor the hardcoded render sequence in `engine::render()` into discrete stages implementing a `Stage` trait.

## Sub-tasks

- [x] **Design the Stage trait** — each stage declares its color space (linear, sRGB gamma, log), accepts an image buffer, returns a modified buffer. The engine auto-inserts color space conversions between stages
- [x] **Extract per-pixel adjustments into a stage** — move exposure, contrast, highlights/shadows, whites/blacks, white balance, HSL, color grading, tone curves into a single per-pixel stage
- [x] **Extract neighborhood ops into stages** — detail pass, dehaze, noise reduction, grain each become independent stages
- [x] **Move profiling into the pipeline executor** — replace the `profile_stage!` macro calls in `engine::render()` with automatic per-stage timing in the executor. This eliminates macro injection and makes profiling a pipeline concern, not a per-stage concern
- [ ] **Stage-level caching** — cache intermediate results at stage boundaries. When a parameter changes, only recompute from the affected stage forward. Key for interactive editing performance
- [ ] **Color-space-aware stage insertion** — auto-insert conversions between stages with different working color spaces. Enables LUTs designed for different input spaces (sRGB, log, linear)

## Parked

- [ ] GPU path doesn't share the pluggable stage/executor architecture *(epic candidate)*. Each new CPU stage or color space must be hand-reimplemented and hand-ordered in WGSL, with no executor or auto-insert equivalent. As the CPU pipeline grows more pluggable, this manual mirroring is a growing source of CPU/GPU drift and consistency risk. No design yet for a GPU-side executor or a shared stage description both paths consume. Surfaced building color-space-aware insertion, where the CPU executor gained auto-insert but the GPU kept its fused, hand-ordered LUT bracket.
- [ ] The WGSL `Params` uniform struct is hand-duplicated across ~18 shader files, so every parameter-layout change is an 18-file edit prone to silent drift. The shaders already share `common::*` modules via `naga_oil` `#import`, which can export struct definitions too — a single `common::params` module would collapse this. Surfaced twice now during struct-layout changes (StageInputs work and the GPU LUT-encoding field).

## Considerations

- Today's single per-pixel pass is very cache-friendly. Stages mean multiple passes over the image — this is a real performance tradeoff.
- We now have 4 neighborhood/buffer-level ops (detail, dehaze, noise reduction, grain), which is enough to justify the abstraction.
- Stage caching is the key enabler for interactive editing — without it, changing one slider re-runs the entire pipeline.
- Don't over-design the trait interface. Start with a minimal trait and evolve it as new stages reveal what's actually needed.

## Related

- [Performance](performance.md) — stage caching and pipeline design affect render performance
- [Local Adjustments](local-adjustments.md) — per-region rendering interacts with pipeline stages
- [Color Management](color-management.md) — color-space-aware stages enable wider gamut support
