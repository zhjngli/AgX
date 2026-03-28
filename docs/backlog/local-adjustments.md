# Local Adjustments

Brushes, gradients, and radial filters for applying edits to specific image regions.

## Sub-tasks

- [ ] **Mask types** — brush (freeform painted), linear gradient (feathered line), radial gradient (feathered ellipse)
- [ ] **Per-region parameter application** — each masked region carries its own set of adjustments (exposure, contrast, saturation, etc.)
- [ ] **Parametric mask storage** — store mask definitions (not pixel data) in presets for resolution-independent, human-readable representation
- [ ] **Multiple overlapping masks** — additive combination or explicit blend modes
- [ ] **AI-assisted masking** — subject detection, sky detection for auto-generated masks (stretch goal)

## Considerations

- This is a major architectural addition — local adjustments fundamentally change the render model from "apply global params to every pixel" to "apply global + per-region params."
- Applying N local adjustments means N partial re-renders. Caching intermediate results becomes important.
- Parametric storage keeps masks resolution-independent and human-readable in presets.

## Related

- [Pluggable Pipeline](pluggable-pipeline.md) — local adjustments interact with pipeline stage ordering
- [Advanced Research](advanced-research.md) — AI masking for automatic subject/sky selection
- [Geometric Corrections](geometric-corrections.md) — geometric corrections affect mask/gradient coordinates
