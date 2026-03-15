# Rename oxiraw → AgX Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rename the project from "oxiraw" to "agx" across all crate names, imports, error types, FFI bindings, scripts, docs, and tests.

**Architecture:** Mechanical rename in dependency order — directories first, then Cargo.toml, then error types, then bulk source replacement, then FFI, scripts, and docs. Each task is independently verifiable via `cargo check` or `cargo test`.

**Tech Stack:** Rust workspace, clap CLI, C FFI bindings (libraw), cargo, git

---

### Task 1: Create branch and rename directories

**Files:**
- Rename: `crates/oxiraw/` → `crates/agx/`
- Rename: `crates/oxiraw-cli/` → `crates/agx-cli/`

**Step 1: Create feature branch**

```bash
git checkout -b refactor/rename-to-agx
```

**Step 2: Rename crate directories**

```bash
git mv crates/oxiraw crates/agx
git mv crates/oxiraw-cli crates/agx-cli
```

**Step 3: Commit**

```bash
git add -A
git commit -m "refactor: rename crate directories oxiraw → agx"
```

---

### Task 2: Update Cargo.toml files

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/agx/Cargo.toml`
- Modify: `crates/agx-cli/Cargo.toml`

**Step 1: Update workspace root Cargo.toml**

Change workspace members from `crates/oxiraw` → `crates/agx` and `crates/oxiraw-cli` → `crates/agx-cli`.

**Step 2: Update core library Cargo.toml**

Change `name = "oxiraw"` → `name = "agx"`.

**Step 3: Update CLI Cargo.toml**

- Change `name = "oxiraw-cli"` → `name = "agx-cli"`
- Change description to reference "agx" instead of "oxiraw"
- Change dependency `oxiraw = { path = "../oxiraw" ... }` → `agx = { path = "../agx" ... }`

**Step 4: Verify workspace resolves**

Run: `cargo metadata --format-version=1 | head -5`
Expected: No errors, workspace resolves with new names.

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: update Cargo.toml crate names and paths"
```

---

### Task 3: Rename error type and update lib.rs

**Files:**
- Modify: `crates/agx/src/error.rs`
- Modify: `crates/agx/src/lib.rs`

**Step 1: Rename OxirawError → AgxError in error.rs**

Replace all occurrences of `OxirawError` with `AgxError` in `crates/agx/src/error.rs`.

**Step 2: Update lib.rs re-export**

Change `pub use error::{OxirawError, Result}` → `pub use error::{AgxError, Result}`.

**Step 3: Update all references to OxirawError in source files**

Search all `.rs` files for `OxirawError` and replace with `AgxError`. This includes:
- `crates/agx/src/` (error definitions, From impls, doc comments)
- `crates/agx-cli/src/main.rs` (error matching)
- `crates/agx-cli/README.md` (doc references)

**Step 4: Verify**

Run: `cargo check -p agx`
Expected: Compiles clean.

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: rename OxirawError → AgxError"
```

---

### Task 4: Bulk rename oxiraw:: → agx:: in all Rust source

**Files:**
- Modify: All `.rs` files in `crates/agx-cli/src/` (main.rs, batch.rs)
- Modify: `crates/agx-cli/tests/integration.rs`

**Step 1: Replace all `oxiraw::` qualified paths with `agx::`**

In `crates/agx-cli/src/main.rs`:
- `use oxiraw::` → `use agx::`
- All `oxiraw::` qualified paths → `agx::`

In `crates/agx-cli/src/batch.rs`:
- All `oxiraw::` qualified paths → `agx::`

In `crates/agx-cli/tests/integration.rs`:
- `CARGO_BIN_EXE_oxiraw-cli` → `CARGO_BIN_EXE_agx-cli`

**Step 2: Update clap command name**

In main.rs, change `name = "oxiraw"` → `name = "agx"` in the clap derive attribute.

**Step 3: Verify CLI compiles**

Run: `cargo check -p agx-cli`
Expected: Compiles clean.

**Step 4: Run all tests**

Run: `cargo test -p agx && cargo test -p agx-cli`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: rename oxiraw:: imports and paths to agx::"
```

---

### Task 5: Rename FFI C bindings

**Files:**
- Modify: `crates/agx/src/decode/libraw_meta.c`
- Modify: `crates/agx/src/decode/raw.rs`
- Modify: `crates/agx/build.rs`

**Step 1: Rename C functions**

In `libraw_meta.c`, rename all functions from `oxiraw_get_*` to `agx_get_*`:
- `oxiraw_get_make` → `agx_get_make`
- `oxiraw_get_model` → `agx_get_model`
- `oxiraw_get_iso` → `agx_get_iso`
- `oxiraw_get_shutter` → `agx_get_shutter`
- `oxiraw_get_aperture` → `agx_get_aperture`
- `oxiraw_get_focal_len` → `agx_get_focal_len`
- `oxiraw_get_timestamp` → `agx_get_timestamp`
- `oxiraw_get_lens` → `agx_get_lens`
- `oxiraw_get_lens_make` → `agx_get_lens_make`

**Step 2: Update Rust FFI declarations**

In `raw.rs`, update all `extern "C"` function declarations from `oxiraw_get_*` to `agx_get_*`, and all call sites.

**Step 3: Update build.rs**

Change `build.compile("oxiraw_libraw_meta")` → `build.compile("agx_libraw_meta")`.

