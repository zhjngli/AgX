# Color Management

**Category:** Color
**Status:** Backlog

## Problem / Opportunity

oxiraw currently works exclusively in sRGB. Professional workflows require wider gamuts (Adobe RGB for print, ProPhoto RGB for lossless editing, Display P3 for Apple displays), ICC profile handling, and accurate per-camera color rendering. Full color management is the difference between a consumer tool and a professional one.

## Key Considerations

- **Adobe RGB**: Wider gamut for professional print workflows — more greens and cyans than sRGB
- **ProPhoto RGB**: Very wide gamut used internally by Lightroom for lossless editing. Avoids clipping colors during aggressive adjustments
- **Display P3**: Apple's wide-gamut display standard for modern monitors
- **ICC profile reading**: Read embedded ICC profiles from input images to determine their actual color space
- **ICC profile embedding**: Embed correct ICC profiles in output images so downstream software interprets colors correctly
- **Color space conversion**: Convert between working spaces (sRGB, Adobe RGB, ProPhoto RGB, etc.)
- **Soft proofing**: Preview how an image will look in a different color space (e.g., CMYK for print) — useful for print preparation
- **lcms2 integration**: The `lcms2` Rust crate provides production-quality ICC profile handling. Major external dependency but the standard approach
- **Per-camera color matrices**: Custom color matrices for each camera model to improve color accuracy from raw files. DCP (DNG Camera Profile) and ICC camera profiles provide this data
- Current sRGB-only invariant (ARCHITECTURE.md core invariant #3) must be relaxed when this work begins

## Related

- [Pluggable Pipeline](pluggable-pipeline.md) — color-space-aware stages enable automatic conversions
- [Processing Parity](processing-parity.md) — per-camera profiles improve color accuracy vs reference processors
- [Ecosystem Interop](ecosystem-interop.md) — ICC profiles matter for cross-tool compatibility
