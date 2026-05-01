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
| `metadata` | engine, preset, adjust, lut, encode                | error, decode (`is_raw_extension`, `raw::extract_raw_metadata`) |
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
- **decode**: No image processing or adjustments. No encoding. No metadata interpretation beyond what LibRaw provides.
- **metadata**: No pixel manipulation. No encoding. Does not decide what to do with metadata -- it only extracts and represents it.
- **encode**: No decoding. No adjustments. No preset logic. Receives final pixels and metadata, writes output.
- **preset**: No I/O beyond TOML file reading. No pixel math. Does not execute adjustments -- it only declares parameter values.
- **engine**: No direct file I/O for decoding/encoding (delegates to decode/encode modules). Does not define adjustment algorithms (delegates to adjust module for CPU, WGSL shaders for GPU). Pipeline stages are orchestrated in a fixed order; stages are not reorderable by consumers. The engine selects GPU or CPU pipeline at runtime — this is transparent to consumers.
- **agx-cli**: No image processing logic. Thin wrapper that parses CLI arguments and calls library API.
- **agx-docgen**: No image processing logic. Dev-only build tool that generates CLI and preset reference markdown for the documentation site.

## Core Invariants

These invariants must hold across the entire codebase. The [architecture explanation](docs/book/src/explanation/concepts/architecture.md) and [design decisions](docs/book/src/explanation/concepts/design-decisions.md) cover *why* each is load-bearing.

1. **Always re-render from original** — the engine holds an immutable original and applies all adjustments from scratch on every render.
2. **Declarative presets** — preset files declare parameter values, not operation sequences.
3. **sRGB only** — no working-space conversion, no ICC profile handling.
4. **Fixed render order** — the engine applies adjustments in a hardcoded order regardless of preset key order.
5. **Dual pipeline, same output** — CPU and GPU pipelines produce near-identical output; CPU is the canonical path.

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
