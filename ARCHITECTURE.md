# AgX Architecture

Read this file before making structural changes to the codebase.

**How to read this file:** `ARCHITECTURE.md` is the *contract* — the rules that govern the codebase boundaries. The discussion of *why* those rules exist lives in the [architecture explanation](docs/book/src/explanation/concepts/architecture.md) on the published site. Read this file when you need to look up or change a rule. Read the explanation when you want to understand the reasoning.

AgX is an open-source photo editing library and CLI in Rust. The architecture follows an always-re-render-from-original model with declarative presets.

## Module Dependency Graph

```
                    ┌──────────────┐
                    │   error.rs   │   (foundation — no deps on other modules)
                    └──────┬───────┘
                           │
         ┌─────────────────┼─────────────────┐
         ▼                 ▼                 ▼
   ┌──────────┐      ┌──────────┐      ┌──────────┐
   │  adjust   │      │   lut    │      │  decode   │
   └──────┬───┘      └─────┬────┘      └─────┬────┘
          │                │                  │
          │                │           ┌──────┴──────┐
          │                │           ▼             │
          │                │     ┌──────────┐        │
          │                │     │ metadata │        │
          │                │     └─────┬────┘        │
          │                │           │             │
          │           ┌────┘     ┌─────┘             │
          │           │          ▼                   │
          │           │    ┌──────────┐              │
          │           │    │  encode  │              │
          │           │    └──────────┘              │
          │           │                              │
          │    ┌──────┴─────┐                        │
          │    │   preset   │                        │
          │    └──────┬─────┘                        │
          │           │                              │
          └─────┬─────┘                              │
                ▼                                    │
          ┌──────────────┐                           │
          │    engine    │◄──────────────────────────┘
          │  ┌────┬────┐ │
          │  │CPU │GPU │ │   (runtime pipeline selection)
          │  └────┴────┘ │
          └──────┬───────┘
                 │
          ┌──────────────┐
          │   agx-cli    │   (consumer — depends on library only)
          └──────────────┘
```

## Dependency Rules

These rules are enforced by `crates/agx/tests/architecture.rs`.

| Module     | MUST NOT import from                              | May import from                                          |
|------------|---------------------------------------------------|----------------------------------------------------------|
| `adjust`   | engine, decode, encode, preset, lut, metadata     | external crates only (palette)                           |
| `lut`      | engine, decode, encode, preset, metadata           | error                                                    |
| `decode`   | engine, encode, preset, adjust, lut, metadata      | error                                                    |
| `metadata` | engine, preset, adjust, lut, encode                | error, decode (`is_raw_extension`, `raw::extract_raw_metadata`, `is_heic_extension`, `heic::extract_heic_metadata`) |
| `encode`   | engine, preset, adjust, lut, decode                | error, metadata (`ImageMetadata`)                        |
| `preset`   | decode, encode, metadata                           | engine (`Parameters`), lut (`Lut3D`), error              |
| `engine`   | no restrictions within library                     | adjust, lut, preset, error                               |
| agx-cli    | —                                                  | agx (library API only)                                   |
| agx-e2e    | —                                                  | agx, agx-cli (test-only crate, not part of the library/CLI dependency graph) |
| agx-docgen | —                                                  | agx (`docgen` feature), agx-cli (dev-only tool for generating reference docs) |
| agx-lut-gen| —                                                  | none (standalone build tool for generating .cube LUT files; no runtime deps) |

## Negative Constraints

What does NOT exist in each module -- violations of these constraints indicate a design problem.

