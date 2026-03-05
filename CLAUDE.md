# Oxiraw

Open-source photo editing library + CLI in Rust with a portable, human-readable preset format.

## Workspace Layout

Cargo workspace with two crates:
- `crates/oxiraw/` -- core library (decode, engine, adjustments, presets, encode)
- `crates/oxiraw-cli/` -- thin CLI wrapper

## Architecture

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for module dependency graph, dependency rules, core invariants, and per-module READMEs.

## Conventions

- Rust 2021 edition
- `thiserror` for error types
- `serde` for all serializable types
- Tests live alongside source in standard Rust `#[cfg(test)]` modules
- Structural tests in `crates/oxiraw/tests/architecture.rs` enforce module dependency rules; see "When a Structural Test Fails" in `ARCHITECTURE.md`

## Developer Workflow

Follow this cycle for features and significant changes. See [`docs/contributing/developer-workflow.md`](docs/contributing/developer-workflow.md) for detailed guidance.

### 1. Design (if needed)
Write a design doc in `docs/plans/` when the change adds/modifies modules, changes dependencies, or touches 3+ files. Skip for bug fixes, single-file changes, and doc updates.

### 2. Implement
Work on a feature branch (`feat/`, `fix/`, `refactor/`). Write tests alongside code. Follow module contracts in per-module READMEs. Commit incrementally.

### 3. Verify
Run `./scripts/verify.sh`. This runs format check, clippy, all tests (unit + architecture + CLI), and doc link validation.

### 4. Document
Update `ARCHITECTURE.md` if modules, dependencies, or invariants changed. Update affected module READMEs (public API, extension guide). Cross-link any new design docs.

### 5. Self-review
Re-read the diff. Check: Did I implement what was asked? Did I add anything extra? Do tests verify behavior?

## Definition of Done

Before merging, verify:
1. `./scripts/verify.sh` passes
2. `ARCHITECTURE.md` updated if modules, dependencies, or invariants changed
3. Affected module `README.md` files updated
4. Design doc cross-linked from `ARCHITECTURE.md` (if applicable)

## Key Docs

- [`ARCHITECTURE.md`](ARCHITECTURE.md) -- module layering, dependency rules, invariants
- [`docs/plans/`](docs/plans/) -- design and implementation plans
- [`docs/ideas/`](docs/ideas/) -- future feature ideas
- [`docs/contributing/`](docs/contributing/) -- developer workflow, evolving architecture
