# Ideas Backlog

Future features and ideas for AgX. Nothing here is committed — pick an idea file to explore. When an idea is picked up for implementation, remove its file from this directory.

Ideas are roughly ordered by alignment with the project philosophy: preset-first batch editing via CLI and API. Features that make presets more expressive or batch workflows faster come first.

## Editing — Per-Pixel Features

All per-pixel editing features are now implemented: exposure, contrast, highlights, shadows, whites, blacks, white balance, HSL adjustments, vignette, color grading, and tone curves. No remaining ideas in this category.

## Editing — Neighborhood Operations

These need access to surrounding pixels and require a multi-pass approach (separate pass over the full image buffer after per-pixel adjustments). Once we have 3+ of these, formalize into a pluggable stage-based pipeline.

Some of these (sharpening, film grain) are preset-friendly — they apply uniformly and make sense in batch workflows. Others (local adjustments, geometric corrections) are more photo-specific and lower priority for the current preset-first direction.

| File | Summary | Notes |
|------|---------|-------|
| [sharpening-and-detail.md](sharpening-and-detail.md) | Sharpening, clarity, texture (convolution kernels), and noise reduction | Preset-friendly |
| [film-and-grain.md](film-and-grain.md) | Film grain simulation and film emulation database | Algorithm TBD (may be per-pixel or neighborhood) |
| [dehaze.md](dehaze.md) | Atmospheric haze removal (local region analysis) | |
| [local-adjustments.md](local-adjustments.md) | Brushes, gradients, and radial filters for per-region edits | Photo-specific, lower priority |
| [geometric-corrections.md](geometric-corrections.md) | Lens corrections, perspective, crop and rotation | Photo-specific, lower priority |

## Pipeline & Infrastructure

| File | Summary |
|------|---------|
| [performance.md](performance.md) | Render parallelization, buffer reduction, and other potential optimizations (needs profiling) |
| [preset-tooling.md](preset-tooling.md) | Schema versioning, validation, and authoring shortcuts |
| [multi-preset-cli.md](multi-preset-cli.md) | Decode once, apply N presets per CLI invocation (cuts e2e test time) |
| [pluggable-pipeline.md](pluggable-pipeline.md) | Stage-based render pipeline with caching and color-space awareness (build after 3+ neighborhood ops) |

## Color & Ecosystem

| File | Summary |
|------|---------|
| [color-management.md](color-management.md) | Wide gamut, ICC profiles, per-camera color matrices |
| [ecosystem-interop.md](ecosystem-interop.md) | XMP/costyle/pp3 import/export and sidecar files |
| [heic-support.md](heic-support.md) | HEIC/HEIF format decoding support |
| [processing-parity.md](processing-parity.md) | Understanding and reducing rendering differences vs other editors |

## Platform & UI

| File | Summary |
|------|---------|
| [platform-and-distribution.md](platform-and-distribution.md) | REST API, GPU, WASM, preset marketplace |
| [ui.md](ui.md) | Desktop and web UI, histogram, before/after, undo/redo |
| [advanced-research.md](advanced-research.md) | AI editing, HDR merge, panorama, focus stacking, tethered shooting |
