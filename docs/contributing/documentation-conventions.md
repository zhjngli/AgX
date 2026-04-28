# Documentation conventions

This document describes the conventions for writing documentation in AgX. It is the reference for the rules; the [umbrella documentation initiative](../plans/2026-04-06-documentation-initiative-design.md) explains the rationale.

## Audiences and the Diataxis quadrants

AgX's documentation is organized using the [Diataxis framework](https://diataxis.fr/), which splits documentation into four quadrants based on what the reader wants from the document:

- **Tutorials** are learning-oriented walkthroughs. The reader is new to AgX and wants to be guided through a successful first experience.
- **How-to guides** are task-oriented recipes. The reader knows what they want to accomplish and needs the steps.
- **Reference** is information-oriented. The reader needs to look something up — a CLI flag, a preset field, a function signature, a color space conversion.
- **Explanation** is understanding-oriented. The reader wants to know how something works under the hood and why it was designed that way.

### Surface mapping

Tutorials and how-to guides live exclusively in mdbook under `docs/book/src/`. Reference content is split: the library API reference is rendered by rustdoc; the CLI reference and preset format reference are rendered by mdbook (auto-generated from `clap::Command` and serde types via the `agx-docgen` crate); the conceptual reference (color spaces, LUT format, photographic terminology) is hand-written prose under `docs/book/src/reference/concepts/`. Explanation content is split similarly: algorithm explanations live as sibling `.md` files alongside the Rust source and are pulled into both rustdoc and mdbook from the same file; architectural and philosophical explanations live in mdbook only at `docs/book/src/explanation/concepts/`.

### Authoring rules

Five rules govern content placement and page structure. New PRs are reviewed against these rules.

#### Rule 1 — The category test (which quadrant?)

A page belongs in the quadrant that matches its **dominant reader intent**, not its topic. A page about color spaces could live in reference (look up the conversion formulas) or explanation (understand why operations live in different spaces). Apply the test by asking: "what is the reader trying to do *right now*?" — look up a fact (reference), follow a recipe (how-to), learn from scratch (tutorial), or understand the why (explanation). One topic can have multiple pages, one per quadrant where reader demand exists.

#### Rule 2 — One quadrant per page

A reference page must not contain "Why X" sections. An explanation page must not contain exhaustive enumeration of fields and ranges. When a single page would naturally serve both intents, split it: factual lookup material stays in reference; rationale and design discussion move to a paired explanation page. Pages in this kind of pair link to each other via a `See also` block.

#### Rule 3 — Page structure within a section

Every section has an `index.md` landing page that orients the reader: what is here, how to navigate, what to read first. Sibling pages within a section share a consistent skeleton where reasonable. Algorithm explanation pages share one skeleton (mermaid diagram → prose include → `See also`). Conceptual explanation pages share a different skeleton (intro → named subsections → `See also`). Reference concept pages share another (intro → named subsections → `See also`). Consistency within a section, variation across sections.

#### Rule 4 — Section structure within a page

Headings should reflect reader intent, not implementation structure. `## Why pipeline order matters` is a reader-intent heading; `## Implementation` is not. Headings starting with "Why" or "How" usually belong in explanation pages. Headings that are nouns naming an artifact (a CLI flag, a type, a color space, a preset field) usually belong in reference pages. When a heading in a page does not match the page's quadrant, that is a signal to split.

#### Rule 5 — Cross-quadrant linking

Reference pages link out to explanation for the "why" via a `See also` block at the bottom. Explanation pages link to reference for exhaustive lookup. Tutorials link forward to how-to and reference for next steps. How-tos link back to reference for fields and forward to explanation for context. The site's link graph should be navigable: from any quadrant, the reader can reach the others. `mdbook-linkcheck` enforces that the links resolve; the rule itself is about *what* to link, not *how* to express it.

## Item-level documentation: `///` comments

Every public item in `crates/agx/` and `crates/agx-cli/` should carry a `///` doc comment.

