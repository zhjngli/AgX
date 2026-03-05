# LUT Support Design

**Date**: 2026-02-16
**Status**: Approved

## Overview

Add support for applying 3D LUTs (Look-Up Tables) to images via the `.cube` file format. LUTs are pre-computed color transformations widely used in photography and video for color grading, film emulations, and creative looks.

This is the first step toward a full LUT ecosystem (apply → generate → bundle/marketplace). The focus is on applying `.cube` LUTs as part of the editing pipeline.

## Background: What Is a LUT?

A LUT maps input RGB values to output RGB values via a pre-computed table. Unlike parametric adjustments (exposure = multiply by 2^stops), a LUT is opaque — it doesn't encode *what* transformation it represents, just the input→output mapping.

**1D LUT**: One lookup curve per channel. Fast but limited — cannot do cross-channel color transformations. (Out of scope for now.)

**3D LUT**: A cube of output RGB values indexed by input R, G, B. A 33x33x33 cube (common size) contains 35,937 entries. Any input RGB that falls between lattice points is trilinearly interpolated from the 8 surrounding entries. Can represent arbitrary color transforms including cross-channel effects.

## The `.cube` Format

The `.cube` format is the de facto standard for LUT interchange. It was defined by Adobe for use in DaVinci Resolve and is supported by virtually every editing tool (Lightroom, Photoshop, Resolve, Capture One, Final Cut, etc.).

It's a plain text file:

```
TITLE "Film Emulation"
LUT_3D_SIZE 33
DOMAIN_MIN 0.0 0.0 0.0
DOMAIN_MAX 1.0 1.0 1.0
# Comments start with #
0.000000 0.000000 0.000000
0.003906 0.000000 0.000000
...
```

**Header keywords:**
- `TITLE "name"` — optional descriptive title
- `LUT_3D_SIZE N` — creates N×N×N entries (common sizes: 17, 33, 65)
- `DOMAIN_MIN r g b` — minimum input values (default 0.0 0.0 0.0)
- `DOMAIN_MAX r g b` — maximum input values (default 1.0 1.0 1.0)
- Lines starting with `#` are comments

**Data layout:** Each line is three space-separated floats (R G B output values). Entries are ordered with R changing fastest, then G, then B.

**Interpolation:** For input values between lattice points, trilinear interpolation is used — the 8 surrounding cube vertices are looked up and blended based on the fractional position within the cell.

## Color Space Considerations

**LUTs are color-space-dependent.** A LUT is just a numeric mapping — it doesn't know what color space its values represent. The LUT and the engine must agree on the input color space, or the results will be wrong.

Most creative `.cube` LUTs (film emulations, color grades) expect **sRGB gamma** input in the 0.0–1.0 range. This is because colorists create LUTs while looking at a screen, which displays sRGB.

Some LUTs (particularly in video workflows) expect **log** input (S-Log3, LogC, etc.) for converting camera log footage to display color. These are out of scope for now but the architecture should not preclude them.

**Our approach:** Apply the LUT in sRGB gamma space, which is where our pipeline already operates for perceptual adjustments. This is correct for the vast majority of LUTs.

**Future:** When we add wider color space support and a pluggable pipeline architecture, LUTs can declare their expected input/output color space, and the pipeline can auto-insert conversions. See `docs/ideas/pluggable-pipeline.md` for the pluggable pipeline vision.

## Design

### Data Model

```rust
/// A 3D Look-Up Table for color transformation.
pub struct Lut3D {
    pub title: Option<String>,
    pub size: usize,              // N in N×N×N (e.g. 33)
    pub domain_min: [f32; 3],     // input range minimum per channel
    pub domain_max: [f32; 3],     // input range maximum per channel
    pub table: Vec<[f32; 3]>,     // N^3 RGB output entries
}
```

**Methods:**
- `Lut3D::from_cube_str(text: &str) -> Result<Self>` — parse .cube text
- `Lut3D::from_cube_file(path: &Path) -> Result<Self>` — parse .cube file
- `Lut3D::lookup(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32)` — trilinear interpolation lookup

