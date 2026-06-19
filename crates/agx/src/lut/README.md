# lut

## Purpose

Parse, store, and look up 3D color LUTs for creative color grading.

## Public API

- `Lut3D` -- struct holding the 3D lattice (`title`, `size`, `domain_min`, `domain_max`, `table`, `encoding`)
- `Lut3D::from_cube_str(text)` -- parse a `.cube` format string
- `Lut3D::from_cube_file(path)` -- load and parse a `.cube` file
- `Lut3D::lookup(r, g, b)` -- trilinear interpolation returning transformed `(r, g, b)`
- `LutEncoding` -- enum: `Srgb` (default) / `Linear`. Declares the color space the LUT was authored in. Both variants use sRGB primaries; `Srgb` uses the sRGB transfer curve (gamma-encoded), `Linear` is sRGB-primaries linear light.

Internal submodule `cube` contains the `.cube` parser.

## LUT Encoding

The `encoding` field on `Lut3D` tells the pipeline what color space the LUT expects as input. It is set from the `[lut] encoding` field in preset TOML when a preset loads a LUT; the `.cube` file format itself carries no encoding metadata.

- **`Srgb` (default)** — the LUT was authored with sRGB-gamma input values, as is universal for creative `.cube` LUTs produced in Lightroom, Resolve, Capture One, and similar tools.
- **`Linear`** — the LUT was authored with linear-light input values, using sRGB primaries. Use this for LUTs produced by tools that operate in a linear-light pipeline.

Both encodings assume sRGB primaries. The engine's CPU executor reads the declared encoding and auto-inserts a conversion to the matching color space before calling the LUT stage. The GPU path handles the bracket explicitly in its fused pass.

## Extension Guide

To support a new LUT format (e.g., `.3dl`):

1. Add a new submodule (e.g., `lut/threedl.rs`) with a parse function returning `Lut3D`.
2. Add a `from_3dl_str` / `from_3dl_file` constructor on `Lut3D`.
3. No changes needed to `lookup` -- all formats produce the same `Lut3D` struct.

## Does NOT

- Apply LUTs to images (the engine does that, via `LutStage`).
- Know about presets, encoding, or decoding.
- Perform color space conversion. The `encoding` field records the declared authoring space; the engine converts the buffer into that space before sampling and back out afterward.

## Key Decisions

- **Single `Lut3D` struct for all formats.** Parsing is format-specific; lookup is format-agnostic.
- **Trilinear interpolation.** Values between lattice points are blended from the 8 surrounding cube vertices, giving smooth color transitions.
- **Input clamping.** `lookup` clamps inputs to the domain range rather than erroring, matching standard LUT behavior.
- **Encoding declared per LUT, not per pixel.** The `encoding` field is set once at load time (from the preset's `[lut] encoding` key). The engine uses it to select the correct conversion bracket; `lookup` itself receives values already in the declared encoding's domain.
