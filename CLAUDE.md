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

## Definition of Done

Every change must satisfy all applicable items before merging:

1. Implementation code + tests
2. Update `ARCHITECTURE.md` if the change adds modules, changes dependencies, or introduces new invariants
3. Update affected module `README.md` files (public API, extension guide)
4. Verify `tests/architecture.rs` still passes
5. If a new design doc was written, add a cross-link from `ARCHITECTURE.md`

## Key Docs

- [`ARCHITECTURE.md`](ARCHITECTURE.md) -- module layering, dependency rules, invariants
- [`docs/plans/`](docs/plans/) -- design and implementation plans
- [`docs/ideas/`](docs/ideas/) -- future feature ideas
- [`docs/contributing/`](docs/contributing/) -- contribution guides (evolving architecture, etc.)
