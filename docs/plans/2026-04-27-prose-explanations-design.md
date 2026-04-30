# Prose Explanations (Sub-project #8) Design

**Date:** 2026-04-27

**Parent initiative:** [Documentation Initiative](2026-04-06-documentation-initiative-design.md)

**Backlog tracker:** [`docs/backlog/documentation-initiative.md`](../backlog/documentation-initiative.md), sub-project #8.

## Problem

Sub-project #8 is the last content sub-project of the Documentation Initiative. The umbrella design assigned it four content slots — architecture overview, preset-first philosophy, render pipeline conceptual overview, design decisions and trade-offs — under the assumption that none of that prose existed yet on the site.

Two things have changed since the umbrella design was written:

1. Sub-project #5 (conceptual reference refresh) shipped `reference/concepts/render-pipeline.md`, `preset-model.md`, and a refreshed `color-spaces.md`. Each of those pages contains substantial "why" prose alongside lookup material — `render-pipeline.md` has a "Why pipeline order matters" section, `preset-model.md` has "Why partial parameters" and "Mental model" sections, `color-spaces.md` has "Why exposure and white balance are in linear space," "Why tone adjustments are in sRGB gamma space," and "Why LUTs are in sRGB gamma space."
2. Sub-project #4 shaped `explanation/` into a single flat list of algorithm pages. There is no architectural counterpart and no convention for sibling page structure once architectural prose lands.

The original #8 framing — "add four pages" — is now too narrow. Adding architectural prose alongside the existing algorithm pages without addressing the category errors in `concepts/` would permanently encode an inconsistent split: explanation content in the reference quadrant, and the new explanation pages duplicating part of what `concepts/` already covers.

Sub-project 8 should instead deliver three intertwined things: codify Diataxis-derived rules for content placement and page structure; apply those rules to the existing pages with category errors; add the missing prose explanations the umbrella design called for.

## Goal

Finish the Documentation Initiative's content side by:

1. Establishing concrete authoring rules — derived from the Diataxis framework — that govern which quadrant a piece of content belongs in, how to structure pages within a section, and how to structure sections within a page. Future PRs are reviewed against these rules.
2. Splitting `concepts/render-pipeline.md`, `concepts/preset-model.md`, `concepts/color-spaces.md`, and `concepts/lut-format.md` so that reference pages contain only lookup material and explanation pages carry the rationale.
3. Reorganizing `explanation/` into two sub-sections — `concepts/` for architectural prose and `algorithms/` for the existing algorithm pages.
4. Writing three new explanation pages: `architecture.md`, `philosophy.md`, `design-decisions.md`.
5. Trimming `README.md` and `ARCHITECTURE.md` to short summaries plus pointers into the book, while preserving the contract material (dependency graph, rules table, invariants list) in `ARCHITECTURE.md` for repo-level discoverability.

## Non-goals

