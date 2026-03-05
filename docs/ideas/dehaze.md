# Dehaze

**Category:** Editing
**Status:** Backlog

## Problem / Opportunity

Atmospheric haze reduces contrast and shifts colors toward the ambient light color (usually blue/gray). A dehaze tool recovers contrast and color in hazy images — useful for landscapes, cityscapes, and any outdoor photography. Lightroom's dehaze slider is one of its most popular features.

## Key Considerations

- The standard approach is based on the Dark Channel Prior (He et al., 2009) — estimate a transmission map from the darkest channel in local patches, then invert the haze model
- Simplified versions skip the full transmission map and use a local contrast enhancement approach (faster, less accurate)
- A single slider (amount, -100 to +100) is the expected UX — negative values add haze/fog effect
- Dehaze is a neighborhood operation (requires local patch statistics) — not a per-pixel adjustment
- Must handle sky regions carefully — the dark channel prior breaks down in bright sky areas
- Pipeline placement: typically early in the adjustment chain, after exposure correction

## Related

- [Sharpening and Detail](sharpening-and-detail.md) — dehaze and clarity are related (both enhance local contrast)
- [Pluggable Pipeline](pluggable-pipeline.md) — dehaze needs buffer access as a neighborhood operation
