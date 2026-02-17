# Oxiraw Architecture Design

**Date**: 2026-02-14
**Status**: Approved

## Overview

Oxiraw is an open-source photo editing library and CLI written in Rust. It provides a portable, human-readable preset format (TOML) for storing photo editing parameters, and an engine capable of applying those edits to images across standard and raw formats.

The long-term vision includes a full-featured editing suite, a preset marketplace for sharing, and potential import/export of presets from proprietary ecosystems (Lightroom, Capture One).

## Core Principles

1. **Open and readable presets**: Editing parameters stored in TOML — easy to read, share, version control, and hand-edit.
2. **Always-re-render-from-original**: The engine preserves the original image data and re-renders from scratch whenever any parameter changes. This makes the system appear order-independent from the user's perspective, even though the internal rendering pipeline applies adjustments in a fixed order. This was a deliberate design decision — photo editing operations are not mathematically commutative, so true order-independence at the algorithm level is not feasible. The fixed internal order is an implementation detail that users and presets never see or control.
3. **Declarative parameter state**: Presets and the engine define a set of parameter values (exposure, contrast, etc.), not an ordered sequence of operations. The engine interprets the full parameter state each render.
4. **Extensibility**: The architecture should support adding new adjustment types (HSL, curves, color grading, sharpening, NR, etc.) without changing the core engine model.

## Architecture

### Pipeline

```
Input File ──► Decode ──► Engine (holds original + params) ──► Render ──► Encode ──► Output File
                                      ▲
                                      │
                              Preset (TOML) or
                              direct param setting
```

### Decode

- **Standard formats** (JPEG, PNG, TIFF): via the `image` crate.
- **Raw formats** (RAF, ARW, NEF, CR3, DNG, etc.): via LibRaw FFI bindings (`libraw-rs` or `rsraw`). LibRaw supports 1000+ cameras and is the de facto standard for raw decoding.
- All decoded images are normalized to a common internal representation (linear RGB buffer).

### Engine

The `Engine` is the central struct:
- Owns the original decoded image buffer (immutable after decode).
- Owns the current parameter state (all adjustment values, defaulting to neutral).
- `render()` applies all adjustments from scratch in a fixed internal order and returns the result.
- Individual parameter setters update state and mark the render as stale.

For future UI integration: on any parameter change, the engine re-renders from the original. Caching strategies (lower-res preview, intermediate caching) can be layered on top without changing the core model.

### Adjustments (MVP)

MVP adjustment parameters — all default to neutral (no change):

- **Exposure**: stops, range -5.0 to +5.0
- **Contrast**: range -100 to +100
- **Highlights**: range -100 to +100
- **Shadows**: range -100 to +100
- **Whites**: range -100 to +100
- **Blacks**: range -100 to +100
- **White balance temperature**: Kelvin
- **White balance tint**: green/magenta shift

Future: HSL (hue/saturation/luminance per channel), tone curves (parametric + point, per-channel), color grading (3-way wheels), sharpening, noise reduction, grain, dehaze, clarity, texture, lens corrections, vignetting.

### Preset Format (TOML)

> **Note**: The preset format is illustrative and subject to change. We want flexibility to support features like composable presets, shortcut options, preset inheritance, and other patterns that may emerge during development. Do not consider any specific schema locked in.

Example (illustrative):

```toml
[metadata]
name = "Warm Golden Hour"
version = "1.0"
author = "zli"

[tone]
exposure = 0.5
contrast = 15
highlights = -30
shadows = 25

[white_balance]
temperature = 6200
tint = 10
```

Unspecified values default to neutral. Serialization via `serde` + `toml` crate.

### CLI

> **Note**: CLI interface is illustrative and subject to change. Exact subcommands, flags, and interaction patterns will evolve as we develop the library.

The CLI is a thin wrapper over the library. Possible commands include applying presets to images, editing with inline parameters, inspecting presets, and exporting settings. The library crate should be fully functional independent of the CLI.

### Encode / Output

Output to standard formats (JPEG, PNG, TIFF) via the `image` crate. Quality settings and format-specific options configurable.

## Project Structure

```
oxiraw/
├── Cargo.toml              # workspace root
├── crates/
│   ├── oxiraw/             # core library crate
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── decode/     # format decoding (LibRaw FFI + image crate)
│   │       ├── engine/     # rendering engine
│   │       ├── adjust/     # adjustment algorithms
│   │       ├── preset/     # TOML preset loading/saving
│   │       └── encode/     # output encoding
│   └── oxiraw-cli/         # thin CLI wrapper
│       └── src/
│           └── main.rs
└── docs/
    ├── plans/              # design and implementation plans
    └── ideas/              # future feature exploration
```

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `image` | Standard format I/O, image buffer types |
| `imageproc` | Pixel-level processing operations |
| `palette` | Color space conversions (HSL, Lab, XYZ) |
| `libraw-rs` / `rsraw` | Raw format decoding via LibRaw FFI |
| `toml` + `serde` | Preset serialization |
| `clap` | CLI argument parsing |
| `thiserror` | Error types |

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Rust | Performance for pixel-level math, strong type system, personal learning goal |
| LibRaw via FFI for raw decoding | Covers 1000+ cameras including CR3. Pure Rust alternatives lack CR3 support |
| Always-re-render-from-original | Gives user-facing order-independence. Operations are not mathematically commutative, so this is the standard approach used by Lightroom, darktable, RawTherapee |
| TOML for presets | Human-readable, supports comments, first-class Rust support, well-typed |
| Declarative parameter state | Presets are just values, no ordering. Matches photographer mental model |
| Workspace with library + CLI | Library is the core product; CLI is a thin consumer. Future UIs, WASM targets, etc. can also consume the library |

## Color Space (MVP Decision)

**For the MVP, we use sRGB exclusively.**

- **sRGB** is the standard color space for displays, web, and consumer photography. JPEG/PNG files are sRGB by default. Most monitors display sRGB.
- **Adobe RGB** is a wider-gamut space for professional print workflows (more greens/cyans). Not needed for MVP.
- **ProPhoto RGB** is even wider, used internally by Lightroom. Overkill for now.
- **Display P3** is Apple's wide-gamut display standard. Future consideration.
- **Raw files** have no inherent color space — they're sensor data. Color space is applied during demosaicing. When we add LibRaw integration, the raw decoder will output to sRGB.

What this means for the implementation:
- Decoded standard images (JPEG/PNG/TIFF) are assumed to be sRGB.
- We convert sRGB → linear sRGB (using palette crate's `Srgb` → `LinSrgb`) for internal processing.
- Exposure and white balance operate in linear sRGB space.
- Contrast, highlights, shadows, whites, blacks operate in sRGB gamma space.
- Output is encoded back to sRGB gamma for saving.
- No ICC profile handling in MVP — we assume sRGB throughout.

Future: wider gamut support (Adobe RGB, ProPhoto RGB, Display P3), ICC profile reading/embedding, color space conversion between working spaces.

## Open Questions

- Final project name (oxiraw is a working name)
- Exact preset schema — intentionally flexible for now
- CLI subcommand structure — will evolve with the library
- Whether to support composable/layered presets (apply preset A, then overlay preset B)
- LibRaw binding crate choice: `rsraw` vs `libraw-rs` vs `libraw-sys` (need to evaluate maturity)