- New tutorials, how-to guides, or reference content — out of scope.
- Polish/theme work (sub-project #9) — out of scope.
- Audit and rewrite of the photography lexicon pages (`tone.md`, `color.md`, `detail.md`, `effects.md`) and `color-models.md` — those are clean reference and stay as-is.
- New algorithm pages or rewrites of existing algorithm prose — the algorithm pages move location but their content does not change.
- Changes to top-level book sections — Diataxis already maps cleanly onto the existing top-level structure (intro, install, tutorials, how-to, reference, explanation).

## Audience

Same as the umbrella initiative: CLI users and preset authors first, curious photo nerds second, contributors third. The new explanation pages mainly serve the second and third audiences — readers who want to understand *why* AgX is shaped the way it is, not just look up *what* the shape is.

## Diataxis-derived authoring rules

Five rules to add to `docs/contributing/documentation-conventions.md`. The current conventions doc establishes the four quadrants and surface-mapping in a single paragraph; that paragraph gets refactored into a structured "Audiences and the Diataxis quadrants" section that ends with these rules as a numbered subsection. The conventions doc is also refactored where helpful for clarity, not just appended to.

### Rule 1 — The category test (which quadrant?)

A page belongs in the quadrant that matches its **dominant reader intent**, not its topic. A page about color spaces could live in reference (look up the conversion formulas) or explanation (understand why operations live in different spaces). The test: ask "what is the reader trying to do *right now*?" — look up a fact (reference), follow a recipe (how-to), learn from scratch (tutorial), or understand the why (explanation). One topic can have multiple pages, one per quadrant where reader demand exists.

### Rule 2 — One quadrant per page

A reference page must not contain "Why X" sections. An explanation page must not contain exhaustive enumeration of fields and ranges. When a single page would naturally serve both intents, split it: factual lookup material stays in reference; rationale and design discussion move to a paired explanation page. Pages in this kind of pair link to each other via a `See also` block.

### Rule 3 — Page structure within a section

Every section has an `index.md` landing page that orients the reader: what is here, how to navigate, what to read first. Sibling pages within a section share a consistent skeleton where reasonable. Algorithm explanation pages share one skeleton (mermaid diagram → prose include → `See also`). Conceptual explanation pages share a different skeleton (intro → named subsections → `See also`). Reference concept pages share another (intro → named subsections covering the topic → `See also`). Consistency within a section, variation across sections.

### Rule 4 — Section structure within a page

Headings should reflect reader intent, not implementation structure. `## Why pipeline order matters` is a reader-intent heading; `## Implementation` is not. Headings starting with "Why" or "How" usually belong in explanation pages. Headings that are nouns naming an artifact (a CLI flag, a type, a color space, a preset field) usually belong in reference pages. When a heading in a page does not match the page's quadrant, that is a signal to split.

### Rule 5 — Cross-quadrant linking

Reference pages link out to explanation for the "why" via a `See also` block at the bottom. Explanation pages link to reference for exhaustive lookup. Tutorials link forward to how-to and reference for next steps. How-tos link back to reference for fields and forward to explanation for context. The site's link graph should be navigable: from any quadrant, the reader can reach the others. `mdbook-linkcheck` enforces that the links resolve; the rule itself is about *what* to link, not *how* to express it.

## Page-by-page split rules

Three concept pages have clear category errors and must be split. One has a narrow trim. The split is mechanical once the reader-intent test in Rule 1 is applied — material that answers "look it up" stays in reference; material that answers "understand why" moves to explanation.

### `render-pipeline.md`

**Stays in `reference/concepts/render-pipeline.md`:**

- The "Stages" mermaid diagram and accompanying per-stage list of color space + pass description.
- A short "Color space discipline" paragraph (one to two sentences) noting that each stage runs in the space its math is correct in, with a `See also` link to `concepts/color-spaces.md` for the lookup view of that fact.

**Moves to `explanation/concepts/render-pipeline.md`:**

- The "Why pipeline order matters" section in full — the worked examples explaining why exposure precedes tonal sliders, why dehaze and denoise stay in linear space, why dehaze precedes denoise, why the LUT lives where it does inside the per-pixel pass, and why grain follows detail and dehaze.
- Plus a short orienting intro for the explanation page that frames the topic ("the render pipeline is a fixed sequence of stages, and the order is load-bearing — this page explains why").

### `preset-model.md`

**Stays in `reference/concepts/preset-model.md`:**

- "Three parts" — metadata, partial parameters, optional LUT.
- "The `extends` chain" — load order, merge semantics, recursion-through-composites, last-write-wins-at-leaf.

**Moves to `explanation/concepts/preset-model.md`:**

- "Why partial parameters" — the framing that presets describe what they change, not the full parameter set.
- "Mental model" — the patch-on-baseline framing.
- A short orienting intro that frames the topic ("a preset is a description of an edit; this page explains the design decisions behind that description").

### `color-spaces.md`

**Stays in `reference/concepts/color-spaces.md`:**

- "Linear vs sRGB Gamma" — definitions, the conversion formulas, what each value means at 0.5.
- "Working space" — one to two sentences defining the term and stating that AgX's working space is sRGB.
- A new short "Per-stage color-space table" — a compact table mapping each pipeline stage to the space it runs in, for at-a-glance lookup. (This is a reference-friendly extraction of facts that today live inside the prose.)

**Moves to `explanation/concepts/color-spaces.md`:**

- "The AgX Pipeline" prose section.
- "Why exposure and white balance are in linear space."
- "Why tone adjustments are in sRGB gamma space."
- "Why LUTs are in sRGB gamma space."
- "Current limitations" and "Future: wider color spaces" — these are design-rationale prose, not lookup facts.
- A short orienting intro framing the topic.

### `lut-format.md` (narrow trim)

The bulk of `lut-format.md` is genuine spec reference (file-format syntax, header keywords, data layout, interpolation, sizes). It stays.

The "How AgX generates its bundled LUTs" section is implementation rationale, not format reference. It moves out of `concepts/lut-format.md`. Two viable destinations are folded into a single decision made during implementation:

- A short page `explanation/concepts/lut-generation.md` with just this content. Cleaner Diataxis split.
- A subsection of `explanation/concepts/design-decisions.md`. Lower page count.

The implementation plan picks one based on whether the prose stands alone as a self-contained explanation page or feels orphaned without the surrounding decisions context.

### Cross-link audit

Every split adds reciprocal `See also` blocks: each reference page points at its paired explanation page; each explanation page points at its paired reference page. Every other place in the book that links into one of the four pages above is audited during implementation; most inbound links keep pointing at the reference page (which is still the canonical lookup), but a few ("see why X happens") are redirected to the explanation page. `mdbook-linkcheck` catches any broken links during the implementation PR.

## New explanation pages

Three new pages under `explanation/concepts/`. Outlines below describe structure, source material, and the key sections each page must cover. Prose is written during implementation, not in this design.

### `architecture.md` — discursive expansion of `ARCHITECTURE.md`

Source material: `ARCHITECTURE.md`, the per-module README files, and `docs/plans/2026-02-14-architecture-design.md`.

Sections:

- **Intro.** AgX as a layered library — the layers (adjust, lut, decode, metadata, encode, preset, engine, agx-cli, agx-docgen, agx-e2e, agx-lut-gen) and what the contract between them is.
- **The dependency graph.** Same diagram as `ARCHITECTURE.md`, with prose discussion of *why* it is shaped this way — which boundaries are load-bearing, which are conventional, why `engine` sits at the bottom of the library and not above other modules, why `agx-cli` depends only on the library API and never on internal modules.
- **Core invariants explained.** The five invariants from `ARCHITECTURE.md` (always-re-render-from-original, declarative presets, sRGB only, fixed render order, dual pipeline same output) each get a paragraph of *why this invariant exists* — what would break without it, what was considered instead. This page is the prose counterpart to the bare list in `ARCHITECTURE.md`.
- **Negative constraints.** Discussion of "what's deliberately NOT in each module" with concrete examples of where the temptation has come up (and how the right answer was to push the work to a different module instead of crossing a boundary).
- **When the architecture should evolve.** Pointer to `docs/contributing/evolving-architecture.md`, with framing on when boundary changes are worth doing vs working around.

### `philosophy.md` — preset-first photo editing

Source material: the "Philosophy" section in `README.md`, the project context in `docs/backlog/README.md`, and the framing scattered through how-to guides.

Sections:

- **Intro.** "AgX is preset-first." This page makes that explicit and explains what it implies.
- **Presets as recipes.** What a recipe means here vs an operation log. Why the recipe model is durable across software upgrades and across machines, and why it makes presets shareable in a way that opaque sidecar formats are not.
- **Batch-oriented, not pixel-level.** What AgX is good at and what it deliberately is not. No UI for spot retouching, no undo stack, no local adjustments yet — and why these are cohereent omissions, not gaps awaiting features.
- **Shareable by design.** Plain text, version controllable, fork-and-remix. The marketplace-future framing.
- **CLI and API first.** Why no GUI is on the critical path, what changes if and when one is added.
- **See also.** Cross-links to `concepts/preset-model.md` (the lookup view of how presets compose) and `explanation/concepts/preset-model.md` (the mental model for how presets layer onto the baseline).

### `design-decisions.md` — invariants and load-bearing decisions

Source material: the existing design docs in `docs/plans/`, the umbrella initiative design, and the architecture explanation page above.

Structure:

- **Top: load-bearing invariants.** A bulleted list of the five invariants from the architecture page plus the philosophical invariants (preset-first, sRGB only, fixed pipeline order, no operation order in presets). Each one-liner is a hyperlink to the corresponding entry in the narrative below. This gives readers a fast, scannable answer to "what defines AgX?"
- **Body: 6–8 narrative entries.** Each entry is one decision and follows the same skeleton:
  - **What we chose** (one paragraph).
  - **What we considered** (one paragraph — alternatives that were on the table).
  - **Why we chose this** (one paragraph — the reasoning at the time).
  - **What this costs** (one paragraph — what is harder because of this choice).

Initial decisions to cover (final list confirmed during implementation):

1. Always-re-render-from-original.
2. Declarative presets (no operation order).
3. sRGB-only working space.
4. Fixed pipeline order.
5. Dual CPU + GPU pipeline with CPU as the canonical path.
6. Preset partial-parameter merge semantics (recursive merge of composite sections, last-write-wins at the leaf).
7. LUT applied in sRGB gamma space.
8. Preset-first scope (no UI on the critical path).

The page is curated synthesis, not a re-listing of the design docs in `docs/plans/`. Each narrative entry links to one or more design docs for the full historical record.

## Directory restructure and `SUMMARY.md`

The `explanation/` directory grows from a flat list of algorithm pages into two sub-sections.

### New layout

```
explanation/
├── index.md                  ← landing page, points at both sub-sections
├── concepts/
│   ├── index.md              ← lists architectural prose pages
│   ├── architecture.md       ← NEW
│   ├── philosophy.md         ← NEW
│   ├── design-decisions.md   ← NEW
│   ├── render-pipeline.md    ← split from reference/concepts/
│   ├── preset-model.md       ← split from reference/concepts/
│   └── color-spaces.md       ← split from reference/concepts/
└── algorithms/
    ├── index.md              ← lists algorithm pages in pipeline order
    ├── basic.md              ← moved
    ├── color-grading.md      ← moved
    ├── dehaze.md             ← moved
    ├── denoise.md            ← moved
    ├── detail.md             ← moved
    ├── grain.md              ← moved
    ├── hsl.md                ← moved
    ├── tone-curves.md        ← moved
    └── vignette.md           ← moved
```

### Mechanical considerations

- Each algorithm wrapper page in `explanation/algorithms/` continues to pull canonical prose from `crates/agx/src/adjust/*.md` via `{{#include}}`. The `..` count in the include path increases by one because of the new directory level — every algorithm wrapper page has its include path updated.
- Mermaid diagrams and `Related` blocks stay in their algorithm wrapper pages (per the existing convention).
- `Related` cross-links between algorithm pages get updated to point at sibling algorithm paths under the new `algorithms/` directory.
- A new `explanation/algorithms/index.md` is created — short list of algorithms in pipeline order, mirroring what the current `explanation/index.md` does today.
- A new `explanation/concepts/index.md` is created — short list of architectural prose pages.
- The existing `explanation/index.md` becomes a thin landing page that points at both sub-section indexes.

### `SUMMARY.md` Explanation section

```markdown
# Explanation

- [Overview](explanation/index.md)
- [Concepts](explanation/concepts/index.md)
  - [Architecture](explanation/concepts/architecture.md)
  - [Preset-first philosophy](explanation/concepts/philosophy.md)
  - [Design decisions](explanation/concepts/design-decisions.md)
  - [Render pipeline](explanation/concepts/render-pipeline.md)
  - [Preset model](explanation/concepts/preset-model.md)
  - [Color spaces](explanation/concepts/color-spaces.md)
- [Algorithms](explanation/algorithms/index.md)
  - [Basic adjustments](explanation/algorithms/basic.md)
  - [HSL](explanation/algorithms/hsl.md)
  - [Color grading](explanation/algorithms/color-grading.md)
  - [Tone curves](explanation/algorithms/tone-curves.md)
  - [Vignette](explanation/algorithms/vignette.md)
  - [Grain](explanation/algorithms/grain.md)
  - [Dehaze](explanation/algorithms/dehaze.md)
  - [Noise reduction](explanation/algorithms/denoise.md)
  - [Detail pass](explanation/algorithms/detail.md)
```

The "browse by photographer-panel mental model" framing currently in `explanation/index.md` migrates into `explanation/algorithms/index.md`, since it is specifically about grouping the algorithm pages.

## `README.md` and `ARCHITECTURE.md` retention

Per the umbrella design's "Repo discoverability" constraint, neither file becomes an empty stub. Each retains a short summary plus a clear pointer into the book. The mechanical principle: anything that is *a fact a contributor will look up while editing code* stays in `ARCHITECTURE.md`; anything that is *prose explaining design rationale* moves to the explanation page, with `ARCHITECTURE.md` keeping a one-line pointer.

### `README.md` (root)

| Section | Action |
|---------|--------|
| Project name + one-line description + AgX-chemistry note | Keep |
| Philosophy | **Trim** to a four-bullet summary (presets as recipes, batch-oriented, shareable, CLI/API first), each ending with a pointer to `docs/book/src/explanation/concepts/philosophy.md` for the full discussion |
| Features | Keep |
| Install | Keep |
| Sample images | Keep |
| Quick Start | Keep |
| Metadata Preservation | Keep |
| Preset Format | **Trim** to a 10-line example + pointer to `reference/preset.md` for the full schema and `concepts/preset-model.md` for the mental model |
| Library Usage | Keep |
| Project Structure | **Trim** to a 5-line tree summary + pointer to `ARCHITECTURE.md` |
| Architecture | **Trim** to a one-paragraph summary + pointer to `explanation/concepts/architecture.md` (discussion) and `ARCHITECTURE.md` (contract) |
| Testing | Keep |
| Building with Raw Support | Keep |
| License | Keep |

The trims preserve repo-level first-contact value (a visitor lands on the GitHub page and sees what AgX is, what it does, and how to install it) while moving long-form discussion to the book.

### `ARCHITECTURE.md` (root)

| Section | Action |
|---------|--------|
| Top-of-file framing | **Add** a short "How to read this file" note: `ARCHITECTURE.md` is the *contract* (what the boundaries are); `explanation/concepts/architecture.md` is the *discussion* (why they're shaped this way). Read this for rules, read the book for understanding |
| Module Dependency Graph (ASCII tree) | Keep — canonical contract diagram |
| Dependency Rules table | Keep — the contract |
| Negative Constraints | Keep |
| Core Invariants | **Trim** to a numbered list of one-sentence invariant statements; each item ends with a pointer to the matching narrative entry in `explanation/concepts/architecture.md` |
| Per-Module Details | Keep |
| Design Docs index table | Keep — the `docs/plans/` index lives here |
| When a Structural Test Fails | Keep — operational doc for contributors |

The principle: a contributor running `grep` from a checked-out repo can still find every rule that governs the codebase. Discussion of *why* the rules exist lives one click away in the book.

## Enforcement and CI

No new lints or CI gates introduced by this sub-project. Existing enforcement is sufficient:

- `mdbook-linkcheck` — catches broken intra-site links from the moves.
- `markdownlint-cli2` — catches markdown style regressions.
- `cargo doc --no-deps --workspace` with `RUSTDOCFLAGS="-D warnings"` — catches broken intra-doc links from any `///` or `//!` cross-references that touch moved pages.
- The `back-links` and `sibling-md-clean` enforcement added in sub-project #5 — continues to apply to algorithm wrapper pages in their new location.

The implementation plan should explicitly run all of these as part of verification and address any failures inline.

## Implementation phasing

The implementation lands as a single PR. The user's preference is one PR over multiple to reduce churn and keep the conventions, splits, and new content reviewable as a coherent unit.

Within the single PR, commits are staged carefully so each is independently sensible:

1. Refactor `documentation-conventions.md` and add the five Diataxis-derived rules.
2. Create `explanation/concepts/` and `explanation/algorithms/` directories. Move the nine algorithm wrapper pages into `algorithms/`. Update `{{#include}}` paths and inter-algorithm cross-links. Update `SUMMARY.md`.
3. Split `concepts/render-pipeline.md`, `concepts/preset-model.md`, `concepts/color-spaces.md` into reference + paired explanation pages. Add `See also` blocks on both sides. Update `SUMMARY.md` for the new explanation pages.
4. Trim `concepts/lut-format.md` and decide the destination for the LUT-generation prose (its own page or folded into design-decisions).
5. Write `explanation/concepts/architecture.md`.
6. Write `explanation/concepts/philosophy.md`.
7. Write `explanation/concepts/design-decisions.md`.
8. Trim `README.md` per the retention rules above.
9. Trim `ARCHITECTURE.md` per the retention rules above.
10. Cross-link audit — sweep every inbound link to the four split concept pages and redirect "why" links to the explanation counterpart. Run `verify.sh` and fix any breakage.
11. Update `docs/backlog/documentation-initiative.md` to check off sub-project #8.

The commit boundaries are a recommendation; the implementation plan refines them as needed.

## Success criteria

The sub-project is "done" when:

1. `docs/contributing/documentation-conventions.md` is refactored and includes the five Diataxis-derived rules (category test, one quadrant per page, page structure, section structure, cross-quadrant linking).
2. `reference/concepts/render-pipeline.md`, `preset-model.md`, `color-spaces.md` carry only reference material; their "why" content has moved to paired `explanation/concepts/` pages with reciprocal `See also` blocks.
3. The "How AgX generates its bundled LUTs" prose has moved out of `concepts/lut-format.md` to its decided destination.
4. `explanation/concepts/` exists with `architecture.md`, `philosophy.md`, `design-decisions.md`, `render-pipeline.md`, `preset-model.md`, `color-spaces.md`, plus an `index.md`.
5. `explanation/algorithms/` exists with all nine existing algorithm pages moved in. `{{#include}}` paths and inter-algorithm cross-links resolve. An `algorithms/index.md` lists them in pipeline order.
6. `explanation/index.md` is a thin landing page pointing at both sub-section indexes.
7. `SUMMARY.md` reflects the new structure with the two sub-sections under Explanation.
8. `README.md` is trimmed per the retention rules; long-form Philosophy and Architecture discussion lives in the book with pointers from the README.
9. `ARCHITECTURE.md` is trimmed per the retention rules; the contract material (dependency graph, rules table, negative constraints, design docs index) is intact, and discussion of *why* the invariants exist lives in `explanation/concepts/architecture.md`.
10. `scripts/verify.sh` passes — markdown-lint, mdbook-linkcheck, doc tests, sibling-md-clean, back-links.
11. `docs/backlog/documentation-initiative.md` checks off sub-project #8 as complete.

The Documentation Initiative as a whole is complete after this sub-project, save for the optional sub-project #9 (theme polish), which is deferred unless the default mdbook theme feels inadequate after content lands.

## Related

- [Documentation Initiative Design](2026-04-06-documentation-initiative-design.md) — umbrella design doc.
- [Conceptual reference refresh](2026-04-25-conceptual-reference-refresh-design.md) — sub-project #5, which introduced the concept pages now being split.
- [Tutorials and How-to design](2026-04-26-tutorials-and-how-to-design.md) — sub-projects #6 and #7, which established the audience-driven framing this sub-project follows.
- [Diataxis framework](https://diataxis.fr/) — the framework the rules are derived from.
- [`docs/contributing/documentation-conventions.md`](../contributing/documentation-conventions.md) — the conventions doc this sub-project refactors and extends.