The first line is an imperative summary (e.g., `/// Decode a raw image file into a linear RGB buffer.`). A blank line separates the summary from any further detail. Optional sections include `# Examples`, `# Errors`, and `# Panics`, in that order. Examples in `# Examples` should compile — `cargo doc` runs them as doctests when the example is in a fenced Rust code block.

Cross-references to other items in the crate use rustdoc's intra-doc link syntax (`` [`Engine`] ``, `` [`crate::adjust::grain`] ``), not raw URLs. The `deny(rustdoc::broken_intra_doc_links)` lint validates these on every `cargo doc` run.

## Module-level documentation: `//!` comments

Every module file should carry a `//!` doc comment that summarizes its contents. For modules in `crates/agx/src/adjust/` whose explanation is shared with the mdbook explanation pages, the prose lives in a sibling `.md` file (see "The shared `.md` file convention" below) instead of being inlined into `//!` lines.

For modules whose `//!` doc is short and not reused on the site, inline `//!` lines are fine — there is no need to introduce a sibling `.md` file just to preserve uniformity.

## The shared `.md` file convention

When a module's explanation is meant to appear in both rustdoc and the mdbook site, the canonical source lives as a markdown file alongside the Rust source. The shared file is a sibling of the `.rs` file. The Rust file pulls it into rustdoc via `#![doc = include_str!("<name>.md")]`; the mdbook page pulls the same file via `{{#include}}`.

The canonical worked example is the grain algorithm. The shared file is `crates/agx/src/adjust/grain.md`. The Rust side at `crates/agx/src/adjust/grain.rs` reads:

```rust
#![doc = include_str!("grain.md")]
//!
//! See [`super::dehaze`] and [`super::denoise`] for related passes.

// ... rest of the file ...
```

The mdbook side at `docs/book/src/explanation/algorithms/grain.md` reads:

```markdown
# Grain

{{#include ../../../../../crates/agx/src/adjust/grain.md}}

## Related

- [Grain API reference](../../api/agx/adjust/grain/index.html)
```

There is no anchor mechanism to maintain. The entire shared `.md` file is the explanation content. Editing it updates both surfaces on the next build with no possibility of drift.

Every shared `.md` file starts with an HTML comment block that names the canonical source file and records the bidirectional editing rule:

```markdown
<!-- Canonical source: crates/agx/src/adjust/<name>.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->
```

The rule is symmetric: when the algorithm changes materially, update the sibling `.md`; when the sibling `.md` changes materially, verify its claims against the code before committing.

The shared `.md` file should contain only the reusable explanation prose, with no top-level heading and no cross-`.md` links. Reasons:

- Both consuming surfaces add their own outer heading. A heading inside the included file would render twice.
- Cross-file references differ between the two surfaces. Rustdoc intra-doc syntax (`` [`super::dehaze`] ``) does not resolve in mdbook. Conversely, mdbook relative links into `../api/` or another `.md` file do not resolve correctly when included in a rustdoc page. Keeping shared files free of cross-`.md` links avoids the cross-surface link problem.

Cross-references that need rustdoc validation live in inline `//!` lines on the Rust side, **outside** the include. Cross-references that need mdbook resolution live in the wrapping mdbook page, **outside** the include. The grain example demonstrates both patterns. External `https://` links are fine in a shared file because both rustdoc and mdbook render them the same way.

Use GitHub-Flavored Markdown footnotes only within the same shared file. Do not use a footnote in a shared file to point at another source file or mdbook page; put that cross-file reference in the wrapping mdbook file instead.

If a shared algorithm also has WGSL shader implementations, the shader files use the structured header format described in "WGSL shader headers" below.

## WGSL shader headers

Non-common WGSL shader files start with a five-line structured header. Keep the field names and order fixed:

```wgsl
// Algorithm: <short description of the shader pass>
// Canonical explanation: <shared .md or reference page>
// CPU equivalent: <Rust source path and function when applicable>
// Bindings: <storage/uniform bindings in human-readable form>
// Entry points: main
```

