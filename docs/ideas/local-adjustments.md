# Local Adjustments

**Category:** Editing
**Status:** Backlog

## Problem / Opportunity

Local adjustments (brushes, linear gradients, radial filters) allow applying edits to specific regions of the image rather than globally. This is essential for tasks like dodging/burning, selective color correction, sky enhancement, and subject isolation. Every professional photo editor supports local adjustments.

## Key Considerations

- **Mask types**: Brush (freeform painted), linear gradient (feathered line), radial gradient (feathered ellipse)
- **Per-region parameters**: Each masked region carries its own set of adjustments (exposure, contrast, saturation, sharpening, etc.) — essentially a subset of global parameters
- **Mask representation**: Binary mask, feathered grayscale mask, or parametric definition (gradient endpoints, ellipse center/radii)
- **Parametric storage**: Storing mask definitions (not pixel data) in presets keeps them resolution-independent and human-readable
- **Multiple masks**: Users expect to create many overlapping masks. Masks combine additively or with explicit blend modes
- **Performance**: Applying N local adjustments means N partial re-renders of the affected region. Caching intermediate results becomes important
- **AI-assisted masking**: Subject detection, sky detection, etc. could auto-generate masks (see advanced-research.md)
- This is a major architectural addition — local adjustments fundamentally change the render model from "apply global params to every pixel" to "apply global + per-region params"

## Related

- [Pluggable Pipeline](pluggable-pipeline.md) — local adjustments interact with pipeline stage ordering
- [Sharpening and Detail](sharpening-and-detail.md) — commonly used as per-region adjustments
- [Advanced Research](advanced-research.md) — AI masking for automatic subject/sky selection
