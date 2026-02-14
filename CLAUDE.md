# Oxiraw

Open-source photo editing library + CLI in Rust with a portable, human-readable preset format.

## Project Structure

Cargo workspace with two crates:
- `crates/oxiraw/` — core library (decode, engine, adjustments, presets, encode)
- `crates/oxiraw-cli/` — thin CLI wrapper

## Architecture

- **Always-re-render-from-original**: Engine holds immutable original image + mutable parameter state. Every render applies all adjustments from scratch. This makes the system order-independent from the user's perspective.
- **Declarative presets**: TOML files declaring parameter values, not operation sequences.
- **Raw decoding**: LibRaw via FFI for raw formats; `image` crate for standard formats (JPEG, PNG, TIFF).

## Key Design Docs

- Architecture design: `docs/plans/2026-02-14-architecture-design.md`
- Future ideas: `docs/ideas/future-features.md`

## Conventions

- Rust 2021 edition
- `thiserror` for error types
- `serde` for all serializable types
- Tests live alongside source in standard Rust `#[cfg(test)]` modules