**Step 4: Verify**

Run: `cargo check -p agx --features raw`
Expected: Compiles clean (if libraw is available; skip if not installed).

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: rename FFI bindings oxiraw_get_* → agx_get_*"
```

---

### Task 6: Update scripts and verify.sh

**Files:**
- Modify: `scripts/verify.sh`

**Step 1: Update verify.sh**

- Change comment "Verification script for oxiraw" → "Verification script for agx"
- Change `cargo clippy -p oxiraw -p oxiraw-cli` → `cargo clippy -p agx -p agx-cli`
- Change `cargo test -p oxiraw` → `cargo test -p agx`
- Change `cargo test -p oxiraw-cli` → `cargo test -p agx-cli`

**Step 2: Verify script works**

Run: `./scripts/verify.sh`
Expected: All 5 checks pass.

**Step 3: Commit**

```bash
git add -A
git commit -m "refactor: update verify.sh for agx crate names"
```

---

### Task 7: Update documentation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify: `ARCHITECTURE.md`
- Modify: `crates/agx/README.md` (if exists)
- Modify: `crates/agx-cli/README.md`
- Modify: `example/README.md`
- Modify: `docs/contributing/developer-workflow.md` (if references oxiraw)

**Step 1: Update README.md**

- Replace `# oxiraw` → `# AgX`
- Replace all `oxiraw` references with `agx` in descriptions and code examples
- Replace `oxiraw-cli` → `agx-cli` in command examples
- Replace CLI command examples: `oxiraw apply` → `agx apply`, `oxiraw edit` → `agx edit`, etc.

**Step 2: Update CLAUDE.md**

- Replace `# Oxiraw` → `# AgX`
- Replace crate name references
- Update test file path references

**Step 3: Update ARCHITECTURE.md**

- Replace `Oxiraw` → `AgX` in title and descriptions
- Replace `oxiraw-cli` → `agx-cli` in module diagram
- Replace `oxiraw` → `agx` in crate references

**Step 4: Update crate-level READMEs**

- `crates/agx-cli/README.md`: Replace all `oxiraw` → `agx` references

**Step 5: Update example/README.md**

- Replace `cargo run -p oxiraw-cli` → `cargo run -p agx-cli`
- Replace other oxiraw references

**Step 6: Verify doc links**

Run: `./scripts/verify.sh` (includes doc link validation)
Expected: All checks pass.

**Step 7: Commit**

```bash
git add -A
git commit -m "docs: update all documentation for agx rename"
```

---

### Task 8: Rename test temp file prefixes (cosmetic cleanup)

**Files:**
- Modify: All test code in `crates/agx/src/` (preset/mod.rs, metadata.rs, encode/mod.rs, lut/mod.rs, decode/mod.rs, engine/mod.rs)
- Modify: `crates/agx-cli/tests/integration.rs`

**Step 1: Bulk replace temp file prefixes**

Replace all `"oxiraw_` prefixes in temp file names with `"agx_` across all test files. These are strings like `"oxiraw_test_preset.toml"` → `"agx_test_preset.toml"`, `"oxiraw_cli_apply_out.png"` → `"agx_cli_apply_out.png"`, etc.

**Step 2: Update test comments**

Replace comment references to `/tmp/oxiraw_test_sample.dng` → `/tmp/agx_test_sample.dng` and `cargo test -p oxiraw` → `cargo test -p agx`.

**Step 3: Run all tests**

Run: `cargo test -p agx && cargo test -p agx-cli`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor: rename test temp file prefixes to agx_"
```

---

### Task 9: Update doc comments and inline references

**Files:**
- Modify: `crates/agx/src/lut/mod.rs` (doc comment line 16)
- Modify: `crates/agx/src/decode/mod.rs` (comments)
- Modify: `crates/agx/src/decode/raw.rs` (doc comment)
- Modify: `crates/agx/tests/architecture.rs` (doc comment)

**Step 1: Search and replace remaining "oxiraw" in doc comments**

Grep all `.rs` files for any remaining "oxiraw" string references in comments and doc strings. Replace with "agx" or "AgX" as appropriate (use "AgX" in prose, "agx" in code references).

**Step 2: Verify no remaining references**

Run: `grep -r "oxiraw" crates/ --include="*.rs" --include="*.c" --include="*.toml"`
Expected: No matches.

Run: `grep -r "oxiraw" scripts/ docs/ *.md`
Expected: No matches (except possibly in plan docs that describe the rename itself).

**Step 3: Commit**

```bash
git add -A
git commit -m "refactor: update remaining doc comments and inline references"
```

---

### Task 10: Final verification and cleanup

**Step 1: Run full verification**

Run: `./scripts/verify.sh`
Expected: All 5 checks pass (format, clippy, library tests, CLI tests, doc links).

**Step 2: Grep for any remaining oxiraw references**

Run: `grep -ri "oxiraw" . --include="*.rs" --include="*.toml" --include="*.md" --include="*.sh" --include="*.c" | grep -v "docs/plans/"`
Expected: No matches outside of plan docs that describe the rename history.

**Step 3: Run cargo fmt**

Run: `cargo fmt --all`
Expected: No changes needed.

**Step 4: Final commit if any fixups needed**

```bash
git add -A
git commit -m "refactor: final cleanup for agx rename"
```
