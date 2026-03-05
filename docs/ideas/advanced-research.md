# Advanced Research

**Category:** Advanced
**Status:** Backlog

## Problem / Opportunity

AI-assisted editing, HDR merge, panorama stitching, focus stacking, and tethered shooting are advanced capabilities that would position oxiraw as a complete photography solution. These are research-heavy features with significant implementation complexity but high user value.

## Key Considerations

- **AI-assisted editing**: Suggest preset adjustments based on image content (scene detection, subject recognition). Could use pre-trained models for scene classification and parameter suggestion. Large dependency footprint (ML runtime)
- **HDR merge**: Combine multiple exposures into a single high-dynamic-range image. Requires alignment (for handheld shots), tone mapping, and ghost removal (for moving subjects). Well-studied algorithms (Debevec, Mertens exposure fusion)
- **Panorama stitching**: Combine overlapping images into a wide-field composite. Requires feature detection, homography estimation, seam blending. Existing libraries (OpenCV via FFI) could provide the heavy lifting
- **Focus stacking**: Combine images with different focus planes for extended depth of field. Requires alignment, focus detection per-region, and blending. Common in macro and landscape photography
- **Tethered shooting**: Direct camera control and live preview via USB. Platform-specific (libgphoto2 on Linux, proprietary SDKs on macOS/Windows). High complexity, niche use case
- All of these are standalone features that don't affect the core editing pipeline — they produce input images that then go through the normal adjustment workflow

## Related

- [Local Adjustments](local-adjustments.md) — AI masking could auto-select subjects/sky
- [Processing Parity](processing-parity.md) — HDR merge quality depends on tone mapping accuracy
- [Platform and Distribution](platform-and-distribution.md) — these features benefit from GPU acceleration
