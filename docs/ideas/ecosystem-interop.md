# Ecosystem Interop

Import/export presets from other photo editors and sidecar file support.

## Sub-tasks

- [ ] **Lightroom XMP import** — parse Adobe Camera Raw XMP presets and convert to AgX format (XML-based, Adobe-specific schemas). Likely highest demand
- [ ] **Capture One .costyle import** — parse Capture One styles (XML-based)
- [ ] **darktable XMP import** — parse darktable sidecar files (different XMP schema than Adobe)
- [ ] **RawTherapee .pp3 import** — parse RawTherapee processing profiles (INI-style format)
- [ ] **Export to XMP/costyle/pp3** — generate other formats from AgX presets. This is lossy — not all parameters map 1:1
- [ ] **Sidecar files** — store per-image edits alongside the source file (e.g., `photo.cr2.agx`), enabling non-destructive workflows without a database

## Considerations

- Parameter mapping is inherently approximate — different tools use different algorithms for the same named adjustment.
- Import priority should be driven by user demand (Lightroom XMP likely has the largest user base).
- Sidecar files would use AgX's native TOML preset format with a naming convention.

## Related

- [Preset Tooling](preset-tooling.md) — validation helps catch import errors
- [Processing Parity](processing-parity.md) — imported presets may produce different results due to algorithm differences
