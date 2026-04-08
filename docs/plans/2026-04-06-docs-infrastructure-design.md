# Docs Infrastructure & Scaffolding (Sub-project #1)

**Date:** 2026-04-06
**Parent:** [Documentation Initiative](2026-04-06-documentation-initiative-design.md)

## Problem

The umbrella documentation initiative depends on a working build, deploy, and enforcement pipeline before any content sub-project can move forward. Today AgX has no mdbook scaffold, no GitHub Pages workflow, no `cargo doc` gate in CI, and no lint attributes that put pressure on missing documentation. Sub-projects #2–#9 cannot run in parallel — or even start — until that infrastructure exists and has been validated end-to-end with at least one piece of real content.

## Goal

Stand up the full documentation pipeline so that, the moment this sub-project lands, every later sub-project can focus exclusively on writing content. Concretely:

1. An empty-but-valid mdbook lives at `docs/book/`, with one folder per Diataxis quadrant and a single worked example (the grain algorithm) that exercises the shared-source-file mechanism end-to-end.
2. A GitHub Actions workflow builds mdbook + rustdoc on every push to `main` and deploys to GitHub Pages via the modern `actions/deploy-pages` API.
3. `cargo doc` is wired into `scripts/verify.sh` as a hard gate, with `RUSTDOCFLAGS="-D warnings"`.
4. Day-one lint enforcement is in place on both `agx` and `agx-cli`: `warn(missing_docs)` plus `deny(rustdoc::broken_intra_doc_links)`.
5. Repo-wide rename of "oxiraw" to "AgX" in the existing `docs/reference/` files lands as the first commit on the branch (the prep work item from the umbrella spec).
6. A `docs/contributing/documentation-conventions.md` document codifies how authors write `///` and `//!` comments, anchor blocks, and link them to mdbook pages.

The acceptance bar is "merging this sub-project unblocks every sibling sub-project," not "the site has any real content yet." Real content arrives in #2–#8.

## Non-goals

Explicitly out of scope for this sub-project:

- **Auto-generated CLI / preset reference.** That is sub-project #3 and requires the new `agx-docgen` crate. Sub-project #1 ships placeholder pages at `docs/book/src/reference/cli.md` and `docs/book/src/reference/preset.md` that say "generated in sub-project #3" and link back to this design doc.
- **API doc retrofit.** No new `///` comments are added in this sub-project beyond what already exists. The retrofit is sub-project #2; sub-project #1 only sets the lint level to `warn` so the pressure becomes visible.
- **Real algorithm explanations.** Only the grain module gets a worked-example explanation in this sub-project, as a way to validate the shared-source-file pipeline end-to-end. The other adjust modules are sub-project #4.
- **Real tutorials, how-to guides, conceptual reference content, or prose explanations.** Each Diataxis quadrant gets one "this section is being built — see [link to umbrella spec]" placeholder file. Real content is sub-projects #5–#8.
- **External link checking.** No external links exist on the site after sub-project #1, so `mdbook-linkcheck` runs in internal-only mode. The external workflow is added later by whichever sub-project first introduces an external link (see umbrella spec, "Enforcement and CI").
- **PR previews.** Local `mdbook serve` only. Cloudflare Pages or Netlify previews are deferred.
- **Custom domain or theme customization.** Default theme, default `<username>.github.io/AgX/` URL. Theme work is sub-project #9.

## Scope (work items)

The branch lands these items in roughly this order:

