# lut

## Purpose

Parse, store, and look up 3D color LUTs for creative color grading.

## Public API

- `Lut3D` -- struct holding the 3D lattice (`title`, `size`, `domain_min`, `domain_max`, `table`)
- `Lut3D::from_cube_str(text)` -- parse a `.cube` format string
- `Lut3D::from_cube_file(path)` -- load and parse a `.cube` file
- `Lut3D::lookup(r, g, b)` -- trilinear interpolation returning transformed `(r, g, b)`

Internal submodule `cube` contains the `.cube` parser.

## Extension Guide

To support a new LUT format (e.g., `.3dl`):

1. Add a new submodule (e.g., `lut/threedl.rs`) with a parse function returning `Lut3D`.
2. Add a `from_3dl_str` / `from_3dl_file` constructor on `Lut3D`.
3. No changes needed to `lookup` -- all formats produce the same `Lut3D` struct.

## Does NOT

- Apply LUTs to images (the engine does that).
- Know about presets, encoding, or decoding.
- Perform color space conversion (assumes sRGB-gamma input; engine wraps the sample with a gamma-Rec.2020 ↔ gamma-sRGB conversion bracket so the LUT module itself stays gamut-agnostic and sRGB-gamma-domain).

## Key Decisions

- **Single `Lut3D` struct for all formats.** Parsing is format-specific; lookup is format-agnostic.
- **Trilinear interpolation.** Values between lattice points are blended from the 8 surrounding cube vertices, giving smooth color transitions.
- **Input clamping.** `lookup` clamps inputs to the domain range rather than erroring, matching standard LUT behavior.
- **LUT input domain stays sRGB-gamma** — third-party `.cube` files written for the sRGB workflow remain portable; the engine handles the wide-working-space conversion at the call site.
