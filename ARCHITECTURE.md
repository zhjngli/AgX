# Oxiraw Architecture

Read this file before making structural changes to the codebase.

Oxiraw is an open-source photo editing library and CLI in Rust. The architecture follows an always-re-render-from-original model with declarative presets.

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
          └──────┬───────┘
                 │
          ┌──────────────┐
          │  oxiraw-cli  │   (consumer — depends on library only)
          └──────────────┘
```

## Dependency Rules

These rules are enforced by `crates/oxiraw/tests/architecture.rs`.

| Module     | MUST NOT import from                              | May import from                                          |
|------------|---------------------------------------------------|----------------------------------------------------------|
| `adjust`   | engine, decode, encode, preset, lut, metadata     | external crates only (palette)                           |
| `lut`      | engine, decode, encode, preset, metadata           | error                                                    |
| `decode`   | engine, encode, preset, adjust, lut, metadata      | error                                                    |
| `metadata` | engine, preset, adjust, lut, encode                | error, decode (`is_raw_extension`, `raw::extract_raw_metadata`) |
| `encode`   | engine, preset, adjust, lut, decode                | error, metadata (`ImageMetadata`)                        |
| `preset`   | decode, encode, metadata                           | engine (`Parameters`), lut (`Lut3D`), error              |
| `engine`   | no restrictions within library                     | adjust, lut, preset, error                               |
| oxiraw-cli | —                                                  | oxiraw (library API only)                                |

## Negative Constraints

What does NOT exist in each module -- violations of these constraints indicate a design problem.

- **adjust**: No image I/O. No file system access. No knowledge of presets or engine state. Pure pixel math only.
- **lut**: No image decoding/encoding. No preset parsing. Does not apply LUTs to images (that is the engine's job).
- **decode**: No image processing or adjustments. No encoding. No metadata interpretation beyond what LibRaw provides.
- **metadata**: No pixel manipulation. No encoding. Does not decide what to do with metadata -- it only extracts and represents it.
- **encode**: No decoding. No adjustments. No preset logic. Receives final pixels and metadata, writes output.
- **preset**: No I/O beyond TOML file reading. No pixel math. Does not execute adjustments -- it only declares parameter values.
- **engine**: No direct file I/O for decoding/encoding (delegates to decode/encode modules). Does not define adjustment algorithms (delegates to adjust module).
- **oxiraw-cli**: No image processing logic. Thin wrapper that parses CLI arguments and calls library API.

## Core Invariants

These invariants must hold across the entire codebase:

1. **Always re-render from original**: The engine holds an immutable original image and mutable parameter state. Every render applies all adjustments from scratch to the original. This makes the system order-independent from the user's perspective and eliminates accumulated rounding errors.

2. **Declarative presets**: Preset files are TOML documents declaring parameter values, not operation sequences. A preset says "exposure = +1.0", not "apply exposure +1.0 after white balance".

3. **sRGB only**: All internal processing uses the sRGB color space. No color management pipeline, no ICC profile handling, no working space conversion.

4. **Fixed render order**: The engine applies adjustments in a fixed, hardcoded order regardless of the order parameters appear in presets or API calls. The render order is an engine implementation detail, not a user-facing concept.

## Per-Module Details

Each module has (or will have) a README.md documenting its public API, internal structure, and specific constraints.

| Module     | README                                               |
|------------|------------------------------------------------------|
| adjust     | [`crates/oxiraw/src/adjust/README.md`](crates/oxiraw/src/adjust/README.md)     |
| lut        | [`crates/oxiraw/src/lut/README.md`](crates/oxiraw/src/lut/README.md)           |
| decode     | [`crates/oxiraw/src/decode/README.md`](crates/oxiraw/src/decode/README.md)     |
| metadata   | [`crates/oxiraw/src/metadata/README.md`](crates/oxiraw/src/metadata/README.md) |
| encode     | [`crates/oxiraw/src/encode/README.md`](crates/oxiraw/src/encode/README.md)     |
| preset     | [`crates/oxiraw/src/preset/README.md`](crates/oxiraw/src/preset/README.md)     |
| engine     | [`crates/oxiraw/src/engine/README.md`](crates/oxiraw/src/engine/README.md)     |
| oxiraw-cli | [`crates/oxiraw-cli/README.md`](crates/oxiraw-cli/README.md)                   |

## Design Docs

### Plans

| Date       | Document                                                                         |
|------------|----------------------------------------------------------------------------------|
| 2026-02-14 | [Architecture Design](docs/plans/2026-02-14-architecture-design.md)              |
| 2026-02-15 | [MVP Implementation](docs/plans/2026-02-15-mvp-implementation.md)                |
| 2026-02-16 | [LUT Support Design](docs/plans/2026-02-16-lut-support-design.md)                |
| 2026-02-16 | [LUT Implementation](docs/plans/2026-02-16-lut-implementation.md)                |
| 2026-02-16 | [Raw Format Support Design](docs/plans/2026-02-16-raw-format-support-design.md)  |
| 2026-02-16 | [Raw Format Implementation](docs/plans/2026-02-16-raw-format-implementation.md)  |
| 2026-02-17 | [Image Quality & Metadata Design](docs/plans/2026-02-17-image-quality-metadata-design.md)       |
| 2026-02-17 | [Image Quality & Metadata Implementation](docs/plans/2026-02-17-image-quality-metadata-implementation.md) |
| 2026-03-04 | [Harness Engineering Design](docs/plans/2026-03-04-harness-engineering-design.md)                |
| 2026-03-04 | [Harness Engineering Implementation](docs/plans/2026-03-04-harness-engineering-implementation.md)|
| 2026-03-05 | [Developer Loop Design](docs/plans/2026-03-05-developer-loop-design.md)                          |
| 2026-03-05 | [Developer Loop Implementation](docs/plans/2026-03-05-developer-loop-implementation.md)          |

### Ideas

| Document                                              |
|-------------------------------------------------------|
| [Ideas Backlog](docs/ideas/README.md)                  |

## When a Structural Test Fails

The architectural tests in `crates/oxiraw/tests/architecture.rs` enforce the dependency rules above. When a test fails, follow this protocol:

1. **Read the assertion message.** It will tell you exactly which module imported from a forbidden dependency and which line caused the violation.

2. **Check if the import is accidental.** Most failures are unintentional -- a quick refactor pulled in a type from the wrong module, or a new `use` statement crossed a boundary. Fix by moving the type, re-exporting it from the correct module, or restructuring the code.

3. **If the dependency is genuinely needed**, the architecture may need to evolve. Do not simply suppress the test. Instead, follow the process in `docs/contributing/evolving-architecture.md`:
   - Document why the new dependency is necessary in a design doc.
   - Update the dependency rules table in this file and the structural test.
   - Update affected module READMEs.
   - Get the change reviewed — boundary changes affect the entire codebase.

The goal is not to prevent all change, but to make boundary changes visible and intentional rather than accidental.
