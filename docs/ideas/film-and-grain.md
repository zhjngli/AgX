# Film and Grain

**Category:** Editing
**Status:** Backlog

## Problem / Opportunity

Film grain simulation adds organic texture that mimics analog film stocks. A film emulation database (community-contributed profiles for Portra, Ektar, Tri-X, etc.) would let users apply the look of classic films. These are among the most popular creative effects in photo editing — Fujifilm's film simulation modes are a major selling point.

## Key Considerations

- **Grain**: Parameters include amount, size (fine/coarse), and roughness. Grain should be luminance-aware (more visible in midtones, less in deep shadows/blown highlights)
- **Grain implementation**: Perlin noise or simplex noise scaled to grain size, blended into the image. Must be resolution-aware (grain size should look consistent across different image sizes)
- **Film emulation database**: Each film profile is essentially a LUT + grain preset + tone curve adjustment. Could be distributed as bundled presets combining existing tools
- **Community contributions**: Need a format and submission process for community film profiles
- Film emulation LUTs already work with the existing LUT system — the database adds curation and grain on top

## Related

- [Color Grading](color-grading.md) — film looks combine color grading with grain
- [Sharpening and Detail](sharpening-and-detail.md) — grain interacts with noise reduction and sharpening
- [Preset Composability](preset-composability.md) — film profiles are natural candidates for composable presets
