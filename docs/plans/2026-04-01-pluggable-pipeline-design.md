# Pluggable Pipeline Design

**Date:** 2026-04-01
**Branch:** `refactor/pluggable-pipeline`
**Backlog epic:** [pluggable-pipeline.md](../backlog/pluggable-pipeline.md)

## Goal

Refactor the monolithic `Engine::render()` method (~370 lines, two render paths, inline profiling macros) into a stage-based pipeline with discrete, self-contained stages behind a `Stage` trait. The pipeline order remains fixed and hardcoded — this is a code organization refactor, not a user-facing feature.

**In scope:** Stage trait, executor with automatic profiling and color space conversion, extract all pipeline stages, remove `profile_stage!` macro, update docs.

**Out of scope (future work):** Stage-level caching, color-space-aware insertion beyond linear/sRGB, per-pixel stage fusion optimization.

## Decisions

### Always buffer-based

The current dual render path (per-pixel fast path vs. buffer path) is eliminated. All stages read and write a shared `Vec<[f32; 3]>` buffer. Rationale:

- The fast path almost never fires — every real preset uses grain, which forces the buffer path.
- Buffer allocation at 26MP is ~80ms (<1% of total render time). Not worth the code complexity.
- A uniform buffer interface is prerequisite for future stage-level caching.
- Memory overhead (~300MB intermediate buffer at 26MP) is acceptable for single-image CLI workflows. Batch memory pressure is tracked as a [performance backlog item](../backlog/performance.md).

### Fixed pipeline order

The pipeline order is hardcoded in `Pipeline::new()` and not configurable by consumers. This is an existing invariant (documented in `ARCHITECTURE.md`: "fixed render order — hardcoded, not user-facing"). Changing stage order would alter rendering semantics and break preset compatibility.

### Trait objects (dynamic dispatch)

Stages are `Box<dyn Stage>` held in a `Vec`. The vtable overhead is ~7 nanoseconds per render (7 stages x ~1ns per indirect call) against a 4-17s total — unmeasurable. The benefit: each stage struct owns its precomputed state, enabling clean prepare/process separation and future caching.

### Stages delegate to adjust module

Every stage delegates its pixel math to the `adjust` module. Stages own orchestration (prepare, is_active, color space declaration). Adjust owns the algorithms. This maintains the existing separation: `engine` orchestrates, `adjust` computes.

### Color space auto-conversion

Stages declare their input and output color spaces. The executor inserts conversion steps when adjacent stages disagree. Currently only two color spaces exist (LinearSrgb, SrgbGamma), so the conversion table is trivial. When color management lands later, the table grows but the mechanism stays the same.

### Profiling moves to the executor

The `profile_stage!` macro is removed. The executor times each stage (including auto-inserted conversions) in its main loop, feature-gated behind `#[cfg(feature = "profiling")]`. Zero overhead when disabled.

## Design

### Stage Trait

```rust
pub enum ColorSpace {
    LinearSrgb,
    SrgbGamma,
}

pub trait Stage: Send + Sync {
    /// Human-readable name for profiling output.
    fn name(&self) -> &'static str;

    /// Color space this stage expects its input in.
    fn input_color_space(&self) -> ColorSpace;

    /// Color space this stage produces.
    fn output_color_space(&self) -> ColorSpace;

    /// Whether this stage has any effect given current params.
    /// Returning false lets the executor skip it entirely.
    fn is_active(&self, params: &Parameters) -> bool;

    /// Precompute loop-invariant data from params.
    /// Called once per render, before process().
    fn prepare(&mut self, params: &Parameters);

    /// Process the buffer in-place.
    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError>;
}
```

