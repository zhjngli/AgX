# Documentation Initiative Design

**Date:** 2026-04-06

## Problem

AgX has accumulated significant functionality — editing algorithms, a CLI, a preset format, a render pipeline — but its documentation is scattered, inconsistent, and partially stale:

- `README.md` and `ARCHITECTURE.md` give a high-level overview but don't serve CLI users, preset authors, or contributors who need deeper material.
- Nine module `README.md` files under `crates/*/src/*/` provide partial in-code documentation but vary in depth and style.
- `docs/reference/` contains three reference docs (`color-spaces.md`, `grain-algorithm.md`, `lut-format.md`) that still reference the old project name "oxiraw" and are not integrated with anything else.
- Only two `.rs` files in the `agx` library crate have module-level `//!` doc comments. Most public API items lack `///` doc comments.
- There is no published site. Readers can only discover AgX through the GitHub repo's raw files.
- The existing backlog item `docs/backlog/algorithm-documentation.md` is narrowly scoped to algorithm explanations and does not cover the full documentation surface.

AgX needs a coherent documentation system organized around reader needs, with a single source of truth for each piece of content, a mechanism that forces documentation updates when code changes, and a published site that serves its primary audience without forcing them to read Rust source.

## Goal

Build a documentation system for AgX that:

1. Is organized around the [Diataxis framework](https://diataxis.fr/) — tutorials, how-to guides, reference, and explanation — each quadrant serving a distinct reader need.
2. Serves **CLI users, preset authors, and curious photo nerds** as its primary audience, with comprehensive rustdoc as a secondary surface for library consumers.
3. Keeps source of truth **in code** wherever mechanically possible, so code edits force documentation updates and drift is impossible.
4. Publishes to a **GitHub Pages site** with mdbook as the content generator and rustdoc as the API reference, both deployed in a single CI workflow.
5. Decomposes into **independent sub-projects** that can be brainstormed, specced, planned, and implemented on their own cycles.
6. Keeps documentation content **plain markdown files in the repo** so agents working in the repo (Claude Code, other LLM tools, human developers) can still find content via ordinary search tools.

## Non-goals

The following are explicitly out of scope for this initiative:

- **UI / desktop app documentation** — no UI exists yet.
- **Internationalization / translated docs** — English only.
- **Versioned documentation for multiple AgX releases** — single latest version only; can add later if the project publishes releases.
- **API documentation for private items** — only the public library API is documented.
- **Custom domain** — the site lives at `https://<username>.github.io/AgX/` for now. A custom domain (e.g., `agx.dev`) can be added later without design changes.
- **Anything in the `advanced-research.md` backlog epic** — AI editing, HDR merge, etc. are not part of the documentation initiative.

## Audience

Primary audiences drive the center of gravity of the site:

| Priority | Audience | What they need | Served by |
|----------|----------|----------------|-----------|
| 1 | **CLI users & preset authors** | Install guide, CLI reference, preset reference, tutorials, how-tos | mdbook site |
| 1 | **Curious photo nerds** | Algorithm explanations, photography and color reference | mdbook site (algorithm content shared with rustdoc via sibling `.md` files included from both surfaces) |
| 2 | **Contributors** | Architecture, design decisions, module contracts, in-code `//!` explanations | mdbook site + rustdoc |
| 3 | **Library consumers (other Rust projects)** | Comprehensive API reference | rustdoc |

## Taxonomy — Diataxis mapped to AgX

The Diataxis framework organizes documentation by reader intent. Quadrants are about **what the reader wants**, not about **where the bits live**. Multiple physical locations can serve one quadrant, and one source file can contribute to multiple quadrants.

### Quadrant mapping

| Quadrant | Reader intent | AgX content | Lives in |
|----------|---------------|-------------|----------|
| **Tutorials** | Learning-oriented | Install and edit your first photo; batch-apply a look; stack presets | mdbook only |
| **How-to guides** | Goal-oriented | Create your own preset; extend an existing preset; write a custom LUT; use multi-apply for A/B comparisons | mdbook only |
| **Reference** (library API) | Information-oriented, exhaustive | Types, traits, methods, fields — every public API item | rustdoc (generated from `///` item docs and `//!` module docs) |
| **Reference** (CLI) | Information-oriented, exhaustive | Every command, every flag, usage examples | mdbook (generated at build time from `clap::Command` via `agx-docgen`) |
| **Reference** (preset schema) | Information-oriented, exhaustive | Every preset field, type, valid range, default, description | mdbook (generated at build time from serde types via `agx-docgen`) |
| **Reference** (conceptual) | Information-oriented, exhaustive | Color spaces, LUT format, photographic terminology, preset model | mdbook only (prose, doesn't exist in code) |
| **Explanation** (algorithms) | Understanding-oriented | How grain / dehaze / denoise / detail / color grading / tone curves / vignette work, and why | Sibling `.md` files next to each `crates/agx/src/adjust/*.rs` — canonical source. Pulled into rustdoc via `#![doc = include_str!("module.md")]`; included into mdbook via `{{#include ...}}` |
| **Explanation** (architectural) | Understanding-oriented | Preset-first philosophy, render pipeline overview, module dependency model, design decisions | mdbook only (cross-cutting prose) |

### Physical surfaces

```
                    ┌────────────────────────────────┐
                    │          THE SITE              │
                    │                                │
        mdbook →    │  Tutorials       How-to        │
                    │  (100% md)       (100% md)     │
                    │                                │
                    │  Reference       Explanation   │
                    │  ┌──────────┐    ┌──────────┐  │
                    │  │ CLI ref  │    │ Arch/    │  │
                    │  │ Preset   │    │ philo    │  │
                    │  │ Concept  │    │ (md)     │  │
                    │  │ (md)     │    │          │  │
                    │  └──────────┘    │ Algo     │  │
                    │                  │ explain  │  │
                    │                  │ (incl ←) │  │
                    │                  └──────────┘  │
                    └────────────────────────────────┘
                                              ▲
                                              │
                          ┌───────────────────┴──────┐
                          │  shared .md file beside  │
                          │  the Rust source file    │
                          │  (canonical prose)       │
                          └───────────────────┬──────┘
                                              │
                                              ▼
                    ┌────────────────────────────────┐
                    │          RUSTDOC               │
                    │                                │
        rustdoc →   │  Reference       Explanation   │
                    │  ┌──────────┐    ┌──────────┐  │
                    │  │ API ref  │    │ module   │  │
                    │  │ (auto    │    │ docs     │  │
                    │  │ from     │    │ (incl ←) │  │
                    │  │ ///)     │    │          │  │
                    │  └──────────┘    └──────────┘  │
                    │                                │
                    │  (no tutorials or how-tos)     │
                    └────────────────────────────────┘
```

Both the mdbook explanation page and the rustdoc module page pull from the same shared `.md` file that lives next to the Rust source. Neither surface is the canonical source — the shared file is.

## Content-split strategy

Content lives in the location that minimizes drift and best matches the source of truth:

| Content type | Canonical source | Mechanism |
|--------------|------------------|-----------|
| Public library API | `///` and `//!` doc comments in `crates/agx/src/**/*.rs` | rustdoc renders natively |
| CLI reference | `clap::Command` definitions in `crates/agx-cli/src/*.rs` | `agx-docgen` generates markdown at build time |
| Preset reference | Serde struct definitions in `crates/agx/src/preset/` and `crates/agx/src/engine/` | `agx-docgen` derives a JSON Schema via `schemars`, renders as markdown |
| Algorithm explanations | Sibling `.md` files next to each `crates/agx/src/adjust/*.rs` | rustdoc pulls via `#![doc = include_str!("module.md")]`; mdbook includes via `{{#include ...}}` |
| Tutorials | Markdown files in `docs/book/src/tutorials/` | mdbook renders |
| How-to guides | Markdown files in `docs/book/src/how-to/` | mdbook renders |
| Conceptual reference | Markdown files in `docs/book/src/reference/concepts/` | mdbook renders |
| Architectural explanations | Markdown files in `docs/book/src/explanation/` | mdbook renders |

### The shared-`.md`-file convention

Algorithm explanations live as a sibling markdown file next to the corresponding Rust source. For example, `crates/agx/src/adjust/grain.rs` is paired with `crates/agx/src/adjust/grain.md`. The markdown file contains pure prose with no headers (each surface adds its own outer heading):

```markdown
AgX's grain simulation models film grain by convolving white noise
with a Gaussian kernel whose sigma is proportional to the configured
grain size, then modulating the result by per-pixel luminance ...
```

The Rust file pulls the prose into rustdoc as the module-level doc via `include_str!`:

```rust
#![doc = include_str!("grain.md")]
//!
//! See [`super::dehaze`] for the related haze removal pass.

pub struct GrainParams { /* ... */ }
```

Inline `//!` lines following the `include_str!` attribute are still concatenated into the module doc by rustdoc, so cross-references that need rustdoc-native intra-doc validation (like `[`super::dehaze`]`) live there rather than in the shared `.md` file.

The corresponding mdbook page `docs/book/src/explanation/grain.md` includes the same shared file and adds its own cross-reference block:

```markdown
# Grain

{{#include ../../../../crates/agx/src/adjust/grain.md}}

## Related

- [Noise reduction](denoise.md)
- [Grain API reference](../api/agx/adjust/grain/index.html)
```

Editing `crates/agx/src/adjust/grain.md` updates both rustdoc and the mdbook page on the next build. There is no duplication and no mechanism by which the two surfaces can drift apart.

Cross-surface link rules:

- The shared `.md` file contains no cross-references at all. It is pure prose. This avoids the cross-surface link problem (rustdoc intra-doc syntax does not resolve in mdbook, and mdbook relative links do not always resolve correctly when included in rustdoc output).
- Rustdoc-only cross-references (intra-doc syntax, validated by `broken_intra_doc_links`) live in `//!` lines or `#![doc = "..."]` attributes on the Rust side, outside the include.
- Mdbook-only cross-references live in the wrapping mdbook page, outside the include.

## Tooling architecture

### Components

- **mdbook** (content site) — reads `docs/book/src/**/*.md` and `docs/book/src/SUMMARY.md`, emits static HTML to `docs/book/book/`. Supports `{{#include ...}}` natively for pulling shared `.md` files from anywhere in the repo.
- **rustdoc** (API reference) — `cargo doc --workspace --no-deps`, emits HTML to `target/doc/`.
- **agx-docgen** (new crate, `crates/agx-docgen/`) — a small dev-only Rust binary that:
  - Instantiates the CLI's `clap::Command` and emits `docs/book/src/reference/cli.md` using the `clap-markdown` crate.
  - Derives a JSON Schema from the `Preset` serde types using the `schemars` crate, then renders it as `docs/book/src/reference/preset.md` via a custom walker.
  - Follows the same pattern as the existing `crates/agx-lut-gen/` crate: dev-only, not published.
- **mdbook preprocessors:**
  - `mdbook-linkcheck` — validates all intra-site and outbound links.
  - `mdbook-mermaid` — renders mermaid code blocks for architecture diagrams.
  - `mdbook-katex` — renders LaTeX math in algorithm explanation pages.
- **GitHub Actions workflow** (`.github/workflows/docs.yml`) — on push to `main`, runs `agx-docgen`, builds mdbook, builds rustdoc, merges both artifacts into one tree, and deploys to the `gh-pages` branch. mdbook output lives at the root, rustdoc output lives at `/api/`.

### Publishing layout

```
<username>.github.io/AgX/              ← mdbook root (index = landing page)
<username>.github.io/AgX/tutorials/
<username>.github.io/AgX/how-to/
<username>.github.io/AgX/reference/
<username>.github.io/AgX/explanation/
<username>.github.io/AgX/api/          ← rustdoc root
<username>.github.io/AgX/api/agx/      ← library crate API
<username>.github.io/AgX/api/agx_cli/  ← CLI crate API
```

### Repo layout after sub-project #1

```
docs/
├── book/
│   ├── book.toml               ← mdbook config
│   └── src/
│       ├── SUMMARY.md
│       ├── introduction.md
│       ├── tutorials/
│       │   └── .placeholder
│       ├── how-to/
│       │   └── .placeholder
│       ├── reference/
│       │   ├── cli.md          ← generated by agx-docgen (or placeholder)
│       │   ├── preset.md       ← generated by agx-docgen (or placeholder)
│       │   └── concepts/
│       │       └── .placeholder
│       └── explanation/
│           └── .placeholder
├── backlog/                    ← unchanged
├── contributing/               ← unchanged
├── plans/                      ← unchanged (this doc lives here)
└── reference/                  ← existing files, retired/moved in sub-project #5
```

## Enforcement and CI

Enforcement is strict on everything mechanical from day one. The one expensive check (`deny(missing_docs)`) is staged.

### Day-one (sub-project #1)

- `#![warn(missing_docs)]` on **both `agx` and `agx-cli`** (visible pressure, does not break builds). Excludes `agx-e2e` (test crate, no public API) and `agx-lut-gen` (dev tool, no consumers).
- `#![deny(rustdoc::broken_intra_doc_links)]` on **both `agx` and `agx-cli`** (no retrofit cost, only checks existing links).
- `scripts/verify.sh` runs `cargo doc --no-deps --workspace` with `RUSTDOCFLAGS="-D warnings"` so rustdoc warnings become errors in CI.
- `mdbook-linkcheck` runs on every PR; broken site links fail CI. **Internal links only.** External link checking is deferred until external links actually exist in the book — likely first introduced by sub-project #4 (algorithm explanations referencing papers) or sub-project #5 (conceptual reference refresh). When added, external checks run on a weekly cron and on PRs that touch `docs/book/src/**`, never on every PR, to avoid CI flakiness from rate limits and link rot.
- `agx-docgen` runs as part of the mdbook build. Generated files are either checked in with a CI diff check, OR gitignored and always regenerated. Final decision deferred to sub-project #3.

### End of sub-project #2 (API doc retrofit)

- `#![warn(missing_docs)]` → `#![deny(missing_docs)]` as the final commit of the retrofit branch, on **both `agx` and `agx-cli`**.
- From that point forward, any new `pub` item without a `///` doc comment fails the build in either crate.
- Rationale for documenting `agx-cli`: clap struct doc comments feed both `--help` text AND `agx-docgen`'s rendered CLI reference page. Documenting them is high-value, not redundant with rustdoc.

## Sub-project decomposition

The initiative decomposes into nine sub-projects. Each one gets its own brainstorm → spec → plan → implementation cycle, committed to `docs/plans/` as a dated design doc.

### Ordering

```
#1 infrastructure  →  #2 retrofit  →  { #3 #4 #5 #6 #7 #8 in parallel }  →  #9 polish (optional)
```

Sub-project #1 blocks everything. Sub-project #2 blocks sub-projects #3–#8 directly. Sub-projects #3–#8 can run in parallel on separate branches once #2 lands. Sub-project #9 depends transitively on the content sub-projects being complete — it is optional and deferrable, and only worth tackling if the default mdbook theme feels inadequate after content has shipped.

### Sub-project list

| # | Name | Scope | Blocking |
|---|------|-------|----------|
| 1 | **Docs infrastructure & scaffolding** | mdbook skeleton under `docs/book/`, `SUMMARY.md` with placeholder pages for all four quadrants, GitHub Actions workflow for building and deploying to `gh-pages`, `warn(missing_docs)` + `deny(broken_intra_doc_links)` + `cargo doc` in `verify.sh`, `mdbook-linkcheck` preprocessor, mdbook-mermaid, mdbook-katex, in-code doc conventions doc under `docs/contributing/` | Blocks all |
| 2 | **API doc retrofit** | Walk every `pub` item in **both `agx` and `agx-cli`** and add `///` comments. No logic changes. Final commit flips `warn(missing_docs)` → `deny(missing_docs)` on both crates. Single focused PR | Blocks #3–#8 |
| 3 | **Auto-generated reference (`agx-docgen`)** | New `crates/agx-docgen/` crate. CLI reference via `clap-markdown`. Preset reference via `schemars` + custom renderer. Wired into mdbook build via preprocessor or `just` recipe. Drift check in CI | Independent after #2 |
| 4 | **Algorithm explanations** | Sibling `.md` file next to each `crates/agx/src/adjust/*.rs` (grain, dehaze, denoise, detail, color grading, tone curves, vignette, per-pixel adjustments) containing the canonical prose. `#![doc = include_str!("module.md")]` wiring on the Rust side and `{{#include ...}}` wiring in `docs/book/src/explanation/*.md`. Content can draw on existing design docs under `docs/plans/` (e.g., `2026-03-23-grain-design.md`, `2026-03-21-dehaze-design.md`) as reference material, but the canonical explanation lives in the sibling `.md` file. **If this sub-project introduces external links (paper citations, etc.), it must add the external linkcheck workflow per the Enforcement and CI section** | Independent after #2 |
| 5 | **Conceptual reference refresh** | Move `docs/reference/{color-spaces,grain-algorithm,lut-format}.md` into `docs/book/src/reference/concepts/`. The "oxiraw" → "AgX" rename happens in sub-project #1 as the first commit of its branch. Expand with new topics: photographic terminology, preset compositional model, render pipeline conceptual overview, image processing basics. Consider including a "how AgX generates its bundled LUTs" explainer that draws on the `agx-lut-gen` crate's logic — fits the curious-photo-nerd audience. **If this sub-project introduces external links, it must add the external linkcheck workflow per the Enforcement and CI section** | Independent after #2 |
| 6 | **Tutorials** | Install guide. "Edit your first photo with the CLI." "Apply a look to a whole directory with `batch-apply`." "Compare multiple looks on one image with `multi-apply`." Tutorials reference existing sample images in `example/images/` and existing presets in the e2e test suite | Independent after #2 |
| 7 | **How-to guides** | "Create your own preset from scratch." "Extend an existing preset using the `extends` mechanism." "Write and load a custom `.cube` LUT." "Compose layered looks." "Match the output of another preset." Each guide is task-focused and assumes the reader knows why they're there | Independent after #2 |
| 8 | **Prose explanations** | Architecture overview (pulled and expanded from `ARCHITECTURE.md`). Preset-first philosophy (pulled and expanded from `README.md` Philosophy section). Render pipeline conceptual overview (pulled from the stage-based pipeline design doc). Design decisions and trade-offs. `README.md` and `ARCHITECTURE.md` retain short summary content plus clear pointers into `docs/book/src/`, per the repo discoverability constraint — they do not become empty stubs | Independent after #2 |
| 9 | **Polish / theme** *(optional)* | Custom mdbook theme, logo, landing page styling, OG image. Could also evaluate GitHub's pre-baked Pages starter workflows for a more customized deploy pipeline. Only tackled if default theme/deploy feels inadequate | Optional, deferrable |

## Backlog integration

1. Create `docs/backlog/documentation-initiative.md` as a new **mid-term priority #6** epic, replacing the current `algorithm-documentation.md` row.
2. The epic lists sub-projects 1–9 as checkbox items and links to this umbrella design doc and to the design doc of each sub-project once written.
3. Delete `docs/backlog/algorithm-documentation.md`. Its scope is absorbed by sub-project #4 (algorithm explanations) and sub-project #5 (conceptual reference refresh).
4. Update `docs/backlog/README.md`:
   - Mid-term roadmap table: replace the `algorithm-documentation.md` row with a `documentation-initiative.md` row.
   - By-category tables: move the new file from "Quality and Correctness" to a new "Documentation" subsection (or a new top-level category), since the initiative covers more than quality.

## Repo discoverability constraint

All site content must remain **plain markdown files** living in the repo under `docs/book/src/`, not just deployed HTML. This is a hard constraint, not a preference. Rationale: agents working in the repo (Claude Code, other LLM tools, human developers using editors and grep) need to find documentation content via ordinary file search without a network round trip.

Practical implications:

- Content under `docs/book/src/` is readable via `Read`, `Grep`, `Glob` — no special tooling needed.
- Auto-generated reference files (CLI, preset) should be produced at a path that is searchable. If they are gitignored, they must be regenerated on `cargo build` or before any docs-related tooling runs, so an agent running a verify script will regenerate them before searching. (To be decided in sub-project #3.)
- Root-level files that move content to the site (`ARCHITECTURE.md`, `README.md` philosophy section in sub-project #8) should retain short summary content plus a clear pointer to the canonical location in `docs/book/src/`, not become empty stubs. This ensures an agent opening `README.md` sees a discoverable trail to the detailed content.

## Prep work

Before or during sub-project #1:

- **Rename "oxiraw" → "AgX"** in `docs/reference/{color-spaces,grain-algorithm,lut-format}.md`. Small standalone PR. Does not refactor or move the files — just updates the project name. The larger move into `docs/book/src/reference/concepts/` happens in sub-project #5.

## Open questions

These are deferred to the individual sub-project specs, not decided here:

- **Sub-project #1:** Exact mdbook version, specific `book.toml` configuration, which landing page copy to ship with, whether to use the default mdbook theme or start with a light customization.
- **Sub-project #3:** Whether `agx-docgen` output is checked in and diff-checked, or gitignored and always regenerated. Exact schema used for preset reference tables. How to document range constraints (via custom serde attributes? inline in doc comments?).
- **Sub-project #4:** Whether every adjust module needs equal explanation depth, or whether some get deep dives and others get short overviews. How much math to include. Whether to reference academic papers inline or in a separate bibliography.
- **Sub-project #5:** What new conceptual reference topics to add beyond the existing three.
- **Sub-project #6:** Which specific sample images to feature in the tutorials.
- **Sub-project #7:** Which preset composition patterns to feature.

## Success criteria

The initiative is "done" when:

1. `docs/backlog/documentation-initiative.md` exists with all sub-projects #1–#8 checked off (#9 optional).
2. The GitHub Pages site is live with content in all four Diataxis quadrants.
3. `#![deny(missing_docs)]` is active on the `agx` library crate.
4. `agx-docgen` runs in CI and the generated CLI/preset reference pages are live on the site.
5. Every `crates/agx/src/adjust/*.rs` module has a `//!` explanation block that is surfaced on the mdbook site.
6. `scripts/verify.sh` runs `cargo doc` as a gate and the site's broken-link checker passes.
7. `ARCHITECTURE.md` and `README.md` link to the site for expanded content but retain short summaries for repo-level discoverability.

## Related

- [Diataxis framework](https://diataxis.fr/)
- [mdbook](https://rust-lang.github.io/mdBook/)
- [clap-markdown](https://docs.rs/clap-markdown/)
- [schemars](https://docs.rs/schemars/)
- [`docs/backlog/algorithm-documentation.md`](../backlog/algorithm-documentation.md) — superseded by this initiative
- [`docs/contributing/developer-workflow.md`](../contributing/developer-workflow.md) — will be updated to reference the doc conventions established in sub-project #1
