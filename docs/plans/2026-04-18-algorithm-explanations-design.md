# Algorithm Explanations Design (Sub-project #4)

**Date:** 2026-04-18

**Parent initiative:** [Documentation Initiative Design](2026-04-06-documentation-initiative-design.md)

**Backlog epic:** [Documentation Initiative](../backlog/documentation-initiative.md)

## Problem

AgX has eleven distinct image-processing algorithms — exposure, white balance, basic tone (contrast/highlights/shadows/whites/blacks), HSL, color grading, tone curves, vignette, grain, dehaze, noise reduction, detail pass — but no consolidated human-readable documentation explaining any of them. Three partial surfaces exist:

- `docs/reference/{color-spaces,grain-algorithm,lut-format}.md` cover color-space concepts and the grain algorithm at a high level, but are scoped to a few topics and slated for restructure in sub-project #5.
- A handful of `docs/plans/` design docs capture the rationale behind individual algorithm implementations (e.g., `2026-03-21-dehaze-design.md`, `2026-03-23-grain-design.md`), but design docs are historical artifacts, not stable reference material.
- A single 8-line `crates/agx/src/adjust/grain.md` was created during sub-project #1 as the first example of the shared-`.md` convention but has not been extended to other algorithms or upgraded to serve as a meaningful explanation.

A reader who wants to understand what the `dehaze.strength` slider actually does, why the basic tone adjustments happen after the gamma encoding but exposure happens before, or how the grain algorithm maps its size parameter to a Gaussian sigma — has nowhere to look but the Rust source.

Sub-project #4 of the Documentation Initiative fills this gap. It delivers per-algorithm explanation pages co-located with the code, surfaced on both the rendered mdbook site and rustdoc, maintained from a single source of truth per algorithm.

## Goal

Deliver algorithm explanations for all per-pixel and buffer-level adjustments in AgX, produced and maintained via the shared-`.md`-file convention established in sub-project #1. Every explanation page is grounded in existing AgX artifacts (design docs, code, commits, PR descriptions) — no fabricated content. Readers get an "applied" treatment: enough theory to understand the code, more attention to AgX-specific constants and preset-slider behavior than to theoretical derivations that are authoritative elsewhere.

Three kinds of work ship together:

1. **Refactor** — extract algorithm-specific submodules from the 1873-line `adjust/mod.rs` so each algorithm has a home for its sibling `.md` file.
2. **Documentation content** — 10 sibling `.md` files, 9 mdbook explanation pages, a contributor-facing GPU dual-path guide, WGSL shader headers.
3. **Infrastructure** — link verification workflows filling the remaining gaps (internal mdbook linkcheck on PRs, external linkcheck as weekly cron + PR soft-check), and CI reorganization grouping doc checks under a `docs-checks` rollup.

## Non-goals

Explicitly deferred to other sub-projects:

- **Color-spaces conceptual reference page.** Deferred to sub-project #5, which moves `docs/reference/color-spaces.md` into `docs/book/src/reference/concepts/` and expands it.
- **Render pipeline conceptual overview.** Deferred to sub-project #5 and #8.
- **CLI and preset reference pages.** Already delivered by sub-project #3.
- **Tutorials and how-to guides.** Deferred to sub-projects #6 and #7.
- **Automated WGSL doc generation** (option C from the shader-docs brainstorm). Deferred to future work — revisit when GPU is re-evaluated for canonical status or when the WGSL surface expands.
- **CI enforcement of the "new algorithm needs sibling `.md`" rule.** Human-discipline via the developer workflow doc for now; mechanical enforcement is future work.

## Audience

