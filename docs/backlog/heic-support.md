# HEIC/HEIF Format Support

Add decoding support for HEIC/HEIF images (`.heic`, `.heif`), the default photo format on modern Apple devices.

## Sub-tasks

- [ ] **Evaluate codec libraries** — `libheif` via FFI vs pure-Rust options (if any mature)
- [ ] **Implement HEIC decoding** — add to the decode module, route `.heic`/`.heif` extensions
- [ ] **Extract EXIF metadata** — HEIC files carry EXIF metadata that the metadata module should handle
- [ ] **Add e2e test fixtures** — HEIC sample images for golden file comparison
- [ ] **Review patent/licensing** — HEVC codec has patent considerations; understand implications

## Considerations

- HEIC is the default photo format on iPhones since iOS 11. Users can't currently process these without converting to JPEG first (lossy).
- HEIF is a container format that can hold HEVC or AV1 (AVIF) encoded images.
- `libheif` is the most mature option but adds an FFI dependency (similar to LibRaw for RAW).

## Related

- [Ecosystem Interop](ecosystem-interop.md) — HEIC is part of the broader format support story