The header lets maintainers connect each shader to the canonical prose, the CPU implementation it must match, and the bind groups a dispatcher is expected to provide. Current shader headers use `Entry points: main`, and `scripts/verify.sh wgsl-headers` enforces that convention. Common utility modules under `crates/agx/src/shaders/common/` may use simpler file comments when they are not standalone algorithm passes.

## Linking between surfaces

- **Rustdoc to mdbook:** use raw HTTPS URLs to the deployed site. Rustdoc has no intra-doc syntax for links to mdbook content.

  ```markdown
  See the [grain explanation](https://zhjngli.github.io/AgX/explanation/algorithms/grain.html) on the project site.
  ```

- **Mdbook to rustdoc:** use relative links into `../api/`. The deploy workflow places rustdoc output at `_site/api/`, so this path is stable on the deployed site.

  ```markdown
  [Grain API reference](../api/agx/adjust/grain/index.html)
  ```

- **Mdbook to mdbook:** use mdbook's native relative-link resolution — relative paths from the file's location to the target file.

  ```markdown
  [Tutorials](../tutorials/getting-started.md)
  ```

- **Rustdoc to rustdoc:** use intra-doc link syntax. The `deny(rustdoc::broken_intra_doc_links)` lint validates these.

  ```rust
  /// See [`crate::adjust::grain`] for the algorithm.
  ```

## Mermaid diagrams in algorithm pages

Larger algorithm pages can include a Mermaid pipeline diagram alongside the prose. Mdbook renders the diagram via the `mdbook-mermaid` preprocessor, which is already wired into `book.toml`.

Diagrams live in the **wrapping mdbook page** (`docs/book/src/explanation/algorithms/<algo>.md`), not in the shared `.md` file. Rustdoc has no Mermaid support, so a fenced `mermaid` block placed in a shared `.md` would render as raw DSL text in the API reference. Keeping diagrams in the wrapper is consistent with the existing rule that surface-specific content lives outside the include.

The typical structure is:

````markdown
# Algorithm name

## Pipeline

```mermaid
flowchart TD
    A[Stage A] --> B[Stage B]
```

One short paragraph summarizing what the diagram shows.

