# Changelog

All notable changes to this crate are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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
