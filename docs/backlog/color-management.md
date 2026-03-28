# Color Management

Wide gamut support, ICC profiles, and per-camera color matrices for AgX.

## Sub-tasks

- [ ] **ICC profile reading** — read embedded ICC profiles from input images to determine their actual color space
- [ ] **ICC profile embedding** — embed correct ICC profiles in output images so downstream software interprets colors correctly
- [ ] **Color space conversion** — convert between working spaces (sRGB, Adobe RGB, ProPhoto RGB, Display P3)
- [ ] **lcms2 integration** — the `lcms2` Rust crate provides production-quality ICC profile handling (major external dependency)
- [ ] **Per-camera color matrices** — custom color matrices for each camera model (DCP/ICC camera profiles) to improve raw color accuracy
- [ ] **Soft proofing** — preview how an image will look in a different color space (e.g., CMYK for print)
- [ ] **Relax sRGB-only invariant** — update ARCHITECTURE.md core invariant #3 when this work begins

## Considerations

- **Adobe RGB**: wider gamut for professional print — more greens and cyans than sRGB.
- **ProPhoto RGB**: very wide gamut used internally by Lightroom for lossless editing. Avoids clipping during aggressive adjustments.
- **Display P3**: Apple's wide-gamut display standard for modern monitors.
- lcms2 is the industry standard but adds a significant external dependency.
- Per-camera profiles are what make raw converters produce different results — this is a deep rabbit hole.

## Related

- [Pluggable Pipeline](pluggable-pipeline.md) — color-space-aware stages enable automatic conversions
- [Processing Parity](processing-parity.md) — per-camera profiles improve color accuracy vs reference processors
- [Ecosystem Interop](ecosystem-interop.md) — ICC profiles matter for cross-tool compatibility
