# Rename oxiraw to AgX

## Summary

Rename the project from "oxiraw" to "AgX" (display name) / `agx` (crate/CLI name).

AgX is the chemical notation for silver halide — the light-sensitive compound in photographic film. The name sits at the intersection of photography, chemistry, and Rust:

- **Ag** = silver (element 47), the metal that makes film photography work
- **X** = generic halide (chloride, bromide, iodide)
- Silver halide is reduced during film development (a redox reaction)
- Silver tarnishes through oxidation — the same process as iron rust
- Written in proper chemical notation: AgX

## Naming Convention

| Context | Name |
|---------|------|
| Display / branding | AgX |
| Core library crate | `agx` |
| CLI crate | `agx-cli` |
| CLI binary | `agx` |
| Workspace directories | `crates/agx/`, `crates/agx-cli/` |
| GitHub repo | User will rename manually via GitHub settings |

## Scope

### In-scope (automated rename)

1. **Cargo.toml files** — workspace members, crate names, dependencies, binary name
2. **Directory structure** — `crates/oxiraw/` → `crates/agx/`, `crates/oxiraw-cli/` → `crates/agx-cli/`
3. **Rust source** — all `use oxiraw::` imports, `oxiraw::` qualified paths, doc comments
4. **CLI output** — binary name, help text, error messages
5. **Documentation** — ARCHITECTURE.md, README, module READMEs, plan docs (title references)
6. **Scripts** — `verify.sh` and any other scripts referencing `oxiraw`
7. **Test files** — integration tests referencing the binary name or crate

### Out-of-scope (manual)

- GitHub repository rename (user handles via GitHub settings)
- crates.io publishing (separate step after rename)

## Verification

`./scripts/verify.sh` must pass after rename (format, clippy, all tests, doc links).
