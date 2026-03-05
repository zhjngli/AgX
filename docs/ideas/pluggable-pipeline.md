# Pluggable Pipeline

**Category:** Pipeline
**Status:** Backlog

## Problem / Opportunity

The current render pipeline is a hardcoded sequence in `engine::render()`. As adjustments grow (curves, HSL, sharpening, dehaze, local adjustments), this becomes unmaintainable. A pluggable pipeline refactors rendering into discrete stages implementing a `Stage` trait, making it mechanical to add new adjustments and enabling stage-level caching and automatic color space conversion.

## Key Considerations

- **Stage trait**: Each stage declares its color space (linear, sRGB gamma, log), accepts an image buffer, returns a modified buffer. The engine auto-inserts color space conversions between stages
- **Registration**: Stages are registered in order. The engine iterates through `Vec<Box<dyn Stage>>`, calling each in sequence
- **Neighborhood operations**: Sharpening, clarity, dehaze need access to surrounding pixels — naturally modeled as stages operating on the full buffer, unlike current per-pixel adjustments
- **Trade-off**: Today's single per-pixel pass is very cache-friendly. Stages mean multiple passes over the image. Probably premature until there are 12+ adjustments
- **Stage-level caching**: Cache intermediate results at stage boundaries. When a parameter changes, only recompute from the affected stage forward. Key for interactive editing performance
- **Color-space-aware stages**: Each stage declares its input/output color space. The pipeline auto-inserts conversions, which also enables proper support for LUTs designed for different input spaces (sRGB, log, linear)
- Adding 2-3 more features as direct adjustments first will reveal what the trait interface actually needs — don't over-design the abstraction prematurely

## Related

- [Sharpening and Detail](sharpening-and-detail.md) — neighborhood operations motivate stages
- [Dehaze](dehaze.md) — another neighborhood operation
- [Local Adjustments](local-adjustments.md) — per-region rendering interacts with pipeline stages
- [Color Management](color-management.md) — color-space-aware stages enable wider gamut support