- **Curious photo nerds** (initiative's primary audience) — want to understand what each adjustment does and how the sliders feel in practice; don't want a research paper.
- **Contributors** — need to understand constants and trade-offs before modifying algorithms, and dual-path mechanics before adding GPU implementations.
- **Library consumers** — get the same prose via rustdoc module pages when reading the API.

## Refactor: module extraction from `adjust/mod.rs`

Mechanical extraction of seven submodules. No behavior changes, no renames of public functions.

### New submodules (all under `crates/agx/src/adjust/`)

| Module | Contents extracted from `mod.rs` | Approx size |
|--------|----------------------------------|-------------|
| `exposure.rs` | `exposure_factor`, `apply_exposure` | ~15 lines |
| `white_balance.rs` | `apply_white_balance` | ~35 lines |
| `basic_tone.rs` | `apply_contrast`, `apply_highlights`, `apply_shadows`, `apply_whites`, `apply_blacks` | ~65 lines |
| `hsl.rs` | `hue_distance`, `cosine_weight`, `apply_hsl`, `WeightFn` | ~90 lines |
| `color_grading.rs` | `ColorWheel`, `ColorGradingParams`, `ColorGradingPrecomputed`, `apply_color_grading_pre` | ~160 lines |
| `tone_curves.rs` | `ToneCurve`, `ToneCurveParams`, `build_tone_curve_lut`, `lut_lookup`, `ToneCurvePrecomputed`, `apply_tone_curves_pre` | ~220 lines |
| `vignette.rs` | `VignetteShape`, `VignettePrecomputed`, `apply_vignette_pre`, `apply_vignette`, `apply_vignette_buffer` | ~130 lines |

### Unchanged submodules

`grain.rs`, `dehaze.rs`, `denoise.rs`, `detail.rs` keep their current location. Each receives an upgraded `.md` sibling in PR 2.

### What stays in `mod.rs` after extraction

- Re-exports of all public items from submodules (preserves `agx::adjust::apply_contrast` etc. — no external caller edits needed)
- Constants: `LUMA_R`, `LUMA_G`, `LUMA_B`
- Pure helpers: `smoothstep`, `apply_per_channel`, `linear_to_srgb`, `srgb_to_linear`
- Buffer-level orchestrators: `apply_white_balance_exposure_buffer`, `apply_per_pixel_adjustments`, `PerPixelParams`

Estimated post-refactor `mod.rs` size: 250–300 lines.

### Naming rationale

- **`basic_tone.rs`** rather than `tone.rs` to disambiguate from `tone_curves.rs`. "Basic" is Lightroom's own term for this cluster of sliders.
- **`exposure.rs` + `white_balance.rs`** as separate small modules rather than a combined `linear_adjustments.rs` — readable file names matter, and the two operations are conceptually distinct even if they share linear-space semantics.
- **Orchestrators stay in `mod.rs`** following the convention that per-concept math lives in submodules, orchestration glue lives at the module root.

### Rustdoc wiring on each submodule

```rust
#![doc = include_str!("basic_tone.md")]
//!
//! [Optional inline //! lines for rustdoc-intra-doc cross-references
//! like `[`super::tone_curves`]`. These live here rather than in the
//! shared .md to avoid the cross-surface link problem.]

// module contents...
```

### Public API preservation

Every existing `pub use` path continues to resolve. Callers in `engine/`, `preset/`, and `agx-cli` require no edits. Verified by running `scripts/verify.sh` and `scripts/e2e.sh` — if any output byte shifts or any caller fails to compile, the refactor hasn't preserved semantics.

### Architecture test interaction

`crates/agx/tests/architecture.rs` enforces module dependency rules. New submodules are leaves inside `adjust/` with no new outward dependencies, so existing rules hold. If a structural test fails, update the test's ruleset — don't relax the extraction.

## Per-page content template

Every algorithm explanation page follows this template. Section lengths scale with the complexity of the algorithm, but the **Parameters and constants** and **Preset-slider mapping** sections are first-class on every page.

### Template

```markdown
<!-- Canonical source: crates/agx/src/adjust/<module>.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations listed in the Source section below. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

[Intuition paragraph — what the algorithm does in plain terms.]

## How it works

[Enough algorithm description to read our code with understanding.
For well-known algorithms with canonical references (Dark Channel Prior,
à trous wavelets, Fritsch-Carlson), a sketch plus a footnote link. For
AgX-original or simple algorithms, a full procedural description.
LaTeX via katex where formulas clarify.]

## Why we chose it

[Alternatives considered, trade-off that drove the choice. If an AgX
design doc covers this, summarize the decision here — don't just link
to the design doc, because design docs are historical artifacts.]

## Parameters and constants

[Every internal constant explained: value, why that value, what shifts
if tuned.]

| Constant | Value | Role | Sensitivity |
|----------|-------|------|-------------|
| `patch_size` | 15 | Dark-channel sampling window | Smaller = sharper but noisier transmission map |

## Preset-slider mapping

[How user-facing preset fields map into the algorithm's parameters.]

## Source

- **CPU (Rust):** [`crates/agx/src/adjust/<module>.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/<module>.rs)
- **GPU (WGSL):**
  - [`<shader>.wgsl`](...)

The CPU and GPU implementations follow the same math. See the
[GPU architecture guide](...) for dual-path sync mechanics.

## References