- `Send + Sync` bounds: required so `Engine` (which owns the pipeline) can be moved into rayon worker threads for batch processing.
- `prepare(&mut self)` + `process(&self)`: separation enables future caching (skip both if params haven't changed since last prepare).

### RenderContext

```rust
pub struct RenderContext<'a> {
    pub buf: Vec<[f32; 3]>,
    pub width: u32,
    pub height: u32,
    pub params: &'a Parameters,
    pub lut: Option<&'a Arc<Lut3D>>,
}
```

Owned by the executor, passed as `&mut` to each stage's `process()`. The buffer is mutated in place — no allocation between stages.

### Pipeline (Executor)

```rust
pub struct Pipeline {
    stages: Vec<Box<dyn Stage>>,
}
```

**`Pipeline::new()`** constructs the fixed stage list:

1. `WhiteBalanceExposureStage` (Linear → Linear)
2. `DehazeStage` (Linear → Linear)
3. `DenoiseStage` (Linear → Linear)
4. *[executor auto-inserts Linear→sRGB conversion]*
5. `PerPixelAdjustmentsStage` (sRGB → sRGB)
6. `DetailStage` (sRGB → sRGB)
7. `GrainStage` (sRGB → sRGB)
8. `VignetteStage` (sRGB → sRGB)
9. *[executor auto-inserts sRGB→Linear conversion]*

**`Pipeline::execute()`** logic:

1. Build `RenderContext` from original image (copy pixels into `Vec<[f32; 3]>`)
2. Call `prepare()` on all active stages
3. Iterate stages in order:
   - Skip inactive stages
   - Insert color space conversion if adjacent output/input disagree
   - Time the stage (feature-gated)
   - Call `process(&mut ctx)`
4. Final conversion to LinearSrgb if needed
5. Build `Rgb32FImage` from buffer, return `RenderResult`

### Stage Structs

| Stage | File | Color space | Precomputed state | Delegates to |
|---|---|---|---|---|
| `WhiteBalanceExposureStage` | `stages/white_balance_exposure.rs` | Linear → Linear | exposure_factor | new buffer-level fn in `adjust` |
| `DehazeStage` | `stages/dehaze.rs` | Linear → Linear | (none) | `adjust::dehaze::apply_dehaze` |
| `DenoiseStage` | `stages/denoise.rs` | Linear → Linear | (none) | `adjust::denoise::apply_noise_reduction` |
| `PerPixelAdjustmentsStage` | `stages/per_pixel.rs` | sRGB → sRGB | tone curves, color grading, HSL shifts | new buffer-level fn in `adjust` |
| `DetailStage` | `stages/detail.rs` | sRGB → sRGB | (none) | `adjust::detail::apply_detail_pass` |
| `GrainStage` | `stages/grain.rs` | sRGB → sRGB | seed | `adjust::grain::apply_grain_buffer` |
| `VignetteStage` | `stages/vignette.rs` | sRGB → sRGB | VignettePrecomputed | new buffer-level fn in `adjust` |
| `LinearToSrgbConversion` | `stages/color_space_conversion.rs` | Linear → sRGB | (none) | `adjust::linear_to_srgb` |
| `SrgbToLinearConversion` | `stages/color_space_conversion.rs` | sRGB → Linear | (none) | `adjust::srgb_to_linear` |

### Engine Integration

```rust
pub struct Engine {
    original: Rgb32FImage,
    params: Parameters,
    lut: Option<Arc<Lut3D>>,
    pipeline: Pipeline,
}
```

- `render()` changes from `&self` to `&mut self` (pipeline stages need mutable prepare).
- All other public API methods unchanged.
- `Pipeline` is private — not exposed to consumers.

### New adjust Module Functions

Three new buffer-level functions needed, with unit tests:

- `adjust::apply_white_balance_exposure_buffer(buf, w, h, temperature, tint, exposure)` — applies WB + exposure to a linear buffer in place
- `adjust::apply_per_pixel_adjustments_buffer(buf, params, precomputed, lut)` — applies contrast, highlights, shadows, whites, blacks, tone curves, HSL, color grading, LUT to an sRGB buffer in place
- `adjust::apply_vignette_buffer(buf, w, h, precomputed)` — applies position-dependent vignette to an sRGB buffer in place

## File Layout

```
crates/agx/src/engine/
    mod.rs                  -- Engine, Pipeline, RenderContext, Stage trait, ColorSpace
    stages/
        mod.rs              -- re-exports all stages
        white_balance_exposure.rs
        dehaze.rs
        denoise.rs
        per_pixel.rs
        detail.rs
        grain.rs
        vignette.rs
        color_space_conversion.rs
```

## Testing

1. **Existing tests unchanged.** All 292 `adjust` and `engine` unit tests continue to pass. They call `engine.render().image` — same interface.
2. **New adjust buffer function tests.** Each new `apply_*_buffer` function gets unit tests in its respective `adjust` submodule.
3. **Per-stage tests.** Each stage file tests prepare + process on small buffers to verify wiring.
4. **Bit-identical regression test.** Before replacing `render()`, capture output for a set of param combinations. After the refactor, verify the new pipeline produces identical output. This is the primary correctness guarantee.
5. **e2e tests.** `e2e-quick.sh` and full `e2e.sh` must pass — golden file comparison catches any rendering differences.

## Documentation Updates

- **`ARCHITECTURE.md`**: Add `engine/stages/` submodule description, document the stage-based pipeline, update the module dependency narrative. Add the fixed pipeline order invariant explicitly.
- **`engine/README.md`**: Rewrite to describe the Pipeline, Stage trait, executor flow, how to add a new stage, and the fixed order invariant.
- **Affected adjust module READMEs**: Document new buffer-level functions in the public API section.

## Invariants

- **Fixed pipeline order**: The stage list is hardcoded in `Pipeline::new()`. Consumers cannot reorder, insert, or remove stages. This preserves preset compatibility — same params always produce the same output.
- **Always re-render from original**: Each `execute()` starts from the original image. No accumulated state between renders.
- **Stages don't know about each other**: A stage receives a buffer and params. It doesn't know what ran before or after it.
- **Adjust module stays pure**: No pipeline awareness, no I/O, no state. Pure pixel math.

## Future Considerations

Designed into the trait but not implemented now:

- **Stage-level caching**: The `prepare`/`process` split enables a future "dirty check" — if params haven't changed since last prepare, skip both and reuse the cached buffer. The executor would need to store intermediate buffers at stage boundaries.
- **Color management**: The `ColorSpace` enum can grow (ProPhoto, ACEScg, log, etc.) and the executor's conversion table expands. Stages declare their space, executor handles routing.
- **Batch memory pressure**: Always-buffer-based means ~300MB per intermediate buffer at 26MP. Tracked in the performance backlog for future optimization (buffer pooling, lazy allocation).

## References

- [Pluggable pipeline backlog epic](../backlog/pluggable-pipeline.md)
- [Render performance analysis](2026-04-01-render-performance-analysis.md)
- [Profiling infrastructure design](2026-03-31-render-performance-profiling-design.md)
- [ARCHITECTURE.md](../../ARCHITECTURE.md)