We write our own parser (the format is ~100 lines to parse) rather than depending on poorly-maintained external crates (`lut-cube`, `lut_parser`).

### Pipeline Integration

The LUT is applied after all tone adjustments in sRGB gamma space, before converting back to linear:

```
Original (linear)
  → White balance (linear)
  → Exposure (linear)
  → Linear → sRGB gamma
  → Contrast, highlights, shadows, whites, blacks (sRGB gamma)
  → LUT application (sRGB gamma)     ← NEW
  → sRGB gamma → linear
  → Encode to output file
```

This matches the standard workflow in Lightroom and Resolve: parametric adjustments set the base, then the LUT applies a color grade on top.

### Engine Changes

`Parameters` gets a new optional field:

```rust
pub struct Parameters {
    // ... existing fields ...
    pub lut: Option<Lut3D>,
}
```

`Engine::render()` applies the LUT after tone adjustments if `params.lut` is `Some`.

### Preset Format

The TOML preset gets an optional `[lut]` section:

```toml
[metadata]
name = "Cinematic Film"

[tone]
exposure = 0.3
contrast = 10.0

[lut]
path = "cinematic-film.cube"
```

- `path` is resolved relative to the preset file's directory
- Missing `[lut]` section or omitted `path` means no LUT
- Invalid or missing .cube file returns a `Preset` error

### CLI Changes

Both subcommands get a `--lut` flag:

```bash
# Apply a preset that references a LUT
cargo run -p oxiraw-cli -- apply -i photo.jpg -p preset.toml -o out.jpg

# Apply a standalone LUT with inline adjustments
cargo run -p oxiraw-cli -- edit -i photo.jpg -o out.jpg --lut film.cube --exposure 0.5

# Just apply a LUT, no other adjustments
cargo run -p oxiraw-cli -- edit -i photo.jpg -o out.jpg --lut film.cube
```

### Module Structure

```
crates/oxiraw/src/
├── lut/
│   ├── mod.rs          # Lut3D struct, lookup, trilinear interpolation
│   └── cube.rs         # .cube format parser
```

### Documentation Deliverables

- **`docs/reference/color-spaces.md`** — linear vs sRGB gamma explanation, why operations live where they do, how LUTs fit in
- **`docs/reference/lut-format.md`** — .cube format reference, how oxiraw parses it, supported features, limitations
- **Doc comments** on all public LUT types and methods
- **`README.md`** — add LUT section with CLI and library usage examples
- **`example/`** — add a sample .cube LUT file

### Error Handling

New error variants:

```rust
pub enum OxirawError {
    // ... existing ...
    #[error("LUT error: {0}")]
    Lut(String),
}
```

Parse errors include line numbers where possible. Invalid LUT size, mismatched entry count, malformed floats all produce clear error messages.

## Scope

**In scope:**
- 3D LUT parsing from `.cube` files
- Trilinear interpolation
- Pipeline integration (after tone adjustments, sRGB gamma space)
- Preset `[lut]` section with relative path resolution
- CLI `--lut` flag
- Reference documentation
- Sample .cube file in `example/`

**Out of scope (future):**
- 1D LUT support
- LUT generation/export from engine parameters
- Shaper LUTs (1D pre-processing before 3D LUT)
- Non-sRGB input LUTs (log, linear)
- Pluggable pipeline stages with auto color space conversion
- Bundled LUT packs / marketplace
- Tetrahedral interpolation (trilinear is sufficient and simpler)

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| Write our own .cube parser | Format is trivial (~100 lines). Existing Rust crates (`lut-cube`, `lut_parser`) are poorly documented and unmaintained. |
| 3D LUTs only (no 1D) | 3D covers all interesting use cases. 1D is just a per-channel curve — we can add it later if needed. |
| Trilinear interpolation | Standard, well-understood, good quality. Tetrahedral is marginally better but more complex. |
| Apply after tone adjustments | Matches Lightroom/Resolve workflow. Most creative LUTs expect sRGB gamma input after basic corrections. |
| LUT on Parameters struct | Keeps the declarative model — a preset is one bag of settings. |
| Relative path resolution for preset LUTs | Presets and their LUTs can live together in a directory, making them portable. |
