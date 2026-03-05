# Ecosystem Interop

**Category:** Ecosystem
**Status:** Backlog

## Problem / Opportunity

Photographers have existing preset libraries in Lightroom (XMP), Capture One (.costyle), darktable (XMP sidecar), and RawTherapee (.pp3). Import/export support lets users bring their existing work into oxiraw and share presets across tools. Sidecar files store per-image edits alongside source files, enabling non-destructive workflows without a database.

## Key Considerations

- **Lightroom XMP import**: Parse Adobe Camera Raw XMP presets and convert to oxiraw format. XMP is XML-based with Adobe-specific schemas
- **Capture One .costyle import**: Parse Capture One styles (XML-based)
- **darktable XMP import**: Parse darktable sidecar files (different XMP schema than Adobe)
- **RawTherapee .pp3 import**: Parse RawTherapee processing profiles (INI-style format)
- **Export**: Generate XMP/costyle/pp3 from oxiraw presets. This is lossy — not all parameters map 1:1 between tools
- **Sidecar files**: Store per-image edits alongside the source file (like Lightroom's .xmp sidecars). Format could be oxiraw's native TOML preset with a naming convention (e.g., `photo.cr2.oxiraw`)
- Parameter mapping is inherently approximate — different tools use different algorithms for the same named adjustment
- Import priority should be driven by user demand (Lightroom XMP likely has the largest user base)

## Related

- [Preset Composability](preset-composability.md) — imported presets may be partial
- [Preset Tooling](preset-tooling.md) — validation helps catch import errors
- [Processing Parity](processing-parity.md) — imported presets may produce different results due to algorithm differences
