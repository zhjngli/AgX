# Harness Engineering for Oxiraw

**Date**: 2026-03-04
**Status**: Approved

## Overview

Apply harness engineering principles — inspired by [OpenAI's Codex team](https://openai.com/index/harness-engineering/) — to oxiraw. The goal is to make the codebase more effective for AI-assisted development by encoding architecture, constraints, and maintenance obligations as machine-readable artifacts that agents can navigate and enforce.

## Motivation

As oxiraw grows, AI agents (Claude Code, Codex, etc.) need to work effectively across the codebase without constant hand-holding. The harness engineering approach shifts the human role from writing code to designing environments that enable agents to produce consistent, correct work. This benefits both agent-assisted and human-only development.

**Goals:**
- Faster AI-assisted development with fewer wrong turns
- Ability to scale with parallel agent tasks
- Better codebase quality through mechanical enforcement
- Learning exercise in applying harness engineering to a real project

## Principles Applied

| Harness Engineering Principle | Oxiraw Implementation |
|---|---|
| **A map, not a manual** | Thin `CLAUDE.md` → `ARCHITECTURE.md` → per-module READMEs. Context is distributed and navigable. |
| **What the agent can't see doesn't exist** | Definition of Done in `CLAUDE.md` ensures agents proactively maintain the harness alongside feature work. |
| **Mechanical enforcement over documentation** | `tests/architecture.rs` enforces module layering rules. Fails fast on violations. |
| **Ask what capability is missing** | When structural tests fail, the protocol is: fix the code or surface a layering change — don't suppress. |
| **Give the agent eyes** | Structural test output tells the agent exactly which module and import violated the rule. Evolution process doc tells it what to do next. |

## File Structure

```
oxiraw/
├── CLAUDE.md                              # Entry point — thin map legend
├── ARCHITECTURE.md                        # The map — boundaries, layering, invariants
├── crates/
│   ├── oxiraw/
│   │   ├── tests/
│   │   │   └── architecture.rs            # Structural tests for layering
│   │   └── src/
│   │       ├── metadata.rs                # ImageMetadata type + extraction functions
│   │       ├── engine/README.md           # Engine module contract
│   │       ├── decode/README.md           # Decode module contract
│   │       ├── encode/README.md           # Encode module contract
│   │       ├── adjust/README.md           # Adjustment module contract
│   │       ├── preset/README.md           # Preset module contract
│   │       ├── lut/README.md              # LUT module contract
│   │       └── metadata/README.md         # Metadata module contract
│   └── oxiraw-cli/
│       └── README.md                      # CLI module contract
└── docs/
    ├── plans/                             # Design docs (existing)
    ├── ideas/                             # Future features (existing)
    ├── reference/                         # Technical reference (existing)
    └── contributing/
        └── evolving-architecture.md       # How to change the layering
```

## Module Layering

### Dependency Graph

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

### Dependency Rules

| Module | MUST NOT import from | May import from |
|--------|---------------------|-----------------|
| `adjust` | engine, decode, encode, preset, lut, metadata | (external crates only: `palette`) |
| `lut` | engine, decode, encode, preset, metadata | error |
| `decode` | engine, encode, preset, adjust, lut, metadata | error |
| `metadata` | engine, preset, adjust, lut | error, decode (`is_raw_extension`, `raw::extract_raw_metadata`) |
| `encode` | engine, preset, adjust, lut, decode | error, metadata (`ImageMetadata`) |
| `preset` | decode, encode, metadata | engine (`Parameters`), lut (`Lut3D`), error |
| `engine` | — | adjust, lut, preset, error |
| `oxiraw-cli` | — | oxiraw (library API only) |

These rules are enforced by `crates/oxiraw/tests/architecture.rs`.

### Metadata Module Refactoring

The current codebase has cross-module coupling between `decode` and `encode` for metadata:
- `decode/raw.rs` imports `encode::ImageMetadata` to return extracted metadata
- `encode/mod.rs` imports `decode::is_raw_extension` and `decode::raw::extract_raw_metadata`

This is resolved by introducing a `metadata` module:
- `ImageMetadata` struct moves from `encode` to `metadata`
- All `extract_metadata*` functions move from `encode` to `metadata`
- `decode/raw.rs::extract_raw_metadata` changes its return type from `Option<ImageMetadata>` to `Option<Vec<u8>>` (raw EXIF bytes), eliminating its dependency on any metadata/encode type
- `metadata::extract_metadata` orchestrates all extraction strategies and wraps raw bytes into `ImageMetadata`
- `encode` keeps only injection functions (`inject_metadata`, `inject_metadata_tiff`) and imports `ImageMetadata` from `metadata`
- One-way dependency flow: `decode` ← `metadata` → `encode` (metadata bridges them)

### Negative Constraints

- `adjust` has no concept of images, files, or formats — only `f32` math
- `lut` has no concept of images, engines, presets, or file formats — only LUT data and interpolation
- `preset` has no rendering logic — it's pure serialization/deserialization
- `decode` produces image buffers. Does not modify pixel values, know about metadata types, or import from encode
- `metadata` extracts metadata from source files. Does not modify images or inject metadata into output
- `encode` consumes image buffers and injects metadata. Does not extract metadata or import from decode
- `engine` does not know about file formats — it delegates to decode/encode

## CLAUDE.md Design

The revised `CLAUDE.md` stays thin — an entry point that orients agents and humans, not a comprehensive reference. Key contents:

- Project purpose (1 sentence)
- Workspace layout (list the two crates)
- Conventions (Rust edition, error/serde patterns, test location)
- Pointers to `ARCHITECTURE.md` and `docs/contributing/`
- **Definition of Done** checklist for any feature/design change

### Definition of Done

When implementing a feature, these must ALL be addressed:

1. Implementation code + tests
2. Update `ARCHITECTURE.md` if the change adds modules, changes dependencies, or introduces new invariants
3. Update affected module `README.md` files (public API, extension guide)
4. Verify `tests/architecture.rs` still passes
5. If a new design doc was written, add a cross-link from `ARCHITECTURE.md`

## ARCHITECTURE.md Design

The "map" — concise, structural, focused on boundaries and invariants. Contents:

- Module dependency graph (ASCII diagram)
- Layering rules with explicit constraints (table)
- Negative constraints (what does NOT exist in each module)
- Core invariants (always-re-render-from-original, declarative presets, sRGB-only for now)
- Links to per-module READMEs for detail
- Links to design docs in `docs/plans/`
- Structural test failure protocol

### Structural Test Failure Protocol

When `tests/architecture.rs` fails:

1. Read the failing assertion — it names the module and the forbidden import.
2. Check if the import is accidental — can you restructure the code to avoid the cross-module dependency? (e.g., pass a value as a parameter instead of importing a type). This is the common case. Fix the code.
3. If the dependency is genuinely needed — the current layering may need to evolve. Do NOT suppress or work around the test. Instead: note the needed change in the current design doc or create one, ask the user whether to proceed with a layering change, and if approved, follow the process in `docs/contributing/evolving-architecture.md`.

## Per-Module README Template

Each module README follows a consistent structure:

```markdown
# Module Name

## Purpose
One sentence describing what this module does.

## Public API
Key types and functions exposed to other modules.

## Extension Guide
How to add new capabilities (e.g., "to add a new adjustment...").

## Does NOT
Negative constraints — what this module must never do.

## Key Decisions
Important design choices specific to this module, with rationale.
```

These are intentionally short — a few paragraphs each. Simple modules can have minimal versions. The template ensures consistency when written.

## Structural Test

`tests/architecture.rs` mechanically enforces the layering rules by scanning source files for forbidden `use crate::` imports.

**How it works:**
- For each module, scan its `.rs` files for `use crate::` statements
- Assert that no forbidden cross-module imports exist
- E.g., verify that files in `src/adjust/` never contain `use crate::engine`

**What it catches:** Accidental cross-module dependencies that violate the layering.

**What it doesn't catch:** Deeper transitive issues or runtime coupling. Rust's module system already prevents circular crate dependencies. The structural test catches the most common violations at the module level.

Runs with `cargo test` — no special CI setup needed.

## Architecture Evolution Process

Documented in `docs/contributing/evolving-architecture.md`:

1. **When to change**: A new feature requires a dependency that violates current rules, or the layering no longer reflects reality.
2. **Process**: Write a section in the relevant design doc explaining why. Update `ARCHITECTURE.md` (diagram + rules). Update affected module READMEs. Update the structural test. Implement.
3. **Principle**: Changes should constrain the solution space, not expand it. Adding a new rule is preferred over removing one. If removing a rule, document why the constraint is no longer valid.
4. **Agent guidance**: When an agent encounters a structural test failure, it reads `ARCHITECTURE.md` to understand the rule, then either fixes the code (common case) or surfaces the conflict for human decision.

## Deliverables

1. New `metadata` module — `ImageMetadata` type + all extraction functions, resolving decode↔encode coupling
2. Revised `CLAUDE.md` — thin entry point with Definition of Done checklist
3. `ARCHITECTURE.md` — module dependency graph, layering rules, invariants, negative constraints, structural test failure protocol
4. Per-module READMEs (7 library modules + 1 CLI) — purpose, public API, extension guide, negative constraints, key decisions
5. `crates/oxiraw/tests/architecture.rs` — structural test enforcing module dependency rules
6. `docs/contributing/evolving-architecture.md` — process for changing the layering