- **adjust**: No image I/O. No file system access. No knowledge of presets or engine state. Pure pixel math only.
- **lut**: No image decoding/encoding. No preset parsing. Does not apply LUTs to images (that is the engine's job).
- **decode**: No image processing or adjustments. No encoding. No metadata interpretation beyond what LibRaw and libheif expose as raw EXIF blobs.
- **metadata**: No pixel manipulation. No encoding. Does not decide what to do with metadata -- it only extracts and represents it.
- **encode**: No decoding. No adjustments. No preset logic. Receives final pixels and metadata, writes output.
- **preset**: No I/O beyond TOML file reading. No pixel math. Does not execute adjustments -- it only declares parameter values.
- **engine**: No direct file I/O for decoding/encoding (delegates to decode/encode modules). Does not define adjustment algorithms (delegates to adjust module for CPU, WGSL shaders for GPU). Pipeline stages are orchestrated in a fixed order; stages are not reorderable by consumers. The CPU executor auto-inserts color-space conversions between stages from each stage's declared color space; the GPU pipeline uses a fused hand-ordered bracket instead (intentional asymmetry: CPU is the pluggable reference, GPU is an optimized mirror). The engine selects GPU or CPU pipeline at runtime — this is transparent to consumers.
- **agx-cli**: No image processing logic. Thin wrapper that parses CLI arguments and calls library API.
- **agx-docgen**: No image processing logic. Dev-only build tool that generates CLI and preset reference markdown for the documentation site.

## Core Invariants

These invariants must hold across the entire codebase. The [architecture explanation](docs/book/src/explanation/concepts/architecture.md) and [design decisions](docs/book/src/explanation/concepts/design-decisions.md) cover *why* each is load-bearing.

1. **Always re-render from original** — the engine holds an immutable original and applies all adjustments from scratch on every render.
2. **Declarative presets** — preset files declare parameter values, not operation sequences.
3. **Working space is linear Rec.2020** — the pipeline operates in linear Rec.2020 for physical-light stages (WhiteBalanceExposure, Dehaze, Denoise) and gamma-encoded Rec.2020 for perceptual stages (PerPixelAdjustments, Detail, Grain, Vignette). LutStage declares its color space from the LUT's `encoding` field (`SrgbGamma` for `Srgb`, `LinearSrgb` for `Linear`). Color-space conversions between stages are **auto-inserted by the CPU executor** from each stage's declared `input_color_space` / `output_color_space`; the GPU pipeline uses a fused hand-ordered equivalent. The single conversion primitive is `crate::color_space::convert_buffer`, which routes hub-and-spoke through linear Rec.2020. Engine output is always linear Rec.2020. Decode converts inputs into linear Rec.2020; encode converts linear Rec.2020 to the chosen output gamut. Input color space is taken from the embedded ICC profile when present (parsed via LittleCMS behind the `icc` feature; see `crates/agx/src/decode/icc.rs`) and assumed sRGB otherwise; a missing or malformed profile falls back to the sRGB assumption. Either way decode lands in linear Rec.2020.
4. **Encoded output self-identifies with its color space** — every JPEG, PNG, and TIFF embeds an ICC profile matching the selected output gamut (`--output-gamut`, default sRGB), chosen from a fixed set (`SRGB_V4_ICC`, `DISPLAY_P3_V4_ICC`, `ADOBE_RGB_V4_ICC` in `crates/agx/src/encode/icc.rs`). Pixel data is encoded into that same gamut via a fixed primary matrix + transfer curve (`color_space::LINEAR_REC2020_TO_LINEAR_*`), so the embedded profile names the color space the pixels actually live in. The embed is unconditional given a gamut and does not depend on input metadata. The default (sRGB) is byte-identical to the prior sRGB-only output. Tested in `encode::tests::encode_{jpeg,png,tiff}_embeds_srgb_v4_icc` and `encode_jpeg_embeds_selected_gamut_icc`. Input labeling is the symmetric concern, handled at decode (invariant #3).
5. **Fixed render order** — the engine applies adjustments in a hardcoded order regardless of preset key order.
6. **Dual pipeline, same output** — CPU and GPU pipelines produce near-identical output; CPU is the canonical path.

## Per-Module Details

Each module has (or will have) a README.md documenting its public API, internal structure, and specific constraints.

| Module     | README                                               |
|------------|------------------------------------------------------|
| adjust     | [`crates/agx/src/adjust/README.md`](crates/agx/src/adjust/README.md)     |
| lut        | [`crates/agx/src/lut/README.md`](crates/agx/src/lut/README.md)           |
| decode     | [`crates/agx/src/decode/README.md`](crates/agx/src/decode/README.md)     |
| metadata   | [`crates/agx/src/metadata/README.md`](crates/agx/src/metadata/README.md) |
| encode     | [`crates/agx/src/encode/README.md`](crates/agx/src/encode/README.md)     |
| preset     | [`crates/agx/src/preset/README.md`](crates/agx/src/preset/README.md)     |
| engine     | [`crates/agx/src/engine/README.md`](crates/agx/src/engine/README.md)     |
| engine/gpu | GPU pipeline via wgpu + WGSL compute shaders (see engine README)      |
| engine/stages | CPU stage implementations (see engine README)                        |
| agx-cli    | [`crates/agx-cli/README.md`](crates/agx-cli/README.md)                   |
| agx-profile-gen | dev-only tool — generates the bundled sRGB v4, Display P3 v4, and Adobe RGB (1998) v4 ICC profiles via lcms2 (see `crates/agx/src/encode/profiles/README.md`) |

## Design Docs

Design docs live in [`docs/plans/`](docs/plans/). Each is a dated `YYYY-MM-DD-<topic>-design.md` capturing the rationale, alternatives, and decisions for a non-trivial change. Browse the directory chronologically or by topic — file names are descriptive enough to navigate without an index.

The backlog of future work — epics, sub-tasks, and bugs — lives in [`docs/backlog/`](docs/backlog/README.md).

## When a Structural Test Fails

The architectural tests in `crates/agx/tests/architecture.rs` enforce the dependency rules above. When a test fails, follow this protocol:

1. **Read the assertion message.** It will tell you exactly which module imported from a forbidden dependency and which line caused the violation.

2. **Check if the import is accidental.** Most failures are unintentional -- a quick refactor pulled in a type from the wrong module, or a new `use` statement crossed a boundary. Fix by moving the type, re-exporting it from the correct module, or restructuring the code.

3. **If the dependency is genuinely needed**, the architecture may need to evolve. Do not simply suppress the test. Instead, follow the process in `docs/contributing/evolving-architecture.md`:
   - Document why the new dependency is necessary in a design doc.
   - Update the dependency rules table in this file and the structural test.
   - Update affected module READMEs.
   - Get the change reviewed — boundary changes affect the entire codebase.

The goal is not to prevent all change, but to make boundary changes visible and intentional rather than accidental.
