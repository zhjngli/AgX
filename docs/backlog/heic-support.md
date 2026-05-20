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

## Known gaps (post-MVP follow-up)

These were surfaced during the initial HEIC support adversarial review and deliberately deferred. Each is a real concern but doesn't block initial decode shipping.

- **End-to-end coverage for Display P3 and 10-bit branches.** Display P3 synthetic fixture (`synthetic_p3_red.heic`) shipped with the wide-working-space migration; the 10-bit (`heif-enc -b 10 ...`) fixture is still pending. The 10-bit decode code path is reachable only by local manual testing until a synthetic 10-bit fixture lands, with noop-only goldens to keep repo size in check.
- **BT.2020 transfer curve handling.** After SP1 the matrix path for BT.2020 primaries exists (identity matrix to Rec.2020), but `probe_source_color_space` still routes BT.2020 inputs to the sRGB fallback with a warning because the BT.2020-specific OETF (and PQ/HLG HDR variants) requires separate transfer-curve handling. Track under the color-management epic's HDR sub-project.
- **Out-of-gamut clamping audit.** Done in SP1: aesthetic clamps were removed from intermediate stages, the LUT lookup goes through a sign-preserving sRGB-gamma bracket, and the final clamp lives at encode. Wide-gamut intermediates flow unclamped through the engine.
- **EXIF buffer shape consistency.** The HEIC EXIF extractor returns a raw TIFF (header + IFDs, no `Exif\0\0` prefix). The legacy raw-TIFF path (`metadata.rs::extract_metadata_raw_tiff`) returns bytes that include `Exif\0\0`. Downstream encode logic handles both today because of prefix-tolerant parsing in `img-parts`, but the asymmetry is a latent footgun. Decide on a canonical shape and reconcile.
- **Multiple EXIF blocks per HEIF.** `extract_heic_metadata` reads the first EXIF block only. iPhone files occasionally carry more than one; we silently drop the rest. Decide whether to merge or surface them when a real-world case emerges.
- **EXIF orientation tag double-application on output.** AgX correctly rotates pixel data per source EXIF orientation during decode (per the README feature), but the metadata-preservation path then copies the source orientation tag *unmodified* to the output file. Viewers that respect EXIF orientation (GitHub PR diff view, mdbook browser preview, macOS Preview in some configurations) then rotate the already-canonical pixels a second time, producing visibly rotated images. Surfaced by the sample-content-rework (2026-05-18) when iPhone HEICs rendered to PNG/JPEG ended up with `Orientation: Rotate 90 CW` baked into the output. Workaround used: post-process committed images with `exiftool -Orientation= -overwrite_original`. Real fix: at encode time, after applying orientation to pixel data, reset the orientation tag in the preserved EXIF to `Horizontal (normal)` (orientation=1) or strip the tag entirely. Affects all source formats that can carry an orientation tag (HEIC, JPEG, raw via embedded EXIF); HEIC is where users actually hit it because iPhones routinely capture with orientation≠1.

## Related

- [Ecosystem Interop](ecosystem-interop.md) — HEIC is part of the broader format support story