[^cite1]: Author et al. (Year). *Title.* Venue. https://...
```

### Template rules

- **Page title (`#`)** lives in the wrapping mdbook file, not the shared `.md`. Each surface adds its own outer heading.
- **No mdbook relative links or rustdoc intra-doc references inside the shared file** (covered below in cross-surface wiring). GFM footnotes to the same-file References section and external `https://` URLs are the exceptions — both parsers render them identically.
- **Section lengths vary.** Vignette's "How it works" might be a paragraph; dehaze's might be four paragraphs with two formulas. That's expected.
- **`## Parameters and constants` and `## Preset-slider mapping` are first-class on every page.** This is the AgX-specific content readers can't find elsewhere, and it's the highest-value part of the doc for the primary audience.

### Special case — the "Basic adjustments" page

One mdbook page wraps three shared `.md` files instead of one, per the Lightroom/Capture One Basic-panel mental model:

```markdown
# Basic adjustments

[Intro explaining the Basic-panel concept and the linear-then-gamma
pipeline ordering — WB + exposure run in linear space before gamma
encoding; the tone sliders run after. A forward-pointer to the Color
Spaces reference page (from sub-project #5) is omitted in this
sub-project.]

## White balance

{{#include ../../../../crates/agx/src/adjust/white_balance.md}}

## Exposure

{{#include ../../../../crates/agx/src/adjust/exposure.md}}

## Tone sliders

{{#include ../../../../crates/agx/src/adjust/basic_tone.md}}

## Related

- [HSL](hsl.md)
- [Tone curves](tone-curves.md)
- API references: [white balance](../api/agx/adjust/white_balance/index.html), [exposure](../api/agx/adjust/exposure/index.html), [basic tone](../api/agx/adjust/basic_tone/index.html)
```

Each of the three shared `.md` files follows the full template. Rustdoc shows `agx::adjust::white_balance`, `agx::adjust::exposure`, and `agx::adjust::basic_tone` as independent module pages. No prose is duplicated — both surfaces pull from the same three files.

### Content sourcing discipline

Every algorithm page is grounded in **existing AgX artifacts**, not invented content. Writers pull from (in priority order):

1. The algorithm's original design doc in `docs/plans/` (e.g., `2026-03-21-dehaze-design.md`, `2026-03-18-tone-curves-design.md`) for "Why we chose it" rationale, constant justifications, and alternatives considered.
2. The code itself (function bodies, constant declarations, inline comments) for "How it works" and "Parameters and constants."
3. Merged PR descriptions on GitHub and commit messages for context on later refinements.
4. The preset-format definitions in `crates/agx/src/preset/` for "Preset-slider mapping."

If an algorithm lacks a prior design doc (some may predate the `docs/plans/` convention), code and commits are the source; no content is invented to fill gaps. If a section would be empty without fabrication, it is omitted and the page notes that specific aspect is undocumented for future backfill. Every algorithm commit in PR 2 cites its source artifacts in the commit message body so a reviewer can verify claims.

## Cross-surface wiring (rustdoc + mdbook)

The shared-`.md`-file convention was established in sub-project #1. This section pins the specific wiring for all 10 shared files in #4.

### Shared-file rules

- Contains prose sections per the template above.
- Contains GFM footnotes for external references, defined in-file in the final `## References` section.
- **No cross-surface relative links.** Shared files must not contain mdbook-style relative links to other `.md` files or rustdoc intra-doc references (the `[\`TypeName\`]` form). Each wrapper on each surface adds its own cross-references after the included content.
- **External URLs are fine** — they parse identically in mdbook (pulldown-cmark) and rustdoc (pulldown-cmark).
- **GFM footnote exemption to the no-cross-references rule:** footnotes (`[^name]` → `[^name]: ...`) reference a target inside the same file. Both surfaces render them identically as numbered superscripts. This is an explicit exemption.
- **Top-of-file HTML comments** (stripped from rendered output on both surfaces) declare the canonical-source pointer and the bidirectional editing rule. Visible to editors, invisible to readers.

### Mdbook wiring — single-shared-file pages (8 of 9)

Example (`docs/book/src/explanation/dehaze.md`):

```markdown
# Dehaze

{{#include ../../../../crates/agx/src/adjust/dehaze.md}}

## Related

- [API reference](../api/agx/adjust/dehaze/index.html)
- [Detail pass](detail.md) — also uses separable filters
- [Noise reduction](denoise.md) — shares frequency-domain concepts
```

The wrapping `# Dehaze` heading, `## Related` section, and forward-pointers live in the wrapping file. The shared `.md` stays parser-agnostic.

### Mdbook wiring — the Basic page

See the "Basic adjustments" example in the per-page template section above.

### `SUMMARY.md` update

Replaces the current placeholder `Explanation → Overview / Grain` with pipeline-ordered chapters:

```markdown
# Explanation

- [Overview](explanation/index.md)
- [Basic adjustments](explanation/basic.md)
- [HSL](explanation/hsl.md)
- [Color grading](explanation/color-grading.md)
- [Tone curves](explanation/tone-curves.md)
- [Vignette](explanation/vignette.md)
- [Grain](explanation/grain.md)
- [Dehaze](explanation/dehaze.md)
- [Noise reduction](explanation/denoise.md)
- [Detail pass](explanation/detail.md)
```

Pipeline order = the order each stage modifies the image during a render. This gives readers an implicit map of the render path.

`explanation/index.md` gets a short update: "Each page explains one algorithm. Pages are listed in pipeline order — the order in which each stage modifies the image."

### Rustdoc verification

`cargo doc --no-deps --workspace` with `RUSTDOCFLAGS="-D warnings"` (already enforced in `scripts/verify.sh`) catches `include_str!` path failures, broken intra-doc links, unresolved `#![doc = ...]` attributes, and missing docs on any public item. Any error is a build failure.

## WGSL shader headers and GPU contributor guide

Two deliverables bridging the dual-path architecture to the documentation.

### Structured WGSL header convention

Every `.wgsl` file in `crates/agx/src/shaders/` (excluding `common/`) gets a header comment block. No tooling or generation — maintained by code-review discipline.

```wgsl
// Algorithm: Dehaze — transmission map estimation
// Canonical explanation: crates/agx/src/adjust/dehaze.md
// CPU equivalent: crates/agx/src/adjust/dehaze.rs (estimate_transmission)
// Bindings:
//   @group(0) @binding(0) var dark_channel: texture_2d<f32>    // input
//   @group(0) @binding(1) var atmospheric:  texture_storage_2d // uniform
//   @group(0) @binding(2) var transmission: texture_storage_2d // output
// Entry points: main
```

- **Algorithm** — one line, format `<Algorithm name> — <specific role>` for algorithms spanning multiple shaders.
- **Canonical explanation** — repo-relative path to the shared `.md` file; grep-target.
- **CPU equivalent** — repo-relative path to the matching Rust function.
- **Bindings** — compact summary of `@group/@binding` bindings with role (input / output / uniform).
- **Entry points** — names of `@compute` entry functions.

**CI enforcement:** `scripts/verify.sh` gets a simple regex check that every non-common `.wgsl` file starts with the four-line header. Common-utility shaders in `shaders/common/` are exempt.

**Rollout:** one commit in PR 2 touches all 25 non-common shader files.

### GPU contributor guide

New file: `crates/agx/src/engine/gpu/README.md`. Contributor-facing, not part of the mdbook site.

**Sections:**

1. **Purpose** — this module hosts the GPU pipeline; CPU path in `adjust/` is canonical for output correctness per `2026-04-13-gpu-acceleration-design.md`. GPU is opt-in via `--gpu` CLI flag.
2. **Dual-path principle** — algorithm math is documented once in the adjust-module sibling `.md` files. CPU and GPU are two implementations of the same math. Cross-path consistency is checked by `gpu_consistency.rs` integration tests.
3. **`GpuParameters` ↔ WGSL `Params` mapping** — table showing the mirroring rule per field group (linear adjustments, gamma adjustments, HSL, color grading, tone curves, vignette, dehaze, denoise, detail, grain) with byte layout notes.
4. **Adding a new adjustment to both paths** — step-by-step checklist (Rust impl → sibling `.md` → `engine::Parameters` field → `GpuParameters` mirror field → WGSL `Params` field → WGSL shader(s) with header → GPU stage → dispatch wiring → consistency test → e2e updates per the `feedback_e2e_with_features` policy).
5. **Debugging the GPU path** — `RUST_LOG=agx::engine::gpu=debug`, buffer inspection pointers, llvmpipe-on-CI gotchas.
6. **Known limitations** — link to the GPU CI gap tracked in `performance.md`. Note GPU is not the default pipeline.

Length: ~200 lines. Grounded in `2026-04-13-gpu-acceleration-design.md`, `params.rs`, shader source, and the GPU acceleration PR (merge commit `e54c668`).

## Link verification

Two gaps in the current link-check coverage, both filled by this sub-project.

### Current state (before #4)

| Surface | Checker | Gate |
|---------|---------|------|
| `.md`→`.md` in `docs/` tree | `verify.sh doc-links` | Every PR (blocking) |
| Rustdoc intra-doc | `cargo doc` + `RUSTDOCFLAGS="-D warnings"` | Every PR (blocking) |
| Mdbook internal links | `mdbook-linkcheck` | Only on push to `main` in `docs.yml` deploy workflow |
| External `https://` links | `follow-web-links = false` in `book.toml`; not checked anywhere | None |