1. **Rename "oxiraw" → "AgX"** in `docs/reference/{color-spaces,grain-algorithm,lut-format}.md`. First commit on the branch. Pure find-and-replace, no structural changes. The larger move into `docs/book/src/reference/concepts/` is sub-project #5.
2. **Create the mdbook skeleton** at `docs/book/`, including `book.toml`, `src/SUMMARY.md`, and one placeholder file per Diataxis quadrant.
3. **Add the grain worked example.** Create a new `crates/agx/src/adjust/grain.md` containing the prose explanation, add `#![doc = include_str!("grain.md")]` at the top of `crates/agx/src/adjust/grain.rs` so rustdoc renders the prose as the module-level doc, and create `docs/book/src/explanation/grain.md` containing a `{{#include}}` directive that pulls the same `grain.md` file plus a "Related" link block.
4. **Add a small landing page** at `docs/book/src/introduction.md` with a one-paragraph project summary, a screenshot or two from a curated subset of `example/images/`, and links into each Diataxis quadrant. Sample images are copied (not symlinked) into `docs/book/src/images/`.
5. **Wire mdbook preprocessors:** `mdbook-linkcheck` (internal links only), `mdbook-mermaid`, `mdbook-katex`. Configure each in `book.toml`. Versions are pinned in `docs.yml` via `cargo install --version`, not in `book.toml`.
6. **Add a placeholder favicon** generated from a text-based tool. Real branding is sub-project #9.
7. **Add lint attributes** to both library crates: `#![warn(missing_docs)]` and `#![deny(rustdoc::broken_intra_doc_links)]` in `crates/agx/src/lib.rs` and `crates/agx-cli/src/main.rs`. Excludes `agx-e2e` (test crate, no public API) and `agx-lut-gen` (dev tool, no consumers).
8. **Wire `cargo doc` into `scripts/verify.sh`** as a new check, run with `RUSTDOCFLAGS="-D warnings"`.
9. **Add the docs deploy workflow** at `.github/workflows/docs.yml` as described in the "CI" section below. Triggered on push to `main` and on workflow_dispatch. Builds mdbook + rustdoc + deploys to GitHub Pages.
10. **Write the documentation conventions doc** at `docs/contributing/documentation-conventions.md`. Outline below.

A separate prep step happens before the branch is opened:

- **Enable GitHub Pages on the repo.** The user has already toggled the Pages setting and chosen the "GitHub Actions" source, so this is a one-time external prerequisite, not a code change.

## Repo layout after this sub-project

```
docs/
├── book/
│   ├── book.toml
│   └── src/
│       ├── SUMMARY.md
│       ├── introduction.md
│       ├── images/
│       │   └── (small subset copied from example/images/)
│       ├── tutorials/
│       │   └── index.md              ← placeholder
│       ├── how-to/
│       │   └── index.md              ← placeholder
│       ├── reference/
│       │   ├── cli.md                ← placeholder, sub-project #3
│       │   ├── preset.md             ← placeholder, sub-project #3
│       │   └── concepts/
│       │       └── index.md          ← placeholder
│       └── explanation/
│           ├── index.md              ← placeholder for siblings
│           └── grain.md              ← worked example, includes shared prose
├── backlog/                          ← unchanged
├── contributing/
│   ├── developer-workflow.md         ← unchanged
│   ├── evolving-architecture.md      ← unchanged
│   └── documentation-conventions.md  ← new
├── plans/                            ← unchanged
└── reference/                        ← oxiraw → AgX rename only
```

The grain worked example also adds one new file inside the source tree:

```
crates/agx/src/adjust/
├── grain.rs                          ← gains #![doc = include_str!("grain.md")]
└── grain.md                          ← new, canonical prose for the explanation
```

`.github/workflows/` gains one new file:

```
.github/workflows/
├── ci.yml                            ← unchanged
└── docs.yml                          ← new
```

## Tooling

### `book.toml` essentials

```toml
[book]
title = "AgX"
authors = ["AgX contributors"]
description = "Open-source preset-first photo editing library and CLI"
src = "src"
language = "en"

[output.html]
default-theme = "light"
preferred-dark-theme = "navy"
git-repository-url = "https://github.com/OWNER/AgX"
edit-url-template = "https://github.com/OWNER/AgX/edit/main/docs/book/{path}"
additional-css = ["theme/extra.css"]   # placeholder file, empty for now
mathjax-support = false                 # using mdbook-katex instead

[output.html.fold]
enable = true
level = 1

[preprocessor.linkcheck]
follow-web-links = false                # internal only for now
warning-policy = "error"                # broken internal link → CI failure
exclude = []

[preprocessor.mermaid]
command = "mdbook-mermaid"

[preprocessor.katex]
after = ["links"]
```

`OWNER` is a literal placeholder filled in at implementation time with the GitHub user or organization that owns the AgX repo. The same substitution applies to the `<username>` placeholders in the lint-attribute snippets below.

The CI workflow installs a fixed mdbook version via `cargo install --version` (e.g., `mdbook@0.4.40`, `mdbook-linkcheck@0.7.7`, `mdbook-mermaid@0.14.0`, `mdbook-katex@0.9.0`). Exact versions are confirmed during implementation. `book.toml` itself does not pin mdbook.

### `SUMMARY.md` shape

```markdown
# Summary

[Introduction](introduction.md)

# Tutorials
- [Coming soon](tutorials/index.md)

# How-to guides
- [Coming soon](how-to/index.md)

# Reference
- [CLI](reference/cli.md)
- [Preset format](reference/preset.md)
- [Concepts](reference/concepts/index.md)

# Explanation
- [Overview](explanation/index.md)
- [Grain](explanation/grain.md)
```

