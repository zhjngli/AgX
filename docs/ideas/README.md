# Ideas Backlog

Future features and ideas for AgX. Nothing here is committed — pick an idea file to explore. When an idea is picked up for implementation, remove its file from this directory.

## Editing — Per-Pixel Features

These operate on each pixel independently and fit into the current single-pass engine with no architecture changes.

| File | Summary |
|------|---------|
| [tone-curves.md](tone-curves.md) | Parametric and point curves for precise tonal control |
| [color-grading.md](color-grading.md) | 3-way color wheels for shadow/midtone/highlight grading |
| [film-and-grain.md](film-and-grain.md) | Film grain simulation (coordinate-seeded noise) and film emulation database |

## Editing — Neighborhood Operations

These need access to surrounding pixels and require a multi-pass approach (separate pass over the full image buffer after per-pixel adjustments). Once we have 3+ of these, formalize into a pluggable stage-based pipeline.

| File | Summary |
|------|---------|
| [sharpening-and-detail.md](sharpening-and-detail.md) | Sharpening, clarity, texture (convolution kernels), and noise reduction |
| [dehaze.md](dehaze.md) | Atmospheric haze removal (local region analysis) |
| [local-adjustments.md](local-adjustments.md) | Brushes, gradients, and radial filters for per-region edits |
| [geometric-corrections.md](geometric-corrections.md) | Lens corrections, perspective, crop and rotation |

## Pipeline & Infrastructure

| File | Summary |
|------|---------|
| [pluggable-pipeline.md](pluggable-pipeline.md) | Stage-based render pipeline with caching and color-space awareness (build after 3+ neighborhood ops) |
| [preset-tooling.md](preset-tooling.md) | Schema versioning, validation, and authoring shortcuts |
| [multi-preset-cli.md](multi-preset-cli.md) | Decode once, apply N presets per CLI invocation (cuts e2e test time) |

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
| [platform-and-distribution.md](platform-and-distribution.md) | REST API, GPU, WASM, batch processing, preset marketplace |
| [ui.md](ui.md) | Desktop and web UI, histogram, before/after, undo/redo |
| [advanced-research.md](advanced-research.md) | AI editing, HDR merge, panorama, focus stacking, tethered shooting |
