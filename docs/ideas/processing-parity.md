# Processing Parity

**Category:** Advanced
**Status:** Backlog

## Problem / Opportunity

Rendering differences between oxiraw and other photo editors (Capture One, Lightroom, darktable) are expected for any input format — not just raw files. Understanding and optionally reducing these differences helps users transition from other tools and builds confidence in oxiraw's output quality.

## Key Considerations

Multiple factors contribute to rendering differences:

- **Demosaicing algorithm**: LibRaw defaults (AHD/PPG) differ from Capture One's proprietary algorithms, affecting detail and color at the pixel level
- **Tone curves**: Each processor applies its own base tone curve to raw data before user adjustments. LibRaw's default rendering is fairly flat compared to commercial processors
- **White balance calculation**: "Auto" white balance varies between implementations; camera-stored WB may be interpreted differently
- **Exposure mapping**: How "+1 stop" translates to pixel values may differ (linear multiply vs curve-aware lift)
- **Color matrices**: Each processor may use different per-camera color calibration data
- **Gamma/highlight handling**: Highlight recovery, highlight reconstruction, and rolloff behavior vary significantly

This is the nature of raw processing — there is no single "correct" rendering, only different interpretations. Normalizing output to match a specific processor is possible but complex (would require reverse-engineering their tone curves and color science).

Future work could include:

- Configurable base tone curves (flat, medium contrast, match-Lightroom, etc.)
- Per-camera color profiles (DCP/ICC) for more accurate starting points
- User-adjustable demosaicing algorithm selection
- A/B comparison tooling to visualize differences against reference renders

## Editing Algorithm Verification

Beyond raw processing, each per-pixel editing adjustment (exposure, contrast, highlights/shadows, whites/blacks, HSL, color grading, vignette, etc.) should be verified against open-source reference implementations (darktable, RawTherapee) and visually compared against Lightroom/Capture One output.

This is a cross-cutting effort — analyze all editing features at once rather than per-feature:

1. **Reference audit**: read darktable/RawTherapee source for each adjustment type, document the exact algorithm and compare to ours
2. **Visual comparison**: process the same image with identical parameters in AgX vs Lightroom, diff the output
3. **Refine**: adjust weight curves, blending math, or parameter scaling where our results diverge from expected behavior

## Related

- [Color Management](color-management.md) — per-camera profiles improve starting-point accuracy
- [Tone Curves](tone-curves.md) — configurable base tone curves address the tone mapping gap
- [Ecosystem Interop](ecosystem-interop.md) — users importing presets from other tools expect similar results