The `[Coming soon]` link text is intentional. Each placeholder page links back to the umbrella design doc so a reader landing on a stub knows where the content will come from.

## CI

### `docs.yml` workflow

```yaml
name: Docs

on:
  push:
    branches: [main]
  workflow_dispatch:

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: false

jobs:
  build:
    name: Build site
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install libraw
        run: sudo apt-get update && sudo apt-get install -y libraw-dev
      - name: Install mdbook and preprocessors
        run: |
          cargo install --locked mdbook --version 0.4.40
          cargo install --locked mdbook-linkcheck --version 0.7.7
          cargo install --locked mdbook-mermaid --version 0.14.0
          cargo install --locked mdbook-katex --version 0.9.0
      - name: Build rustdoc
        env:
          RUSTDOCFLAGS: "-D warnings"
        run: cargo doc --no-deps --workspace
      - name: Build mdbook
        run: mdbook build docs/book
      - name: Assemble site
        run: |
          mkdir -p _site
          cp -r docs/book/book/html/* _site/
          mkdir -p _site/api
          cp -r target/doc/* _site/api/
      - uses: actions/upload-pages-artifact@v3
        with:
          path: _site

  deploy:
    name: Deploy to GitHub Pages
    needs: build
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - id: deployment
        uses: actions/deploy-pages@v4
```

Notes:

- The workflow is in its own file (`docs.yml`) rather than added as a job to `ci.yml`. `ci.yml` is PR-only; `docs.yml` triggers on `push: main` and `workflow_dispatch`. Splitting them keeps the responsibilities clean and lets PR runs stay fast.
- `mdbook-linkcheck` runs as a preprocessor during `mdbook build`. There is no separate linkcheck step. If a broken internal link exists, `mdbook build` exits non-zero and the deploy never happens.
- The `concurrency` block prevents two main pushes from racing the deploy.

### `verify.sh` additions

Add a sixth check between the existing CLI tests and the documentation link validation:

```bash
# 5. Rustdoc build (treats warnings as errors)
run_check "Rustdoc (cargo doc)" \
    env RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace

# 6. Documentation link validation
# (existing block, renumbered)
```

This catches `broken_intra_doc_links` violations and any rustdoc warning before they hit CI. Local `verify.sh` runs become slightly slower (the first `cargo doc` is the cost; incremental runs are fast), but the failure mode of "PR fails CI on a rustdoc warning the developer never saw locally" disappears.

`verify.sh` does **not** run `mdbook build`. Reason: not every developer will have `mdbook` and the three preprocessors installed locally, and bootstrapping that as a hard requirement for `verify.sh` is friction we do not want yet. The mdbook build runs only in `docs.yml`. If a future sub-project decides every developer must build the book locally, it can revisit this.

### `ci.yml` changes

`ci.yml` is unchanged in this sub-project. PRs continue to run `verify.sh` (which now includes `cargo doc -D warnings`) and the e2e matrix. Docs deploy is decoupled.

## Lint attributes

### `crates/agx/src/lib.rs`

Add at the top of the file, before the existing `pub mod` declarations:

```rust
//! AgX — open-source preset-first photo editing library.
//!
//! See the [project site](https://OWNER.github.io/AgX/) for tutorials,
//! how-to guides, the CLI reference, and the preset format reference.

#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod adjust;
pub mod decode;
// ...
```

The crate-level `//!` doc is short on purpose. Any expanded prose lives in `docs/book/src/introduction.md` and the explanation pages.

### `crates/agx-cli/src/main.rs`

Add at the very top of the file:

```rust
//! AgX command-line interface.
//!
//! See the [project site](https://OWNER.github.io/AgX/reference/cli.html)
//! for the full CLI reference.

#![warn(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::path::PathBuf;
// ...
```

`agx-cli` is a binary crate, so `missing_docs` only applies to the few `pub` items it does expose (currently the clap structs, several `pub` helpers in `batch.rs`, etc.). Most items in a binary crate are private; the warning surface is small. Sub-project #2 is responsible for actually populating the comments and flipping `warn` → `deny`.

### Why `agx-e2e` and `agx-lut-gen` are excluded

