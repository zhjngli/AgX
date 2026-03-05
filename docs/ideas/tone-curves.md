# Tone Curves

**Category:** Editing
**Status:** Backlog

## Problem / Opportunity

Tone curves are the most powerful and flexible tool in any photo editor. Parametric curves (region-based: highlights, lights, darks, shadows) give intuitive control, while point curves (user-placed control points on an RGB or per-channel spline) allow precise tonal sculpting. Adding both would close a major gap with Lightroom and Capture One.

## Key Considerations

- Parametric curves divide the tonal range into regions and apply smooth adjustments within each — simpler UX but less flexible
- Point curves require a spline interpolation algorithm (cubic, Catmull-Rom, or monotone cubic) to produce smooth results between control points
- Both RGB (master) and per-channel (R, G, B) curve modes are expected
- Must decide where in the pipeline curves sit — after tone adjustments but before LUT seems standard
- Curves operate in sRGB gamma space (perceptual) for intuitive behavior
- Could share infrastructure with the existing highlights/shadows adjustments (both are tonal region targeting)

## Related

- [HSL Adjustments](hsl-adjustments.md) — complementary color control
- [Color Grading](color-grading.md) — creative tonal/color shifts
- [Pluggable Pipeline](pluggable-pipeline.md) — curves would be a natural pipeline stage
