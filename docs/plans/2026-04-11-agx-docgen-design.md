# Auto-generated Reference — `agx-docgen` (Sub-project #3)

**Date:** 2026-04-11
**Parent:** [Documentation Initiative](2026-04-06-documentation-initiative-design.md)
**Predecessors:** [Docs Infrastructure (Sub-project #1)](2026-04-06-docs-infrastructure-design.md), [API Doc Retrofit (Sub-project #2)](2026-04-09-api-doc-retrofit-design.md)

## Problem

Sub-projects #1 and #2 stood up the documentation pipeline and documented every `pub` item in the `agx` library crate. The mdbook site and rustdoc are both live and enforced. But two critical reference surfaces still show placeholder pages:

- **CLI reference** (`docs/book/src/reference/cli.md`) — placeholder since sub-project #1. CLI users who visit the site today find "coming in sub-project #3."
- **Preset format reference** (`docs/book/src/reference/preset.md`) — same placeholder. Preset authors have no site-hosted reference for the `.agx` format.

Both surfaces have a source of truth in code — clap `Command` definitions for the CLI, serde struct definitions for presets — but that source of truth is only accessible via rustdoc, which targets Rust library consumers, not CLI users or preset authors.

Without a tool that transforms code definitions into reader-friendly markdown, someone must manually maintain these pages. Manual pages drift from code. This sub-project eliminates that drift.

## Goal

Build `agx-docgen`, a small dev-only binary crate that auto-generates two markdown reference pages from code definitions:

1. **CLI reference** — generated from clap's `Command` tree via `clap-markdown`.
2. **Preset format reference** — generated from serde struct definitions via `schemars` JSON Schema derivation and a custom markdown renderer.

After this sub-project lands:

1. `docs/book/src/reference/cli.md` and `docs/book/src/reference/preset.md` are generated files (gitignored), regenerated on every docs build.
2. `scripts/build-docs.sh` runs `agx-docgen` then `mdbook build` in sequence.
3. `.github/workflows/docs.yml` uses the build script, so the deployed site always reflects the current code.
4. Range constraints on preset fields are documented via `schemars` annotations, with constants shared between the schema annotations and the clamping/validation code. A test cross-checks that schema ranges match the constants, preventing drift.
5. The placeholder pages from sub-project #1 are replaced by real generated content.

## Non-goals

- **No new content writing.** Output quality depends on the `///` doc comments and clap help text written in sub-project #2. If a doc comment reads awkwardly in the rendered table, that is a follow-up, not this sub-project.
- **No mdbook theme changes.** Default table rendering is used. Theme polish is sub-project #9.
- **No external linkcheck workflow.** Generated pages link only to internal site pages (explanation pages). External linkcheck is deferred per the umbrella spec.
- **No preset validation tooling.** `agx-docgen` documents the preset format; it does not validate `.agx` preset files. That is a separate concern.
- **No changes to `verify.sh`.** Docgen correctness is enforced in CI via `docs.yml`, not in the local verify loop. `verify.sh` does not run `mdbook build` (per sub-project #1's decision to avoid requiring local mdbook installation).
- **No changes to existing `///` doc comments.** The retrofit is done; this sub-project consumes those comments as-is.

## Crate structure

New crate at `crates/agx-docgen/`, following the same dev-only pattern as `crates/agx-lut-gen/`:

```
crates/agx-docgen/
├── Cargo.toml
└── src/
    └── main.rs
```

### Dependencies

| Dependency | Purpose |
|---|---|
| `agx` (with `docgen` feature) | Access preset serde types for schema generation |
| `agx-cli` | Access the clap `Command` for CLI reference generation |
| `schemars` | Derive JSON Schema from serde types |
| `clap-markdown` | Render clap `Command` tree as markdown |

`agx-docgen` is not published and is excluded from release builds, same as `agx-lut-gen`.

### What `main.rs` does

Two sequential steps, both invoked unconditionally on every run:

1. **CLI reference.** Call `agx_cli::build_cli()` to obtain the clap `Command`. Pass it to `clap_markdown::help_markdown_command()`. Prepend a "do not edit" header. Write to `docs/book/src/reference/cli.md`.
2. **Preset reference.** Call `schemars::schema_for::<Parameters>()` to obtain the JSON Schema. Walk the schema with a custom renderer that emits grouped markdown tables. Prepend a "do not edit" header. Write to `docs/book/src/reference/preset.md`.

No subcommands, no flags. Fast execution (no I/O beyond two file writes).

## CLI reference generation

### Extracting the clap `Command`

`agx-cli` currently builds its clap `Command` inside `main()`. To make it accessible to `agx-docgen`, extract the command construction into a public function:

```rust
// crates/agx-cli/src/main.rs (or a lib.rs if the crate gains one)

/// Build the top-level clap [`Command`] for the AgX CLI.
pub fn build_cli() -> clap::Command {
    // existing clap builder / derive code moved here
}

fn main() {
    let matches = build_cli().get_matches();
    // ...
}
```

This is a small refactor — one function extraction, no behavior change. `agx-docgen` depends on `agx-cli` and calls `agx_cli::build_cli()`.

### Output format

`clap-markdown` produces markdown with headings per subcommand, flag tables, and usage lines. The output is used as-is with a generated-file header:

```markdown
<!-- Generated by agx-docgen. Do not edit manually. -->
# CLI Reference

[clap-markdown output]
```

If `clap-markdown`'s default formatting needs adjustment (heading levels, anchor slugs, etc.), post-process the string before writing. Start with raw output; polish only if it looks inadequate after the first generation.

The quality of the rendered page depends on the clap doc comments written in sub-project #2, which already describe every command, flag, and argument.

## Preset reference generation

### schemars integration in `agx` crate

`schemars` is added to `crates/agx/` behind an optional feature flag so normal builds do not pay for it:

```toml
# crates/agx/Cargo.toml

[features]
docgen = ["schemars"]

[dependencies]
schemars = { version = "0.8", optional = true }
```

Derive `JsonSchema` conditionally on preset-related types:

```rust
#[cfg_attr(feature = "docgen", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrainParams {
    /// Noise distribution type.
    pub grain_type: GrainType,

    /// Grain intensity (range: 0.0–100.0, default: 0.0).
    #[schemars(range(min = 0.0, max = 100.0))]
    pub amount: f32,

    // ...
}
```

Types that need `#[derive(JsonSchema)]`:
- `Parameters` and `PartialParameters`
- All `*Params` structs: `GrainParams`, `DehazeParams`, `DetailParams`, `NoiseReductionParams`, `ColorGradingParams`, `ToneCurveParams`, `VignetteParams`, `SharpeningParams`
- All preset-related enums: `GrainType`, `VignetteShape`, `ColorSpace`, `HslChannel`, etc.
- `Preset` (if fields beyond `Parameters` are documented in the preset reference)

`agx-docgen` depends on `agx` with the feature enabled:

```toml
# crates/agx-docgen/Cargo.toml

[dependencies]
agx = { path = "../agx", features = ["docgen"] }
```

### Range constants and drift prevention

Numeric range constraints are defined as public constants next to the types they constrain, used in both clamping code and documented via `schemars` annotations:

```rust
/// Minimum grain amount.
pub const GRAIN_AMOUNT_MIN: f32 = 0.0;
/// Maximum grain amount.
pub const GRAIN_AMOUNT_MAX: f32 = 100.0;
/// Default grain amount.
pub const GRAIN_AMOUNT_DEFAULT: f32 = 0.0;

#[cfg_attr(feature = "docgen", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrainParams {
    /// Grain intensity.
    #[schemars(range(min = 0.0, max = 100.0))]
    pub amount: f32,
    // ...
}
```

The `#[schemars(range(...))]` attribute requires literals — it cannot reference `const` values. To prevent drift between the constants (used in clamping code) and the annotation literals (used in schema generation), a test cross-checks them:

```rust
#[cfg(all(test, feature = "docgen"))]
mod schema_tests {
    use super::*;

    #[test]
    fn schema_ranges_match_constants() {
        let schema = schemars::schema_for::<GrainParams>();
        // Walk schema, assert that the "amount" field's
        // minimum == GRAIN_AMOUNT_MIN and maximum == GRAIN_AMOUNT_MAX.
        // Repeat for all constrained fields across all *Params types.
    }
}
```

This test lives in the `agx` crate (gated behind `#[cfg(feature = "docgen")]` so it only compiles when schemars is available). If someone changes a constant but not the annotation, or vice versa, the test fails.

The test is run as part of `cargo test -p agx --features docgen`. `agx-docgen`'s CI build implicitly compiles agx with the docgen feature, so the test runs in CI.

### Preset table rendering

The custom schema walker in `agx-docgen` groups fields to match the on-disk TOML table structure of `.agx` preset files. Rather than grouping solely by `Parameters` struct nesting, the output reflects what preset authors actually see in their files — with sections for `metadata`, `tone`, `white_balance`, and `lut` that don't map 1:1 to `Parameters` fields.

The sections and their sources:

| Output section | Source |
|---|---|
| `metadata` | `Preset` struct fields (name, version, author, extends) — manual rendering |
| `tone` | Flat fields on `Parameters` (exposure, contrast, highlights, shadows, whites, blacks) |
| `white_balance` | Flat fields on `Parameters` (temperature, tint) |
| `lut` | `Preset` struct field (path) — manual rendering |
| `hsl` | `Parameters.hsl: HslChannels` — nested by channel (red, orange, …) |
| `vignette` | `Parameters.vignette: VignetteParams` |
| `color_grading` | `Parameters.color_grading: ColorGradingParams` — nested by wheel (shadows, midtones, highlights, global) |
| `tone_curve` | `Parameters.tone_curve: ToneCurveParams` — nested by curve (rgb, luma, red, green, blue) |
| `detail` | `Parameters.detail: DetailParams` — contains nested `sharpening` sub-table |
| `dehaze` | `Parameters.dehaze: DehazeParams` |
| `noise_reduction` | `Parameters.noise_reduction: NoiseReductionParams` |
| `grain` | `Parameters.grain: GrainParams` |

The original design grouped all flat per-pixel fields (`exposure` through `tint`) into a single "Per-pixel Adjustments" section. During implementation, this was split into `tone` and `white_balance` to match the actual TOML layout. `metadata` and `lut` were added because preset authors need to reference these top-level fields even though they live on `Preset`, not `Parameters`.

Output format (abbreviated):

```markdown
<!-- Generated by agx-docgen. Do not edit manually. -->
# Preset Format Reference

This page documents every field available in an AgX preset TOML file,
organized by on-disk table.

## metadata

Preset metadata, including name, version, author, and optional inheritance.

| Field | Range / Values | Default | Note |
| --- | --- | --- | --- |
| `metadata.name` | string | "" | Human-readable preset name. |
| `metadata.version` | string | "" | Semantic version string for the preset. |

## tone

Controls exposure, contrast, highlights, shadows, whites, and blacks.

| Field | Range / Values | Default | Note |
| --- | --- | --- | --- |
| `tone.exposure` | -5 to 5 | 0 | Exposure in stops, range -5.0 to +5.0. |
| `tone.contrast` | -100 to 100 | 0 | Contrast, range -100 to +100. |

## grain

Controls film grain simulation.

| Field | Range / Values | Default | Note |
| --- | --- | --- | --- |
| `grain.grain_type` | `fine`, `silver`, `harsh` | `silver` | Noise distribution type. |
| `grain.amount` | 0 to 100 | 0 | Grain intensity (range: 0.0–100.0, default: 0.0). |
| `grain.size` | 0 to 100 | 50 | Grain particle size (range: 0.0–100.0, default: 50.0). |

See [Grain](../explanation/grain.md) for a deeper explanation.
```

Table columns:
- **Field** — fully qualified preset field path as it appears in the TOML file (e.g., `tone.exposure`, `hsl.red.hue`)
- **Range / Values** — numeric range from schemars annotations, or enum variant list
- **Default** — from `canonical_default_string()` (see below)
- **Note** — short description, pulled from the `///` doc comment's first sentence

Links to explanation pages are added where the corresponding page exists. The walker checks whether the target explanation page is present in `docs/book/src/explanation/` and only emits the link if so. This avoids broken links for adjustment modules whose explanations have not yet been written (sub-project #4's work).

### Where defaults come from

`schemars` does not reliably auto-discover `Default` impl values, even with `#[schemars(default)]` annotations. The implementation uses a `canonical_default_string()` function in `agx-docgen` that exhaustively matches every field path and derives the default from `Parameters::default()` / `Preset` default values at runtime. This is verified by the `rendered_preset_defaults_match_canonical_runtime_defaults` test, which cross-checks every rendered default cell against the canonical map.

### mdbook output sanitization

Both the CLI and preset reference pages require post-processing for mdbook compatibility:

- **Bracket escaping:** Square brackets `[]` in markdown table cells are interpreted by mdbook as links. The preset renderer escapes them as `&#91;&#93;` (e.g., tone curve point arrays, enum variant lists).
- **CLI title stripping:** `clap-markdown` generates a `# Command-Line Help for \`agx\`` title that conflicts with the page's own title. The sanitizer strips this known title line.
- **Alias rewriting:** `clap-markdown` renders flag aliases as `[alias: \`name\`]`, which mdbook interprets as links. The sanitizer rewrites these to `(alias: \`name\`)`.

## Build pipeline

### Generated files are gitignored

```gitignore
# docs/book/.gitignore
src/reference/cli.md
src/reference/preset.md
```

The placeholder files from sub-project #1 at these paths are deleted and replaced by the gitignore entries. mdbook's `SUMMARY.md` references still point to the same paths — the files are just generated now instead of hand-written.

### Wrapper script: `scripts/build-docs.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

# Generate CLI and preset reference pages from code
cargo run -p agx-docgen

# Build the mdbook site
mdbook build docs/book
```

### `docs.yml` workflow update

Replace the current `mdbook build docs/book` step with `./scripts/build-docs.sh`. Everything else in the workflow is unchanged — rustdoc build, site assembly, and deploy remain as-is.

```yaml
      - name: Build mdbook
        run: mdbook build docs/book
```

becomes:

```yaml
      - name: Generate reference pages and build mdbook
        run: ./scripts/build-docs.sh
```

### Local development

- **Full site build:** `./scripts/build-docs.sh`
- **Live preview:** `cargo run -p agx-docgen && cd docs/book && mdbook serve --open`
- **Editing non-generated pages only:** `cd docs/book && mdbook serve` works fine — generated pages show stale or missing content but nothing breaks

### CI protection

`scripts/build-docs.sh` uses `set -euo pipefail`. If `agx-docgen` exits non-zero (bad schema, broken clap command, etc.), the script fails, the workflow fails, and no deploy happens. No separate drift-check step is needed because the files are never checked in — they are regenerated from scratch on every deploy.

## `agx-cli` refactor

One small change to `crates/agx-cli/`:

Extract the clap command construction into a public function so `agx-docgen` can access it:

```rust
/// Build the top-level clap [`Command`] for the AgX CLI.
pub fn build_cli() -> clap::Command {
    // move existing command builder here
}
```

`main()` calls `build_cli().get_matches()` as before. No behavior change. The crate may need to become a `[[bin]]` + `[lib]` crate (with a `lib.rs` exporting `build_cli` and a `main.rs` calling it), depending on how `agx-cli` is currently structured. Resolved during implementation.

## Workspace `Cargo.toml` update

Add `agx-docgen` to the workspace members list, same as `agx-lut-gen`:

```toml
[workspace]
members = [
    "crates/agx",
    "crates/agx-cli",
    "crates/agx-docgen",  # new
    "crates/agx-e2e",
    "crates/agx-lut-gen",
]
```

## Acceptance criteria

The sub-project is "done" when all of the following hold:

1. `crates/agx-docgen/` exists as a workspace member and compiles.
2. `cargo run -p agx-docgen` generates `docs/book/src/reference/cli.md` and `docs/book/src/reference/preset.md`.
3. The generated CLI reference page renders every subcommand, flag, and argument with help text.
4. The generated preset reference page renders grouped tables with Field, Range/Values, Default, and Note columns for every adjustment category.
5. `docs/book/src/reference/cli.md` and `docs/book/src/reference/preset.md` are gitignored.
6. `scripts/build-docs.sh` exists, runs `agx-docgen` then `mdbook build`, and succeeds.
7. `.github/workflows/docs.yml` uses `scripts/build-docs.sh` instead of bare `mdbook build`.
8. `schemars` is an optional dependency of `agx` behind the `docgen` feature flag.
9. Preset types carry `#[derive(JsonSchema)]` (gated on the `docgen` feature) and `#[schemars(range(...))]` annotations on numeric fields.
10. Public constants for range min/max/default exist alongside the types they constrain.
11. A test gated behind `#[cfg(feature = "docgen")]` cross-checks that schema ranges match the constants.
12. `agx-cli` exports a `pub fn build_cli() -> clap::Command` that `agx-docgen` calls.
13. The sub-project #1 placeholder content in `cli.md` and `preset.md` is replaced by real generated content.
14. `./scripts/verify.sh` still passes (no regressions from the feature flag or the `agx-cli` refactor).
15. The deployed site at `https://zhjngli.github.io/AgX/reference/cli.html` and `.../reference/preset.html` shows real generated content.

## Open questions (resolved)

- **Exact `schemars` version.** Resolved: `schemars = "0.8"` (latest 0.8.x).
- **Exact `clap-markdown` version.** Resolved: `clap-markdown = "0.1"` (latest 0.1.x).
- **`agx-cli` crate structure.** Resolved: `agx-cli` became a `[[bin]]` + `[lib]` crate. All clap type definitions, `impl` blocks, and `build_cli()` moved to `lib.rs`. `main.rs` imports from `lib.rs` and retains only `fn main()`, `run_*` functions, and batch processing.
- **`clap-markdown` output formatting.** Resolved: raw output required post-processing. A `sanitize_markdown()` function strips the auto-generated title and rewrites `[alias: ...]` bracket syntax to `(alias: ...)` for mdbook compatibility.
- **Whether `schemars` auto-discovers `Default` impls.** Resolved: it does not. A `canonical_default_string()` function in `agx-docgen` derives defaults from `Parameters::default()` at runtime, cross-checked by a test.
- **Exact grouping of per-pixel adjustment fields.** Resolved: the flat per-pixel fields were split into `tone` (exposure, contrast, highlights, shadows, whites, blacks) and `white_balance` (temperature, tint) to match the on-disk TOML layout. `metadata` and `lut` sections were added for `Preset`-level fields not on `Parameters`.

## Related

- [Documentation Initiative (umbrella)](2026-04-06-documentation-initiative-design.md)
- [Docs Infrastructure (Sub-project #1)](2026-04-06-docs-infrastructure-design.md)
- [API Doc Retrofit (Sub-project #2)](2026-04-09-api-doc-retrofit-design.md)
- [schemars documentation](https://docs.rs/schemars/)
- [clap-markdown documentation](https://docs.rs/clap-markdown/)
