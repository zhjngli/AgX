# Noise Reduction

**Category:** Editing
**Status:** Backlog

## Problem / Opportunity

Sensor noise degrades image quality, especially in high-ISO and low-light photos. Noise reduction removes luminance and chroma noise while preserving detail. Separate luminance NR and color NR controls with a detail preservation slider are the standard UX. Essential for any serious raw processing workflow.

## Key Considerations

- Wavelet-based or bilateral filtering approaches are common
- Separate luminance NR (reduces grain/noise) and color NR (reduces chroma blotches)
- Detail preservation slider controls the trade-off between smoothing and retaining fine texture
- Neighborhood operation — requires buffer-level access like the detail pass
- Interacts with sharpening — typically applied before sharpening in the pipeline
- Preset-friendly: uniform parameters, no per-photo tuning required

## Related

- [Dehaze](dehaze.md) — both are neighborhood operations
- [Film and Grain](film-and-grain.md) — grain is applied after noise reduction
