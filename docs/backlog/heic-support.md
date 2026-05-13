# HEIC/HEIF Format Support

Add decoding support for HEIC/HEIF images (`.heic`, `.heif`), the default photo format on modern Apple devices.

## Sub-tasks

- [x] **Evaluate codec libraries** — `libheif` via FFI vs pure-Rust options (if any mature)
- [x] **Implement HEIC decoding** — add to the decode module, route `.heic`/`.heif` extensions
- [x] **Extract EXIF metadata** — HEIC files carry EXIF metadata that the metadata module should handle
- [x] **Add e2e test fixtures** — HEIC sample images for golden file comparison
- [x] **Review patent/licensing** — HEVC codec has patent considerations; understand implications
- [ ] **HEIF encode (future)** — write `.heic` (HEVC codec, Apple-compatible, patent-encumbered) or `.avif` (AV1 codec, royalty-free) outputs. Codec choice deferred until concrete demand. Decode lands first; encode is a separate effort with its own licensing review.
- [ ] **Auxiliary HEIF images (future)** — iPhone `.heic` files often carry depth maps, burst frames, or alternate-exposure aux images alongside the primary. Initial decode reads only the primary image; exposing the others (e.g., for portrait-mode depth-aware edits or burst selection) warrants its own design.
- [ ] **XMP handling (selective namespace preservation, future)** — initial HEIC decode extracts EXIF only. XMP packets in iPhone HEIC files mix namespaces that should *not* be naively round-tripped: HDR gain map refs become meaningless once we render to 8-bit sRGB; Apple content credentials become *false* after AgX edits (the file is no longer "iPhone signed"); Lightroom edit-history XMP is misleading because we applied a different edit; Live Photo links and depth-map refs become stale because their companion data isn't carried through. Namespaces that *are* safe to preserve: IPTC keywords/ratings/captions/copyright, Apple photo identifiers (for Photos-app sync). Future design: enumerate namespaces, classify each as preserve / strip / rewrite, implement at decode → metadata module boundary.

## Considerations

- HEIC is the default photo format on iPhones since iOS 11. Users can't currently process these without converting to JPEG first (lossy).
- HEIF is a container format that can hold HEVC or AV1 (AVIF) encoded images.
- `libheif` is the most mature option but adds an FFI dependency (similar to LibRaw for RAW).

## Related

- [Ecosystem Interop](ecosystem-interop.md) — HEIC is part of the broader format support story