- **`agx-e2e`** is a test crate. Its public surface is test functions; documenting them is noise and has no consumers.
- **`agx-lut-gen`** is a dev tool that produces `.cube` files at build time. It has no library consumers. Documenting its internals is low-value compared to documenting the LUT format itself, which lives in conceptual reference (sub-project #5).

Both crates remain free of these lints. If a future need arises (e.g., `agx-lut-gen` becomes externally interesting), revisit then.

## Worked example: grain

The grain example is the smallest end-to-end validation that the shared-source-file pipeline works. It must:

- Surface in rustdoc as a normal module-level doc comment.
- Surface in mdbook as a rendered page.
- Stay in sync automatically: editing the canonical source updates both surfaces on the next build.

### Mechanism: shared `.md` file alongside the Rust source

The canonical source for the prose is a markdown file that lives next to the Rust file: `crates/agx/src/adjust/grain.md`. The Rust file pulls it in as the module's own documentation via `#![doc = include_str!("grain.md")]`. The mdbook page pulls in the same file via `{{#include ...}}`. There is no anchor mechanism to maintain — the entire `.md` file is the explanation content.

This is the same mechanism described in the umbrella spec's "shared-`.md`-file convention" section. An earlier umbrella draft proposed `{{#rustdoc_include}}` pulling content directly from `//!` blocks in `.rs` files; that mechanism does not actually do what we want, since it includes a Rust file as a syntax-highlighted code block rather than as rendered prose. The umbrella spec was amended in this same PR to reflect the corrected mechanism. The spirit of the original decision is unchanged: prose lives in the source tree, both surfaces include it, drift is structurally impossible.

### Canonical source: `crates/agx/src/adjust/grain.md`

A new file containing only prose, no headers (the rustdoc page and the mdbook page each provide their own outer heading):

```markdown
AgX simulates film grain by convolving white noise with a Gaussian kernel
whose sigma is proportional to the configured grain size, then modulating
the result by per-pixel luminance to mimic the way film grain is more
pronounced in midtones than in deep shadows or bright highlights.

The current algorithm replaced an older frequency-based approach. See the
project's design history for the trade-offs and the reference photographs
that informed the parameter choices.
```

The file is intentionally pure prose with no cross-references. Cross-references that need rustdoc validation live in the wrapping `.rs` file as separate `#![doc = "..."]` attributes. Cross-references for mdbook readers live in the wrapping mdbook page. This split avoids the cross-surface link problem (rustdoc intra-doc syntax does not resolve in mdbook, and vice versa).

### Rust side: `crates/agx/src/adjust/grain.rs`

Add at the top of the file, before the existing `use` statements:

```rust
#![doc = include_str!("grain.md")]
//!
//! See [`super::dehaze`] and [`super::denoise`] for related passes.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
// ... existing code unchanged ...
```

The `#![doc = include_str!("grain.md")]` attribute pulls the prose into the module-level doc. The trailing `//!` lines add rustdoc-native intra-doc links that the `broken_intra_doc_links` lint validates. If either `super::dehaze` or `super::denoise` is renamed or removed, `cargo doc` fails. This is the actual day-one exercise of the lint.

### Site side: `docs/book/src/explanation/grain.md`

```markdown
# Grain

{{#include ../../../../crates/agx/src/adjust/grain.md}}

## Related

- [Grain API reference](../api/agx/adjust/grain/index.html)
```

### Verification

After this sub-project lands, both of the following must hold:

- `cargo doc --no-deps -p agx --open` → the grain module page shows the prose from `grain.md` plus the "See also" links.
- `mdbook build docs/book && open docs/book/book/html/explanation/grain.html` → the grain page shows the same prose from `grain.md`, with the mdbook-side "Related" links underneath.

If either side is missing or differs, the shared-source-file plumbing is broken and must be fixed before sub-project #4 starts at scale.

## Documentation conventions doc

`docs/contributing/documentation-conventions.md` codifies the rules for authors. Outline:

1. **Audience and Diataxis recap.** One paragraph each on the four quadrants and which lives where (rustdoc vs. mdbook). Links to the umbrella design doc for the long version.
2. **`///` item docs.** Style: imperative summary line, blank line, optional details, optional `# Examples`. Examples must compile (`cargo doc` runs them as doctests where applicable). Cross-references use intra-doc link syntax (`[`Engine`]`), not raw URLs.
3. **`//!` module docs.** When to write one (every `crates/agx/src/adjust/*.rs` file at minimum). For modules whose explanation is cross-surface (rustdoc + mdbook), the prose lives in a sibling `.md` file pulled in via `#![doc = include_str!("module.md")]` instead of inline `//!` lines. Inline `//!` lines remain the right tool for short module summaries that do not need to be reused on the site.
4. **The shared-`.md`-file convention.** How to add a sibling `.md` file next to a Rust module file, how to wire it into rustdoc via `#![doc = include_str!(...)]`, and how to wire it into an mdbook page via `{{#include ...}}`. How to keep cross-references out of the shared file (rustdoc-only links go in `#![doc = "..."]` attributes on the Rust side; mdbook-only links go in the wrapping mdbook page). The grain example is the canonical reference.
5. **Linking between rustdoc and mdbook.** Rustdoc → mdbook uses raw URLs (no intra-doc syntax exists for cross-surface links). Mdbook → rustdoc uses relative links into `../api/`. Mdbook → mdbook uses `mdbook`'s native relative-link resolution.
6. **Lints.** Document which lints are active (`warn(missing_docs)` and `deny(rustdoc::broken_intra_doc_links)`) and explain the staging plan: `warn` becomes `deny` at the end of sub-project #2.
7. **Local preview commands.** `cargo doc --open` for rustdoc; `cd docs/book && mdbook serve --open` for mdbook (after `cargo install mdbook mdbook-linkcheck mdbook-mermaid mdbook-katex`).

The doc is reference material for future contributors and for the agents working in this repo. It is written in normal prose, not bullet-only, and it crosslinks back to the umbrella design doc.

## Acceptance criteria

The sub-project is "done" when all of these hold:

1. `mdbook build docs/book` succeeds locally and in CI, with `mdbook-linkcheck`, `mdbook-mermaid`, and `mdbook-katex` all active.
2. `cargo doc --no-deps --workspace` succeeds with `RUSTDOCFLAGS="-D warnings"`, both locally via `verify.sh` and in `docs.yml`.
3. `./scripts/verify.sh` runs the new `cargo doc` check as part of its existing sequence and the script as a whole still passes.
4. `crates/agx/src/lib.rs` and `crates/agx-cli/src/main.rs` both carry `#![warn(missing_docs)]` and `#![deny(rustdoc::broken_intra_doc_links)]`.
5. `crates/agx/src/adjust/grain.md` exists as the canonical source of the grain explanation. `crates/agx/src/adjust/grain.rs` pulls it in via `#![doc = include_str!("grain.md")]` and the rustdoc page for the grain module renders the prose. `docs/book/src/explanation/grain.md` includes the same `grain.md` file via `{{#include ...}}` and renders identical content.
6. `docs/book/src/SUMMARY.md` references one placeholder page per Diataxis quadrant plus the grain explanation.
7. A landing page at `docs/book/src/introduction.md` exists with a one-paragraph summary, at least one image from the small `docs/book/src/images/` subset, and links into each Diataxis quadrant.
8. `.github/workflows/docs.yml` exists, triggers on push to `main` and `workflow_dispatch`, builds mdbook + rustdoc into a single `_site/` tree (mdbook at root, rustdoc at `/api/`), and deploys via `actions/deploy-pages@v4`. The deploy succeeds against GitHub Pages on first run.
9. `docs/contributing/documentation-conventions.md` exists with the seven sections above and is linked from `docs/contributing/developer-workflow.md` (one-line addition).
10. `docs/reference/{color-spaces,grain-algorithm,lut-format}.md` no longer reference "oxiraw" (renamed to "AgX"). The files are otherwise unchanged.
11. The published site is reachable at `https://<username>.github.io/AgX/` and serves the landing page, the placeholder pages, the grain explanation page, and the `/api/` rustdoc tree.

## Open questions (deferred to implementation)

These are intentionally not decided in this design and will be resolved during implementation:

- **Exact mdbook and preprocessor versions.** Pinned in `docs.yml` during implementation; bumping is a routine maintenance change.
- **Theme customization scope.** Default mdbook theme is the starting point. Whether the `additional-css` placeholder file is left empty or filled with a few small tweaks (e.g., wider content column, monospace tweaks) is left to the implementer's taste.
- **Landing page copy.** A short paragraph drawn from `README.md` is the starting point. The implementer may revise it.
- **Which sample images.** Pick two or three representative shots from `example/images/` (one landscape, one portrait, one low-light works well). Final selection at implementation time.
- **Exact favicon.** Generated from a text-to-favicon tool. Specific tool and the rendered glyph are a polish detail, not a structural decision.

## Related

- [Documentation Initiative (umbrella)](2026-04-06-documentation-initiative-design.md)
- [Diataxis framework](https://diataxis.fr/)
- [mdbook documentation](https://rust-lang.github.io/mdBook/)
- [`actions/deploy-pages`](https://github.com/actions/deploy-pages)
- [`docs/contributing/developer-workflow.md`](../contributing/developer-workflow.md) — gains a one-line link to the new conventions doc