{{#include ../../../../../crates/agx/src/adjust/<algo>.md}}

## See also

- [API reference](../../api/agx/adjust/<algo>/index.html)
````

Notes for diagram authors:

- The vendored `mermaid.min.js` and `mermaid-init.js` files in `docs/book/` are gitignored; `scripts/build-docs.sh` and `scripts/verify.sh book-linkcheck` run `mdbook-mermaid install docs/book` before the build to (re)create them.
- Use ASCII-safe label text (avoid `<`, `>`, raw `&`); HTML entities inside Mermaid labels are fragile across mdbook's markdown escaping. Words like `negative` / `positive` are clearer than `< 0` / `> 0` anyway.
- Use `<br/>` inside quoted node labels for line breaks.

## Markdown linting

`scripts/verify.sh markdown-lint` runs [`markdownlint-cli2`](https://github.com/DavidAnson/markdownlint-cli2) against every `.md` file the lint config reaches. The configuration lives at the repo root in `.markdownlint-cli2.jsonc` and follows the published [markdownlint rule list](https://github.com/DavidAnson/markdownlint/blob/main/doc/Rules.md).

Active rules cover blank-line discipline (around headings, lists, fenced blocks, tables), heading style, and trailing whitespace. Several rules are intentionally disabled — see comments in `.markdownlint-cli2.jsonc` for the reasoning. Notable disables:

- `MD013` (line length) — long-form prose.
- `MD041` / `MD025` — many files have leading HTML comments or no top-level heading by design.
- `MD060` (table column style) — existing tables use compact pipes; aligning would be cosmetic.
- `MD040` (fenced-block language hints) — too many existing design-doc blocks to retrofit; new code blocks should still include a hint by convention.
- `MD029` (ordered-list prefix) — auto-fix renumbers intentionally-continued lists, changing apparent semantics.
- `MD037` / `MD038` / `MD039` — math expressions like `w * h * c` collide with emphasis-pair detection; auto-fix corrupts the prose.

Local invocation, when `markdownlint-cli2` is not on `PATH`, falls back to `npx --yes markdownlint-cli2`. CI runs the linter as the `markdown-lint` entry in the docs matrix; Node is installed via `actions/setup-node`.

To autofix the rules that support it (most blank-line rules):

```bash
npx --yes markdownlint-cli2 --fix
```

The `MD040` retrofit and a possible style normalization sweep across older design docs are tracked as future tightening work in [the documentation initiative backlog](../backlog/documentation-initiative.md).

## Active lints

Both `crates/agx/` and `crates/agx-cli/` carry two crate-level lints:

- `#![deny(missing_docs)]` — every public item without a `///` or `//!` doc comment fails the build. This ensures new public API surface is documented before it compiles.
- `#![deny(rustdoc::broken_intra_doc_links)]` — every broken intra-doc link in a `///` or `//!` comment fails `cargo doc`.

`scripts/verify.sh` runs `cargo doc --no-deps --workspace` with `RUSTDOCFLAGS="-D warnings"` so any rustdoc warning becomes a local verify failure.

## Local preview

For rustdoc:

```bash
cargo doc --no-deps --workspace --open
```

For mdbook (after the one-time install of mdbook and the three preprocessors):

```bash
cargo install --locked mdbook --version 0.4.40
cargo install --locked mdbook-linkcheck --version 0.7.7
cargo install --locked mdbook-mermaid --version 0.14.0
cargo install --locked mdbook-katex --version 0.9.0

mdbook serve docs/book --open
```

`mdbook serve` watches for file changes and auto-rebuilds, so the typical author workflow is to keep it running in a terminal while editing.

## Where the deployed site lives

Pushes to `main` trigger `.github/workflows/docs.yml`, which builds and deploys the site to `https://zhjngli.github.io/AgX/`. mdbook content lives at the root (e.g., `https://zhjngli.github.io/AgX/explanation/algorithms/grain.html`); rustdoc content lives at `/api/` (e.g., `https://zhjngli.github.io/AgX/api/agx/adjust/grain/index.html`).

## Conceptual reference principles

The conceptual reference quadrant lives at `docs/book/src/reference/concepts/`. Sub-project 5 of the documentation initiative shaped this surface; the principles here govern how it evolves.

- **Purpose split.** Conceptual reference covers *what* a concept is in photography or AgX terms. Algorithm explanations cover *how* AgX implements it (with math). Tutorials say *do this*. How-to guides say *achieve this goal*. Each surface stays in its lane — content does not leak across.
- **Lexicon vs deep dive.** Photographic vocabulary (the `tone.md`, `color.md`, `detail.md`, `effects.md` pages) stays terse: 2-4 sentences per term. Foundations (color spaces, color models) and AgX-specific concepts (preset model, render pipeline, LUT format) get full pages with depth. Don't promote a lexicon entry to its own page unless it visibly outgrows a paragraph.
- **Lexicon grouping.** Photographer-panel mental model — `tone.md` / `color.md` / `detail.md` / `effects.md`. Not pipeline order. New algorithms join the page that matches their photographic role, not the page that matches their pipeline position.
- **Anchor stability.** Headings in `reference/concepts/*.md` are public link targets cited by tutorials, how-to guides, and explanation pages. Renaming a heading is a breaking change to the doc surface. `mdbook-linkcheck` (in `book-linkcheck`) will fail the build if you rename a heading without updating callers, but you should also `git grep` for the old name to find indirect references.
- **Single home per concept.** Each concept has one canonical defining location. Other pages mention the term and link to its home rather than re-defining. For example, "LUT" is defined in `lut-format.md`; `color.md` mentions it but does not redefine it.
- **Per-page depth ceiling.** A concept page covers its scope in 1-3 screens, AgX-shaped, links out for theory. If a page balloons, split it or move detail to an explanation page.

## Cross-link contract

The conceptual reference surface holds together because of four directional rules. The first three are mechanically enforced; the fourth is the contract tutorials and how-to guides can rely on.

- **Concepts → Explanation (always).** Every lexicon page ends with a footer linking to the relevant `explanation/` page(s). Foundation and AgX-specific pages link to relevant explanations where they exist. Code review.
- **Explanation → Concepts (bidirectional).** Every wrapper page in `docs/book/src/explanation/algorithms/*.md` (the page that wraps a sibling `.md` include — not the sibling itself) ends with a `## See also` block listing relevant concept pages, the rustdoc API references, related explanations, and (where applicable) how-to recipes. The bare `explanation/*.md` glob now matches only `index.md` pages (section indices), which have no `## See also` requirement. `scripts/verify.sh back-links` mechanically enforces *presence* of the heading; the *contents* of each block are a code-review concern — the check does not validate that the listed targets exist or are relevant.
- **Sibling `.md` cleanliness.** Files at `crates/agx/src/adjust/*.md` (the canonical algorithm-explanation prose included by both rustdoc and mdbook) contain no relative or non-HTTPS markdown links. External `https://` links remain allowed (they render uniformly across rustdoc and mdbook). The convention exists for the cross-surface reasons documented under "The shared `.md` file convention" above; sub-project 5 added `scripts/verify.sh sibling-md-clean` to enforce it.
- **Tutorials / how-to guides → Concepts (forward-compatible).** Tutorials and how-to guides cite concept anchors directly: `reference/concepts/color.md#white-balance`. Anchors are auto-generated from headings; renaming a heading is a breaking change. The conceptual reference is the stable surface tutorials lean on.

## Reviewer checklist for `docs/` PRs

For PRs that touch `docs/book/src/reference/concepts/` or `docs/book/src/explanation/`, the reviewer ticks:

- [ ] New or changed concept page stays within depth ceiling (1-3 screens, AgX-shaped, links out for theory).
- [ ] No definition duplicates an existing definition elsewhere in the docs.
- [ ] Renamed headings: callers updated (or the linkcheck failure is acknowledged and fixed).
- [ ] New lexicon entry placed under the right group (panel mental model, not pipeline order).
- [ ] Cross-link footers present (concepts → explanation; explanation → concepts).
- [ ] `## See also` block contents are accurate and current — the mechanical check only verifies the heading exists, not what's inside it.

The mechanical checks (`back-links`, `sibling-md-clean`, `book-no-internal-refs`, `book-linkcheck`, `doc-links`, `markdown-lint`) cover the easy cases; the checklist above covers the judgment calls.

## Public surface boundary

The published mdbook site at `docs/book/src/` is end-user content: CLI users, preset authors, and curious photo nerds. It must not leak project-internal artifacts.

Specifically, content under `docs/book/src/` must not link to:

- `docs/plans/` — design docs and implementation plans (project planning).
- `docs/backlog/` — work backlog (project management).
- `docs/contributing/` — contributor guides (this file included).

Why: these surfaces serve different audiences and have different expectations. A reader landing on a tutorial doesn't want to be sent into a roadmap document; a contributor reading conventions doesn't expect them to be cited from the user-facing render-pipeline page. Mixing the two blurs the audience boundary and tends to drag stale planning details onto the published site.

Repo-level entry points (`README.md`, `ARCHITECTURE.md`) are different — they are explicitly contributor-facing and may link freely into `docs/plans/`, `docs/backlog/`, and `docs/contributing/`. Only `docs/book/src/` is constrained by this rule.

Mechanical enforcement: `scripts/verify.sh book-no-internal-refs` greps `docs/book/src/**/*.md` for markdown links into those three directories and fails the build if any are found.

Prose-form leaks (mentioning "the documentation initiative" or a specific sub-project number in the published prose without linking) escape the mechanical check. They are caught by the reviewer checklist and the principle that published prose should describe AgX as a finished tool, not as a project under construction. When a placeholder page needs to say "this section isn't ready yet," prefer a short "Coming soon" framed in terms of the planned content, not a pointer to internal planning.
