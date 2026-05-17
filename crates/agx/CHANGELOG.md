# Changelog

All notable changes to this crate are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed (breaking)

- **Working space widened to linear Rec.2020.** Stages 1–4 (white balance, exposure, dehaze, noise reduction) run in linear Rec.2020; stages 5–8 (per-pixel adjustments, detail, grain, vignette) run in gamma-encoded Rec.2020. Decode converts every input format to linear Rec.2020 at the boundary; encode converts linear Rec.2020 to 8-bit sRGB at output. The public function `linear_to_srgb_rgb8` was renamed to `encode_linear_rec2020_to_srgb_rgb8` and its input contract changed from linear sRGB to linear Rec.2020. Library consumers must migrate at this release.
- **HEIC Display P3 inputs preserve wide gamut end-to-end.** The decoder previously squashed P3 to sRGB at the boundary; iPhone HEIC captures now keep their wider gamut through the entire pipeline.
- **Aesthetic intermediate clamps removed.** Stage outputs no longer clip wide-gamut headroom; only domain-safety clamps (LUT-index, HSL `[0, 1]` palette guard, color-grading luminance weight) remain. The final clamp to display gamut happens at encode.

### Added

- `crate::color_space` module exposing the Rec.2020 ↔ sRGB matrices, the sign-preserving sRGB transfer curve (`srgb_curve_signed` / `_inverse`), and the `wrap_lut_lookup` helper.
- `LinearRec2020` and `GammaRec2020` variants on the `ColorSpace` enum.
- Synthetic Display P3 HEIC e2e fixture (`synthetic_p3_red.heic`) demonstrating wide-gamut preservation.

## [0.1.0] - 2026-04-26

First public release of `agx-photo` to crates.io.

### Added

- **Render pipeline.** Stage-based architecture with the always-re-render-from-original invariant: every render runs from the decoded source image, never from a cached intermediate.
- **Decode.** JPEG, PNG, TIFF via the `image` crate; RAW (Fuji RAF, Sony ARW, Nikon NEF, Canon CR3, and other vendor formats) via LibRaw bindings.
- **Adjustments.** Exposure, contrast, highlights/shadows, whites/blacks, white balance, HSL, color grading (3-way lift/gamma/gain), tone curves, vignette, dehaze (Dark Channel Prior), detail pass (sharpening, clarity, texture), noise reduction (à trous wavelet), and film grain (chromatic, blur-based particle sizing).
- **Presets.** TOML format with composability via `extends`, schema validation, and a portable human-readable layout.
- **LUTs.** `.cube` format 3D LUTs with trilinear interpolation.
- **GPU acceleration.** Opt-in path via `wgpu` compute shaders covering all 9 pipeline stages. CPU remains the canonical path for deterministic cross-platform output.
- **Parallelization.** Per-pixel adjustments, separable Gaussian blur, denoise wavelet passes, grain generation, and dehaze use `rayon` for multi-core speedup.
- **Profiling.** Feature-gated render performance instrumentation, zero overhead when disabled.

[Unreleased]: https://github.com/zhjngli/AgX/compare/agx-photo-v0.1.0...HEAD
[0.1.0]: https://github.com/zhjngli/AgX/releases/tag/agx-photo-v0.1.0
