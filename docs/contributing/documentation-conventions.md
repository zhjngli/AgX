# Documentation conventions

This document describes the conventions for writing documentation in AgX. It is the reference for the rules; the [umbrella documentation initiative](../plans/2026-04-06-documentation-initiative-design.md) explains the rationale.

## Audiences and the Diataxis quadrants

AgX's documentation is organized using the [Diataxis framework](https://diataxis.fr/), which splits documentation into four quadrants based on what the reader wants from the document:

- **Tutorials** are learning-oriented walkthroughs. The reader is new to AgX and wants to be guided through a successful first experience.
- **How-to guides** are task-oriented recipes. The reader knows what they want to accomplish and needs the steps.
- **Reference** is information-oriented. The reader needs to look something up — a CLI flag, a preset field, a function signature, a color space conversion.
- **Explanation** is understanding-oriented. The reader wants to know how something works under the hood and why it was designed that way.

Tutorials and how-to guides live exclusively in mdbook under `docs/book/src/`. Reference content is split: the library API reference is rendered by rustdoc; the CLI reference and preset format reference are rendered by mdbook (auto-generated from `clap::Command` and serde types in a later sub-project); the conceptual reference (color spaces, LUT format, photographic terminology) is hand-written prose under `docs/book/src/reference/concepts/`. Explanation content is split similarly: algorithm explanations live as sibling `.md` files alongside the Rust source and are pulled into both rustdoc and mdbook from the same file; architectural and philosophical explanations live in mdbook only.

## Item-level documentation: `///` comments

Every public item in `crates/agx/` and `crates/agx-cli/` should carry a `///` doc comment.

The first line is an imperative summary (e.g., `/// Decode a raw image file into a linear RGB buffer.`). A blank line separates the summary from any further detail. Optional sections include `# Examples`, `# Errors`, and `# Panics`, in that order. Examples in `# Examples` should compile — `cargo doc` runs them as doctests when the example is in a fenced Rust code block.

Cross-references to other items in the crate use rustdoc's intra-doc link syntax (`` [`Engine`] ``, `` [`crate::adjust::grain`] ``), not raw URLs. The `deny(rustdoc::broken_intra_doc_links)` lint validates these on every `cargo doc` run.

## Module-level documentation: `//!` comments

Every module file should carry a `//!` doc comment that summarizes its contents. For modules in `crates/agx/src/adjust/` whose explanation is shared with the mdbook explanation pages, the prose lives in a sibling `.md` file (see "The shared `.md` file convention" below) instead of being inlined into `//!` lines.

For modules whose `//!` doc is short and not reused on the site, inline `//!` lines are fine — there is no need to introduce a sibling `.md` file just to preserve uniformity.

## The shared `.md` file convention

When a module's explanation is meant to appear in both rustdoc and the mdbook site, the canonical source lives as a markdown file alongside the Rust source. The Rust file pulls it into rustdoc via `include_str!`; the mdbook page pulls the same file via `{{#include}}`.

The canonical worked example is the grain algorithm. The shared file is `crates/agx/src/adjust/grain.md`. The Rust side at `crates/agx/src/adjust/grain.rs` reads:

```rust
#![doc = include_str!("grain.md")]
//!
//! See [`super::dehaze`] and [`super::denoise`] for related passes.

// ... rest of the file ...
```

The mdbook side at `docs/book/src/explanation/grain.md` reads:

```markdown
# Grain

{{#include ../../../../crates/agx/src/adjust/grain.md}}

## Related

- [Grain API reference](../api/agx/adjust/grain/index.html)
```

There is no anchor mechanism to maintain. The entire shared `.md` file is the explanation content. Editing it updates both surfaces on the next build with no possibility of drift.

The shared `.md` file should contain only prose, with no top-level heading and no cross-references. Reasons:

- Both consuming surfaces add their own outer heading. A heading inside the included file would render twice.
- Cross-references differ between the two surfaces. Rustdoc intra-doc syntax (`` [`super::dehaze`] ``) does not resolve in mdbook. Conversely, mdbook relative links into `../api/` do not resolve correctly when included in a rustdoc page. Keeping the shared file link-free avoids the cross-surface link problem.

Cross-references that need rustdoc validation live in inline `//!` lines on the Rust side, **outside** the include. Cross-references that need mdbook resolution live in the wrapping mdbook page, **outside** the include. The grain example demonstrates both patterns.

## Linking between surfaces

- **Rustdoc to mdbook:** use raw HTTPS URLs to the deployed site. Rustdoc has no intra-doc syntax for links to mdbook content.

  ```markdown
  See the [grain explanation](https://zhjngli.github.io/AgX/explanation/grain.html) on the project site.
  ```

- **Mdbook to rustdoc:** use relative links into `../api/`. The deploy workflow places rustdoc output at `_site/api/`, so this path is stable on the deployed site.

  ```markdown
  [Grain API reference](../api/agx/adjust/grain/index.html)
  ```

- **Mdbook to mdbook:** use mdbook's native relative-link resolution — relative paths from the file's location to the target file.

  ```markdown
  [Tutorials](../tutorials/index.md)
  ```

- **Rustdoc to rustdoc:** use intra-doc link syntax. The `deny(rustdoc::broken_intra_doc_links)` lint validates these.

  ```rust
  /// See [`crate::adjust::grain`] for the algorithm.
  ```

## Active lints

Both `crates/agx/` and `crates/agx-cli/` carry one crate-level rustdoc lint today:

- `#![deny(rustdoc::broken_intra_doc_links)]` — every broken intra-doc link in a `///` or `//!` comment fails `cargo doc`. There is no retrofit cost for this lint because the existing surface is link-free; new doc comments are validated as they are written.

A `missing_docs` lint is **not** active at the source level yet. The API doc retrofit sub-project will track and retire the missing-docs warning footprint crate-by-crate, and will introduce missing-docs enforcement at the end of that work — either as a source-level `#![warn(missing_docs)]` (once the warning footprint is zero) or via `RUSTDOCFLAGS` in `scripts/verify.sh`. Until then, contributors should still add `///` comments to every public item they touch, even though the linter is not enforcing it.

`scripts/verify.sh` runs `cargo doc --no-deps --workspace` with `RUSTDOCFLAGS="-D warnings"` so any rustdoc warning becomes a local verify failure. Today this enforces `broken_intra_doc_links` and any default rustdoc warnings; it will extend to `missing_docs` once the retrofit is ready.

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

Pushes to `main` trigger `.github/workflows/docs.yml`, which builds and deploys the site to `https://zhjngli.github.io/AgX/`. mdbook content lives at the root (e.g., `https://zhjngli.github.io/AgX/explanation/grain.html`); rustdoc content lives at `/api/` (e.g., `https://zhjngli.github.io/AgX/api/agx/adjust/grain/index.html`).
