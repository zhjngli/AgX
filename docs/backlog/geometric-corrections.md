# Geometric Corrections

Lens corrections, perspective correction, crop and rotation.

## Sub-tasks

- [ ] **Lens corrections (lensfun FFI)** — distortion, TCA, and vignette correction using lens profile databases for thousands of lens/camera combinations
- [ ] **Manual chromatic aberration correction** — lateral CA as a per-channel scale (longitudinal CA is harder)
- [ ] **Perspective correction** — vertical/horizontal keystone transforms, 4-point perspective warp for advanced use
- [ ] **Crop and rotation** — non-destructive crop with aspect ratio presets (1:1, 4:3, 16:9, custom), rotation with automatic crop

## Considerations

- All geometric operations require image resampling (bilinear or bicubic interpolation) — introduces interpolation quality concerns.
- Pipeline ordering: most editors apply lens corrections early and crop/rotation as metadata. Needs thought on where these fit in the render pipeline.
- lensfun FFI adds a significant external dependency.

## Related

- [Pluggable Pipeline](pluggable-pipeline.md) — geometric stages would be early in the pipeline
- [Local Adjustments](local-adjustments.md) — geometric corrections affect mask/gradient coordinates
