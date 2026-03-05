# Geometric Corrections

**Category:** Editing
**Status:** Backlog

## Problem / Opportunity

Lens corrections (distortion, chromatic aberration, vignette), perspective correction, and crop/rotation are essential geometric tools. Lens corrections fix optical flaws automatically using lens profile databases. Perspective correction fixes converging verticals in architecture. Crop and rotation are the most basic composition tools.

## Key Considerations

- **Lens corrections**: Via lensfun FFI — provides distortion, TCA, and vignette correction profiles for thousands of lens/camera combinations
- **Chromatic aberration**: Can also be corrected manually (lateral CA as a per-channel scale, longitudinal CA is harder)
- **Perspective correction**: Vertical/horizontal keystone transforms. 4-point perspective warp for advanced use. Requires image resampling (bilinear or bicubic interpolation)
- **Crop and rotation**: Non-destructive crop with aspect ratio presets (1:1, 4:3, 16:9, custom). Rotation with automatic crop or canvas extension
- All geometric operations require image resampling — introduces interpolation quality concerns
- Should be applied before pixel-level adjustments (correct geometry first, then adjust tones/colors) or after (standard in most editors). Most editors apply lens corrections early and crop/rotation as metadata
- lensfun FFI adds a significant external dependency

## Related

- [Pluggable Pipeline](pluggable-pipeline.md) — geometric stages would be early in the pipeline
- [Local Adjustments](local-adjustments.md) — geometric corrections affect mask/gradient coordinates
