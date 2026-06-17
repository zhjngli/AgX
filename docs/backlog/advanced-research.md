# Advanced Research

Research-heavy features with significant implementation complexity: AI editing, HDR merge, panorama, focus stacking, tethered shooting.

## Sub-tasks

- [ ] **AI-assisted editing** — suggest preset adjustments based on image content (scene detection, subject recognition). Large dependency footprint (ML runtime)
- [ ] **HDR merge** — combine multiple exposures into a single HDR image. Well-studied algorithms (Debevec, Mertens exposure fusion). Requires alignment and ghost removal
- [ ] **Panorama stitching** — combine overlapping images into a wide-field composite. Requires feature detection, homography estimation, seam blending. OpenCV via FFI could provide the heavy lifting
- [ ] **Focus stacking** — combine images with different focus planes for extended depth of field. Common in macro and landscape photography
- [ ] **Tethered shooting** — direct camera control and live preview via USB. Platform-specific (libgphoto2 on Linux, proprietary SDKs on macOS/Windows). High complexity, niche use case

## Considerations

- All of these are standalone features that produce input images, which then go through the normal adjustment workflow — they don't affect the core editing pipeline.
- Each feature has significant implementation complexity and external dependencies.
- Priority should be driven by user demand. HDR merge and panorama stitching are the most commonly requested.

## Related

- [Local Adjustments](local-adjustments.md) — AI masking could auto-select subjects/sky
- [Processing Parity](processing-parity.md) — HDR merge quality depends on tone mapping accuracy
- [Performance](performance.md) — these features benefit from GPU acceleration