### New: mdbook internal linkcheck on PRs

Add a `book-linkcheck` entry to the CI matrix, running on PRs that touch `docs/book/**`. Uses existing `mdbook-linkcheck` with `follow-web-links = false`. Blocking.

### New: external linkcheck workflow

New file `.github/workflows/external-linkcheck.yml` using [lychee](https://github.com/lycheeverse/lychee) (dedicated link-checker with retry, caching, rate-limiting, exclusion support — not `mdbook-linkcheck`'s `follow-web-links` mode).

**Triggers:**

- **Cron:** Mondays at 09:00 UTC. Catches link rot before it blocks a contributor.
- **On PR** when files matching `docs/**.md`, `crates/**/*.md`, or root-level `*.md` change.

**Policy:**

- Cron runs: `continue-on-error: true`, posts a GitHub Issue when links break (via `peter-evans/create-issue-from-file`).
- PR runs: `continue-on-error: true`, failures as PR annotations. Not hard-blocking — external servers can 500 transiently.

**Config (`.lychee.toml` at repo root):**

```toml
max_retries = 3
retry_wait_time = 2
timeout = 30
max_concurrency = 4
accept = [200, 429]
cache = true
max_cache_age = "1d"

exclude = [
  "arxiv.org",  # intermittent 403s on non-browser user agents
]

exclude_path = [
  "target/",
  "docs/book/book/",
]
```

Exclusion list is populated empirically: if a domain gives consistent false positives, add it with a comment explaining why.

**Local use:** `scripts/verify.sh external-links` (optional target, off by default). Not part of `./scripts/verify.sh all`.

### CI structure reorganization

The `.github/workflows/ci.yml` `fast-checks-matrix` currently bundles all seven fast checks (`fmt`, `clippy`, `test-lib`, `test-cli`, `test-features`, `rustdoc`, `doc-links`) into one rollup. Adding `book-linkcheck` would further bloat that group.

**Restructure:** extract doc-related entries into their own matrix + rollup. No timing change — jobs still run in parallel.

```yaml
docs-matrix:
  strategy:
    matrix:
      check: [rustdoc, doc-links, book-linkcheck]

docs-checks:
  needs: docs-matrix
```

After:

- ✓ Fast checks (fmt, clippy, test-lib, test-cli, test-features)
- ✓ Docs checks (rustdoc, doc-links, book-linkcheck)
- ✓ E2E tests
- ⚪ GPU profiling (soft)
- ⚪ External linkcheck (separate workflow with different triggers)

External linkcheck stays in its own workflow because its trigger pattern (cron + path-filtered) differs from the CI file's PR-trigger model.

### Full link-check matrix after #4

| Surface | Checker | Gate | Notes |
|---------|---------|------|-------|
| `.md`→`.md` in `docs/` | `verify.sh doc-links` | Every PR (blocking) | Existing |
| Rustdoc intra-doc | `cargo doc` + `-D warnings` | Every PR (blocking) | Existing |
| Mdbook internal links | `mdbook-linkcheck` | PRs touching `docs/book/**` (blocking) | **New in #4** |
| External `https://` links | `lychee` | Weekly cron + PRs touching `**/*.md` (soft) | **New in #4** |

## PR plan

Two PRs total. The first is pure refactor with placeholder docs; the second layers in content, infrastructure, and enforcement.

### PR 1: `refactor(adjust): extract submodules from mod.rs`

Goal: pure mechanical refactor. Reviewer focuses on code-movement correctness.

**Commits:**

1. `refactor(adjust): extract exposure submodule` — new `exposure.rs` + placeholder `exposure.md` + `#![doc = include_str!]` wiring + `mod.rs` re-export + test migration.
2. `refactor(adjust): extract white_balance submodule`
3. `refactor(adjust): extract basic_tone submodule`
4. `refactor(adjust): extract hsl submodule`
5. `refactor(adjust): extract color_grading submodule`
6. `refactor(adjust): extract tone_curves submodule`
7. `refactor(adjust): extract vignette submodule`
8. `docs(agx): placeholder md files for existing dehaze/denoise/detail/grain` — adds sibling `.md` + `include_str!` wiring for the four existing submodules. `grain.md` is overwritten from its current 8-line content to the placeholder format (real upgrade lands in PR 2).
9. `docs(agx): mdbook explanation page stubs` — 9 stub pages under `docs/book/src/explanation/` with `{{#include}}` wiring, `SUMMARY.md` updated, `explanation/index.md` updated.

**Placeholder `.md` content:** one paragraph explaining what the algorithm does at a single-sentence level, plus the top-of-file HTML comment declaring the canonical-source pointer. No Parameters/Constants/References sections yet. Satisfies `deny(missing_docs)` and mdbook-linkcheck.

**Verification before submitting PR 1:**

- `scripts/verify.sh all` passes
- `scripts/e2e-quick.sh` passes
- `cargo doc --no-deps --workspace` builds without warnings
- `cargo run -p agx-docgen && mdbook build docs/book` succeeds
- **`/simplify` pass on the full refactor diff** — catches redundant imports, orphaned helpers, copy-pasted test boilerplate. Re-run `verify.sh` + `e2e-quick.sh` post-simplify.

**Diff size estimate:** ~1500 lines added, ~1400 lines deleted. Net small, mostly moves.

### PR 2: `docs(agx): algorithm explanations (sub-project 4)`

Goal: replace placeholder prose with real algorithm explanations; add WGSL headers, GPU contributor guide, linkcheck infrastructure.

**Algorithm content commits (one per algorithm; reviewers can check out a single commit to review one algorithm in isolation):**

1. `docs(agx): write grain algorithm explanation` — sources: `2026-03-23-grain-design.md`, `2026-03-27-grain-size-fix-design.md`, `2026-03-29-chromatic-grain-design.md`, code in `grain.rs`.
2. `docs(agx): write dehaze algorithm explanation` — sources: `2026-03-21-dehaze-design.md`, `2026-04-05-dehaze-parallelization-design.md`, code in `dehaze.rs`, paper refs for Dark Channel Prior and Guided Filter.
3. `docs(agx): write denoise algorithm explanation` — sources: `2026-03-22-noise-reduction-design.md`, code in `denoise.rs`, paper refs for à trous wavelets.
4. `docs(agx): write detail pass algorithm explanation` — sources: `2026-03-21-detail-pass-design.md`, code in `detail.rs`.
5. `docs(agx): write vignette algorithm explanation` — sources: `2026-03-18-vignette-design.md`, code in `vignette.rs`.
6. `docs(agx): write tone curves algorithm explanation` — sources: `2026-03-18-tone-curves-design.md`, code in `tone_curves.rs`, paper ref for Fritsch-Carlson.
7. `docs(agx): write color grading algorithm explanation` — sources: `2026-03-18-color-grading-design.md`, code in `color_grading.rs`.
8. `docs(agx): write HSL algorithm explanation` — sources: `2026-03-05-hsl-adjustments-design.md`, code in `hsl.rs`.
9. `docs(agx): write basic adjustments explanation` — three shared `.md` files (`white_balance`, `exposure`, `basic_tone`) plus the wrapping `basic.md` page's intro teaching the linear-vs-gamma distinction.

**Infrastructure commits:**

10. `docs(agx): add structured headers to all WGSL shaders` — 25 shader files, one commit.
11. `docs(agx): GPU contributor guide (engine/gpu/README.md)`
12. `ci(docs): add book-linkcheck matrix entry and docs rollup` — restructures `.github/workflows/ci.yml` per the CI reorganization section.
13. `ci(docs): external linkcheck workflow and lychee config` — new workflow file, `.lychee.toml`, optional `verify.sh external-links` target.
14. `docs(contributing): sibling-md rule and algorithm checklist` — updates `docs/contributing/documentation-conventions.md` with the bidirectional editing rule; updates `docs/contributing/developer-workflow.md` adding "When adding a new `adjust` submodule or other algorithm-bearing module, create a sibling `.md` with the documentation-conventions template, wire it with `#![doc = include_str!]` + `{{#include}}`, and add a `docs/book/src/explanation/` entry with `SUMMARY.md` listing." Cross-references the WGSL header convention.
15. `docs(backlog): check off documentation-initiative sub-project 4` — updates `docs/backlog/documentation-initiative.md` and the "Open questions" section of the umbrella initiative design doc.

**Verification before submitting PR 2:**

- `scripts/verify.sh all` passes
- `scripts/e2e.sh` passes
- Manual site preview: `./scripts/build-docs.sh && open docs/book/book/html/index.html` — walk all 9 explanation pages
- `cargo doc --open` — spot-check 3 rustdoc module pages
- Spot-check WGSL headers on 3 random shader files
- Local `lychee` run against the PR
- **`/simplify` pass on the non-prose commits** (WGSL header commit for formatting consistency; CI workflow commits for YAML consolidation). Skip on the 9 algorithm prose commits.

**Diff size estimate:** ~2500 lines added. Reviews in per-file chunks.

### Ordering between PR 1 and PR 2

PR 1 must merge before PR 2 branches off. PR 2 can be in-progress locally during PR 1 review.

**Time estimate:** PR 1 is one focused session (~half a day). PR 2 spreads across sessions — one algorithm per session is a reasonable cadence, so ~1–2 weeks depending on review depth.

## Testing and verification

### Refactor preserves behavior

- **Unit tests** in each current section of `mod.rs` move to the new submodule alongside the code being tested. `cargo test -p agx` passes = every extracted function still works.
- **Architecture tests** (`crates/agx/tests/architecture.rs`) verify module dependency rules hold (new submodules are leaves inside `adjust/` with no new outward deps).
- **E2E golden tests** (`agx-e2e` crate) byte-compare rendered output to committed PNGs. Refactor must not shift a single output byte. Run `scripts/e2e-quick.sh` locally during PR 1, `scripts/e2e.sh` in CI.
- **Public API preservation:** diff `cargo doc --no-deps --workspace` output against `main` for any path change. Every `agx::adjust::*` path that existed before must still resolve.
- **Clippy suppressions** attached to items that move travel with them.

### Documentation rendering correctness

- **Rustdoc:** `cargo doc --no-deps --workspace` with `-D warnings` catches `include_str!` path failures, broken intra-doc links, missing docs, duplicate doc attributes.
- **Mdbook:** `mdbook build docs/book` with all preprocessors catches `{{#include}}` path failures, broken internal chapter links, malformed LaTeX, broken anchor fragments.
- **Cross-surface consistency:** structurally impossible to diverge — both surfaces pull from the same shared files. No automated check needed; the link checkers catch wrapper-path errors.
- **Manual eyeball verification** (part of PR 2): build both surfaces, open each page on each, confirm prose, formulas, and footnotes render. Logged as a checklist item in the PR description.

### Content-claim verification

Every algorithm commit in PR 2 cites its source artifacts in the commit body. Reviewers verify claims against the cited sources. No automated check — this is the human-in-the-loop part.

**Anti-pattern to watch for:** writing a plausible-sounding explanation that isn't grounded in the code. Content-sourcing discipline (see template section) forbids this. Empty-without-fabrication sections are omitted, not filled.

### What's deliberately not tested

- **Prose quality** — review-based, not automated.
- **Cross-algorithm consistency of explanations** — caught by review if `dehaze.md` contradicts `denoise.md`.
- **External paper URL stability over time** — handled by the weekly cron's alerts, not by pre-merge blocking.
- **Visual regression on the rendered site** — no screenshot-diff; manual eyeball check suffices for this one-time content drop.

### `scripts/verify.sh` updates

One script change: add the new `external-links` optional target (off by default). The existing `check_doc_links` function's scan list stays unchanged — sibling `.md` files in `crates/agx/src/adjust/` are deliberately *not* added to its scope because they contain only prose (no relative `.md` links per the shared-file convention) and are covered by `mdbook-linkcheck` in their rendered form.

## Open questions (for PR-time resolution)

Smaller decisions deferred to implementation rather than blocking this spec:

- **Exact preprocessor versions for the `book-linkcheck` CI matrix entry.** Should match versions pinned in `docs.yml` (`mdbook 0.4.40`, `mdbook-linkcheck 0.7.7`, `mdbook-mermaid 0.14.0`, `mdbook-katex 0.9.0`), reconfirmed at implementation time.
- **Lychee exclusion list.** `.lychee.toml` draft lists `arxiv.org` as a suggested exclusion. Real list is populated empirically — add domains with consistent false positives, each with a comment explaining why.
- **Forward-reference backfill mechanism.** Section 1 omits forward-refs to Color Spaces (sub-project #5) and Render Pipeline (sub-project #5 and #8). The PR authors of those sub-projects are responsible for backfilling links into #4's pages. A `BACKFILL.md` tracking file is an option but probably over-engineering — direct edits to #4's pages during those sub-projects suffice.
- **Whether the "Basic adjustments" page keeps its linear/gamma teaching paragraph after #5 lands.** Current plan: inline teaching in #4, superseded by a pointer to #5's Color Spaces page when #5 merges. Cheap to adjust.

## Future work

Deliberately out of scope, filed for later:

- **Automated WGSL doc generation** (option C from the shader-docs brainstorm). Extending `agx-docgen` to parse WGSL via `naga` or similar. Revisit when GPU is re-evaluated for canonical status (backlog `performance.md` P8) or when the WGSL surface expands enough that structured headers feel thin.
- **"Soft checks" umbrella workflow.** If more soft CI checks accumulate beyond external-linkcheck + GPU profiling, consider consolidation. Two soft checks isn't enough today.
- **Visual regression testing on the rendered site.** Screenshot-diff for mdbook pages. Not valuable until the site has meaningful theming or interactive components.
- **"Algorithm playground"** — interactive parameter tweaking on rendered pages. Needs WASM builds of AgX and significant JS. Outside docs scope.
- **Auto-generated source cross-references.** Custom mdbook preprocessor generating Source section links from `{{#source ...}}` directives. Low value for 10 pages.
- **CI enforcement of the "new algorithm needs sibling `.md`" rule.** Mechanical check (e.g., `scripts/verify.sh` test that every `pub mod` in `adjust/mod.rs` has a corresponding `<name>.md` sibling). Human discipline via the developer workflow doc is the enforcement mechanism for now.

## Success criteria

Sub-project #4 is done when:

1. Both PRs are merged to `main`.
2. Every adjust submodule (`exposure`, `white_balance`, `basic_tone`, `hsl`, `color_grading`, `tone_curves`, `vignette`, `grain`, `dehaze`, `denoise`, `detail`) has a sibling `.md` file with the full per-page template content — not placeholder.
3. All 11 adjust modules render correctly on both surfaces: rustdoc (individual module pages) and mdbook (9 explanation pages, pipeline-ordered `SUMMARY.md`, basic-adjustments bundling WB + exposure + basic-tone).
4. Every WGSL shader under `crates/agx/src/shaders/` (excluding `common/`) has the structured header from the WGSL section. `scripts/verify.sh` enforces this.
5. `crates/agx/src/engine/gpu/README.md` exists with the contributor guide.
6. `.github/workflows/external-linkcheck.yml` and `.lychee.toml` exist; weekly cron is running; on-PR path-filtered runs are configured.
7. `.github/workflows/ci.yml` has the reorganized `docs-matrix` + `docs-checks` rollup; `book-linkcheck` is blocking on PRs touching `docs/book/**`.
8. `docs/contributing/documentation-conventions.md` documents the shared-`.md` convention, bidirectional editing rule, footnote reference style, and forward-ref-omission rule.
9. `docs/contributing/developer-workflow.md` documents the "new algorithm needs sibling `.md`" checklist item.
10. `docs/backlog/documentation-initiative.md` sub-project #4 is checked off; the "Open questions" section of the umbrella initiative design doc is updated.
11. `scripts/verify.sh all` and `scripts/e2e.sh` pass. Manual walkthrough of all 9 explanation pages confirms they render cleanly in mdbook and rustdoc.

### Non-success criteria (deliberately not gating)

- External link health on merge day — link rot is continuous; soft-gating via lychee is appropriate.
- Exhaustive academic citation coverage — cite the canonical source for the algorithm and important derivations; not every related paper.
- Uniform page length — pages vary with algorithm complexity by design.

### Unblocks for downstream sub-projects

- **#5 (Conceptual reference refresh)** — backfills forward-refs in #4's pages (Color Spaces, linear vs gamma) once its pages ship. Can start in parallel after PR 1 of #4 lands (module layout is established then).
- **#6, #7 (Tutorials, How-to guides)** — will cite the explanation pages. Can start in parallel after PR 2 lands.
- **#8 (Prose explanations)** — will add Render Pipeline links pointing to each stage's algorithm explanation. Same timing as #6 / #7.

## Related

- [Documentation Initiative Design](2026-04-06-documentation-initiative-design.md) — umbrella design doc
- [Documentation Initiative backlog epic](../backlog/documentation-initiative.md) — tracks sub-project status
- [Docs infrastructure design (sub-project #1)](2026-04-06-docs-infrastructure-design.md) — establishes the shared-`.md` convention
- [API doc retrofit design (sub-project #2)](2026-04-09-api-doc-retrofit-design.md) — enables `deny(missing_docs)`
- [agx-docgen design (sub-project #3)](2026-04-11-agx-docgen-design.md) — auto-generated CLI and preset reference
- [GPU acceleration design](2026-04-13-gpu-acceleration-design.md) — dual-path architecture rationale, referenced by the GPU contributor guide
- Per-algorithm design docs (content sources for PR 2 prose): [grain](2026-03-23-grain-design.md), [grain size fix](2026-03-27-grain-size-fix-design.md), [chromatic grain](2026-03-29-chromatic-grain-design.md), [dehaze](2026-03-21-dehaze-design.md), [dehaze parallelization](2026-04-05-dehaze-parallelization-design.md), [noise reduction](2026-03-22-noise-reduction-design.md), [detail pass](2026-03-21-detail-pass-design.md), [vignette](2026-03-18-vignette-design.md), [tone curves](2026-03-18-tone-curves-design.md), [color grading](2026-03-18-color-grading-design.md), [HSL adjustments](2026-03-05-hsl-adjustments-design.md)
