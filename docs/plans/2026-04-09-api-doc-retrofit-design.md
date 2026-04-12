# API Doc Retrofit (Sub-project #2)

**Date:** 2026-04-09
**Parent:** [Documentation Initiative](2026-04-06-documentation-initiative-design.md)
**Predecessor:** [Docs Infrastructure & Scaffolding (Sub-project #1)](2026-04-06-docs-infrastructure-design.md)

## Problem

Sub-project #1 stood up the documentation pipeline — mdbook, rustdoc, the deploy workflow, and `cargo doc -D warnings` in `verify.sh` — but deferred the source-level `#![warn(missing_docs)]` lint that would put pressure on the API surface to actually be documented. The deferral note in sub-project #1's design doc explains why: adding `warn(missing_docs)` triggered 191 latent warnings on `agx`, and `cargo clippy -- -D warnings` (run by `verify.sh`) unconditionally promoted those warnings to hard errors with no clean suppression. The retrofit work was scoped out into a sub-project of its own, which is this one.

Today the situation is:

- `crates/agx` has 191 `pub` items missing `///` doc comments. Breakdown: 122 struct fields, 20 modules, 16 methods, 14 enum variants, 12 associated functions, 4 structs, 1 enum, 1 type alias, plus 1 missing crate-level `//!` on `crates/agx/build.rs`.
- `crates/agx-cli` has 0 missing-docs warnings. Its public surface is the clap struct that drives subcommands; clap's derive macros use `///` comments as `--help` text, so the CLI was already documented as a side effect of being usable.
- The 191 items are concentrated in a few hot spots: `engine/mod.rs` has 93 (the `Parameters`/`Partial*` types), `adjust/mod.rs` has 36 (the `Adjustment` enum and its variant data), and the long tail spreads across `detail.rs`, `error.rs`, `engine/stages/mod.rs`, `lib.rs`, `preset/mod.rs`, `grain.rs`, and a handful of others.

Until those 191 items are documented and the lint is promoted to source-level `#![deny(missing_docs)]`, every later content sub-project (#3–#8) is working against a moving lint target. This sub-project closes that out.

## Goal

Add `///` doc comments to every `pub` item in the `agx` library crate so the missing-docs warning footprint reaches zero, then promote enforcement from the deferred state to source-level `#![deny(missing_docs)]` on both `agx` and `agx-cli`. After this sub-project lands:

1. `crates/agx/src/lib.rs` and `crates/agx-cli/src/main.rs` both carry `#![deny(missing_docs)]` alongside the existing `#![deny(rustdoc::broken_intra_doc_links)]`.
2. `cargo build`, `cargo clippy`, `cargo doc`, and the IDE's rust-analyzer all reject any new `pub` item without a `///` comment, in the same way and from the same source-level attribute.
3. The clippy interaction that bit sub-project #1 stops being a problem — there are no missing-docs warnings left for clippy to promote.
4. Sub-projects #3–#8 inherit a stable enforcement floor and never need to revisit missing-docs as a concern.

The acceptance bar is "the lint is on, the build is green, and the diff is doc comments only." Content depth is tiered by audience importance (see "Tiering rules" below); the minimum depth is "every item has at least one imperative summary line."

## Non-goals

Explicitly out of scope for this sub-project:

- **No logic changes.** The diff is doc comments, the lint-flip attributes, and the one-line `//!` on `build.rs`. Reviewer can grep for non-comment, non-attribute Rust changes and find none. If the retrofit notices a real bug or rough edge in passing, that becomes a separate follow-up issue, not a sneaky diff hunk.
- **No new doctests.** The "Tier 1" headline-API tier uses prose examples written in normal English, not compiling Rust code blocks. Doctests have a real cost (sample image fixtures, error-path setup) and the umbrella spec assigns no specific sub-project to runnable examples. Adding them here would expand scope past what is needed to flip the lint. A future sub-project can introduce them if that becomes a priority.
- **No movement of content into shared `.md` sibling files.** Sub-project #4 owns that work for the algorithm modules. This sub-project leaves `crates/agx/src/adjust/grain.rs` alone (it already has the worked example from sub-project #1) and gives every other adjust module a short inline `//!` placeholder that sub-project #4 will replace.
- **`agx-e2e` and `agx-lut-gen` are still excluded.** Same rationale as the umbrella spec: test crate / dev tool, no public consumers. Both crates remain free of the missing-docs lint.
- **No reorganization, renaming, or visibility changes to the public surface.** If a `pub` item should arguably be `pub(crate)`, that is a separate refactor. This sub-project documents what is currently public, exactly as it is currently public.
- **No bulk edits to `ARCHITECTURE.md` or root `README.md`.** Sub-project #8 owns those.

## Scope: the 191 items

Measured fresh on 2026-04-09 with `RUSTFLAGS="-W missing-docs -A unused" cargo build -p agx`:

| Item kind | Count |
|---|---:|
| Struct fields | 122 |
| Modules (`pub mod`) | 20 |
| Methods | 16 |
| Enum variants | 14 |
| Associated functions | 12 |
| Structs | 4 |
| Type aliases | 1 |
| Enums | 1 |
| Crate (build script) | 1 |
| **Total** | **191** |

Hot spots by file:

| File | Count |
|---|---:|
| `crates/agx/src/engine/mod.rs` | 93 |
| `crates/agx/src/adjust/mod.rs` | 36 |
| `crates/agx/src/adjust/detail.rs` | 9 |
| `crates/agx/src/error.rs` | 8 |
| `crates/agx/src/engine/stages/mod.rs` | 8 |
| `crates/agx/src/lib.rs` | 7 |
| `crates/agx/src/preset/mod.rs` | 6 |
| `crates/agx/src/adjust/grain.rs` | 6 |
| `crates/agx/src/encode/mod.rs` | 3 |
| `crates/agx/src/adjust/denoise.rs` | 3 |
| `crates/agx/src/engine/stages/color_space_conversion.rs` | 2 |
| Long tail: 9 single-warning files in `crates/agx/src/` (7 of the `engine/stages/*.rs` files, plus `lut/mod.rs` and `adjust/dehaze.rs`) | 9 |
| `crates/agx/build.rs` (crate-level `//!`) | 1 |

`crates/agx-cli` has zero missing-docs warnings today. Adding `#![deny(missing_docs)]` to `crates/agx-cli/src/main.rs` is a single-line change with no retrofit cost.

## Tiering rules

The 191 items are not a uniform surface. The headline API that consumers actually touch deserves substantive prose; the long tail of `Partial*` deserialization plumbing deserves a stamped one-liner. Three tiers, plus a module-doc tier.

### Tier 1 — Headline API (substantive prose)

Items in this tier:

- `Engine`, all of its `pub fn`s including `Engine::new` and `Engine::render`
- `Parameters` and `PartialParameters`
- `Preset` and any top-level preset loading function
- `decode`, `encode`, `EncodeOptions`, `OutputFormat`
- `Lut3D` and its constructors
- `AgxError` and the `Result` type alias
- The `Adjustment` enum itself (not its variants)
- The top-level `*Params` structs: `GrainParams`, `DehazeParams`, `DetailParams`, `NoiseReductionParams`, `ColorGradingParams`, `ToneCurveParams`, `VignetteParams`, `SharpeningParams`. One per adjustment.
- `ImageMetadata`

Format: an imperative summary line, a blank line, then a 2–4 sentence "what / when to use" paragraph. References to algorithm explanation pages on the deployed site via raw HTTPS URL where useful (for example, `GrainParams` links to the grain explanation page on the project site). References to peer items use rustdoc intra-doc syntax (`` [`Engine`] ``) so the `broken_intra_doc_links` lint validates them.

No `# Examples` blocks. No doctests.

### Tier 2 — Fields, variants, and methods on Tier 1 types (one-line)

Items in this tier:

- Struct fields on `Parameters`, on every `*Params`, on `Preset`, on `ImageMetadata`
- Enum variants on `Adjustment`, `GrainType`, `VignetteShape`, `ColorSpace`, `HslChannel`
- Methods and associated functions on Tier 1 types beyond the headline ones (for example, helper constructors)

Format: a single imperative sentence describing what the value or variant means. Where relevant, append unit, range, or default in parens at the end of the sentence:

```rust
/// White balance temperature in Kelvin (range: 2000–10000, default: 5500).
pub temperature: f32,
```

This is the part of the surface that sub-project #3's `agx-docgen` will surface in the published preset reference table, so the docs must exist and be correct, but they do not need extended prose.

### Tier 3 — Mechanical and generated (stamped one-liner)

Items in this tier:

- All `Partial*` types (`PartialColorWheel`, `PartialDehazeParams`, `PartialColorGradingParams`, etc.) and their fields
- Internal stage trait impls in `crates/agx/src/engine/stages/`
- Anything obvious from name and type signature where extended prose would be wasted effort

Format: every `Partial*` struct gets a stamped reference back to its canonical type:

```rust
/// Optional, mergeable form of [`ColorGradingParams`] used during preset deserialization.
///
/// See [`ColorGradingParams`] for field semantics.
pub struct PartialColorGradingParams { /* ... */ }
```

`Partial*` fields get a one-line reference:

```rust
/// See [`ColorGradingParams::shadows`].
pub shadows: Option<PartialColorWheel>,
```

The reader gets a working intra-doc link to the real docs. Sub-project #3's `agx-docgen` uses the canonical types' docs (not the partials') when rendering the preset reference table, so partial-side prose is wasted effort.

### Tier 4 — Modules

Every `pub mod` declared in `crates/agx/src/lib.rs` and in module re-export files gets a `//!` summary at the top of the target file. Sub-modules under `crates/agx/src/engine/stages/` each get a one-line `//!` describing the stage's responsibility.

Special cases for the `crates/agx/src/adjust/` directory:

- `grain.rs` already has its `//!` via `#![doc = include_str!("grain.md")]` from sub-project #1. Leave it alone.
- The other adjust modules (`dehaze.rs`, `denoise.rs`, `detail.rs`, `tone_curve.rs`, `color_grading.rs`, `vignette.rs`, `per_pixel.rs`, plus any others) get a short inline `//!` for now. These are intentionally shallow placeholders — one paragraph each, no math, no academic references. Sub-project #4 replaces them with sibling `.md` files following the grain pattern.

`crates/agx/build.rs` gets a one-line `//!` because rustc emits a missing-docs warning for build scripts that lack a crate-level doc:

```rust
//! Build script for the agx crate. Discovers libraw via pkg-config.
```

The `#![deny(missing_docs)]` attribute is *not* added to `build.rs` (build scripts have no public API surface; the lint is irrelevant there). The one-line `//!` is enough to silence the warning.

## Lint flip mechanics

The final commit on the branch flips the lint:

```rust
// crates/agx/src/lib.rs — top of file
//! AgX — open-source preset-first photo editing library.
//!
//! See the [project site](https://zhjngli.github.io/AgX/) for tutorials,
//! how-to guides, the CLI reference, and the preset format reference.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
```

```rust
// crates/agx-cli/src/main.rs — top of file
//! AgX command-line interface.
//!
//! See the [project site](https://zhjngli.github.io/AgX/reference/cli.html)
//! for the full CLI reference.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
```

`scripts/verify.sh` does not change. The existing `cargo doc --no-deps --workspace` step with `RUSTDOCFLAGS="-D warnings"` already promotes any rustdoc warning (including missing-docs, once the source-level deny is on) to a verify failure. No new check, no new flag combination.

The clippy interaction that forced sub-project #1 to defer the lint stops being a problem after the retrofit. The interaction was: `cargo clippy -- -D warnings` saw 191 missing-docs warnings and turned them into hard errors. After the retrofit, there are zero such warnings, so clippy has nothing to promote and the deny attribute is harmless.

### Intermediate-commit cleanliness

Commits 1 through N-1 add doc comments but do not yet have the source-level deny attribute. Because the deferred state has no source-level missing-docs lint, those intermediate commits are fully `verify.sh`-clean — the script does not warn or fail on undocumented items in the deferred state. Only the final lint-flip commit makes missing-docs items hard errors, and by then there are zero of them.

Useful side effect: the branch never has a broken `verify.sh` state. Every commit can be checked out and built without rebasing or stashing.

For mid-implementation progress measurement (not enforcement), the standalone command

```bash
RUSTFLAGS="-W missing-docs -A unused" cargo build -p agx 2>&1 \
  | grep -c "warning: missing documentation"
```

reports the current warning count and should monotonically decline toward zero.

## Implementation tactic

### One PR, structured commits inside it

The work lands on the existing `docs/api-retrofit` branch as a single PR. Inside the PR, commits are grouped by cohesive area so a reviewer can either review commit-by-commit or read the whole diff. Suggested commit ordering:

1. `docs(agx): document error types and crate-level entries` — `error.rs`, `lib.rs` re-exports, `build.rs` one-line `//!`, the small misc files (`metadata.rs`, `lut/mod.rs`, `decode/mod.rs`, `encode/mod.rs`)
2. `docs(agx): document Engine, Parameters, and the engine module` — the 93-item hot spot in `engine/mod.rs`
3. `docs(agx): document the Adjustment enum and adjust module surface` — the 36-item hot spot in `adjust/mod.rs`
4. `docs(agx): document adjust submodules` — `dehaze.rs`, `denoise.rs`, `detail.rs`, `grain.rs` (the 6 item-level warnings, not the module doc which already exists), `tone_curve.rs`, `color_grading.rs`, `vignette.rs`, `per_pixel.rs`
5. `docs(agx): document preset, encoder, and decoder modules` — `preset/mod.rs`, plus anything left over
6. `docs(agx): self-review pass` — sweep for tier-rule consistency, intra-doc link breakage, accidental code change, prose quality on Tier 1 items
7. `docs(agx, agx-cli): flip warn → deny missing_docs` — the lint-flip commit, which is the smallest commit on the branch (a few attribute lines plus the documentation conventions doc update)

Every commit in this list passes `verify.sh`. Steps 1 through 6 add doc comments only. Step 7 is the load-bearing one.

### Parallel agent implementation

Steps 2–5 are independent of each other (different files, no shared state). They can be dispatched in parallel via `superpowers:dispatching-parallel-agents` during the implementation phase, with each agent owning one cohesive area. Step 6 (self-review) runs after all parallel work is done. Step 7 (lint flip) runs last. The decision to use parallel agents is an executing-plans concern, not a spec concern — recorded here so the implementation plan can pick it up.

### Self-review pass (step 6)

Before the lint flip, run a single self-review pass over the cumulative diff:

1. **Tier-rule consistency.** Spot-check that Tier 1 items have a 2–4 sentence paragraph, Tier 2 items have a one-line sentence with units/ranges where applicable, and Tier 3 items use the stamped reference pattern. Fix any drift.
2. **Intra-doc link correctness.** Run `cargo doc --no-deps -p agx` and verify zero `broken_intra_doc_links` warnings. The `[`Foo`]` syntax is easy to typo.
3. **Accidental code change.** `git diff main...HEAD -- 'crates/agx/**/*.rs'` and grep for any non-comment, non-attribute Rust line. Should be empty.
4. **Prose quality on Tier 1.** Re-read the Tier 1 entries cold. They should describe what the type is, when to use it, and what the consumer cares about — not just restate the type signature.
5. **Doc-test compilation.** `cargo doc --no-deps --workspace` should succeed. We are not adding new doctests, but a stray code fence in a doc comment can still trigger one. If a doctest accidentally appears, either remove the fence or annotate it as `text` so rustdoc doesn't try to compile it.

This step exists because the simplify skill (which would be the natural fit for "review changed code for quality") is meant for code, not doc comments — so this sub-project replaces it with a doc-specific manual review.

## Verification and acceptance

### During implementation, after each commit

- `./scripts/verify.sh` passes (already clean — see "Intermediate-commit cleanliness" above)
- `RUSTFLAGS="-W missing-docs -A unused" cargo build -p agx 2>&1 | grep -c "warning: missing documentation"` shows a monotonically declining count toward 0

### Before the lint-flip commit

- The above grep returns `0` for `agx`
- `agx-cli` is still at `0` (no regression from accidental new pub items)

### After the lint-flip commit

- `./scripts/verify.sh` passes — this is the real gate, with the deny attribute now load-bearing
- `cargo doc --no-deps --workspace` with `RUSTDOCFLAGS="-D warnings"` succeeds
- `./scripts/e2e-quick.sh` passes (smoke check that no incidental code change snuck in)

### Acceptance criteria

The sub-project is "done" when all of the following hold:

1. Every `pub` item in `crates/agx/src/**/*.rs` carries a `///` doc comment.
2. Every `pub mod` declared in any module file has a corresponding `//!` in the target file.
3. `crates/agx/src/lib.rs` carries `#![deny(missing_docs)]`.
4. `crates/agx-cli/src/main.rs` carries `#![deny(missing_docs)]`.
5. `crates/agx/build.rs` carries a one-line `//!`.
6. `./scripts/verify.sh` passes after the lint flip.
7. `./scripts/e2e-quick.sh` passes after the lint flip.
8. The diff contains only doc comments, the lint-flip attributes, and the `build.rs` `//!`. No code changes, no formatting changes outside doc-comment lines.
9. `docs/contributing/documentation-conventions.md` updated: the "Active lints" section now reads "deny(missing_docs) on agx and agx-cli" instead of the deferred-warn language.
10. Sub-project #1's design doc (`2026-04-06-docs-infrastructure-design.md`) gets a one-line "Resolved in sub-project #2" annotation under the "Deferral" subsection. Forward pointer only — does not change content of the deferral note itself.

## Handoff to sub-project #4

Two things sub-project #4's brainstorm should pick up from this spec:

1. **Adjust module `//!` docs added in this sub-project are minimal placeholders.** Every `crates/agx/src/adjust/*.rs` file (except `grain.rs`, which already has the worked example from sub-project #1) gets a one-paragraph inline `//!` describing what the adjustment does. These are intentionally shallow — no math, no references, no algorithm walkthrough. Sub-project #4 replaces them with sibling `.md` files following the `grain.md` pattern.

2. **Per-pixel adjustments may warrant individual algorithm explanations.** The umbrella spec lists "per-pixel adjustments" as a single bullet under sub-project #4. In practice the per-pixel module covers shadows, highlights, contrast, whites, blacks, saturation, vibrance, exposure, white balance, and HSL — each with its own algorithmic character. Sub-project #4's brainstorm should consider whether to ship one combined "per-pixel adjustments" explanation or one explanation per adjustment (or per cohesive group like "tonal range" and "color"). Recommendation from this spec: lean toward separate pages per adjustment, so curious-photo-nerd readers can land directly on `explanation/contrast.html` and find what they want without scrolling through a 12-section monster.

## Open questions

Intentionally minimal — most decisions are pinned.

- **Whether the lint-flip commit also bumps `docs/contributing/documentation-conventions.md`.** Probably yes (it is a small one-paragraph edit and pairs naturally with the lint flip), but the implementer can split it into its own commit if the diff feels cleaner that way.
- **Exact wording of Tier 1 prose for the three or four really important entry points** (`Engine`, `Engine::render`, `Preset`, `decode`). Picked at implementation time. The tier rule defines the *shape* of the prose; the wording is the implementer's call.

## Related

- [Documentation Initiative (umbrella)](2026-04-06-documentation-initiative-design.md)
- [Docs Infrastructure & Scaffolding (Sub-project #1)](2026-04-06-docs-infrastructure-design.md) — see the "Deferral" subsection of "Lint attributes" for the original deferral rationale this sub-project resolves
- [`docs/contributing/documentation-conventions.md`](../contributing/documentation-conventions.md) — gets a one-line update to its "Active lints" section in the lint-flip commit
