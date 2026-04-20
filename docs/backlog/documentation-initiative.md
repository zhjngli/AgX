# Documentation Initiative

Umbrella epic tracking the multi-sub-project effort to build a coherent documentation system for AgX: published mdbook site, rustdoc as API reference, CLI and preset reference auto-generated from code, algorithm explanations co-located with source.

**Design doc:** [Documentation Initiative Design](../plans/2026-04-06-documentation-initiative-design.md)

See the design doc for problem statement, goals, Diataxis quadrant mapping, tooling architecture, enforcement strategy, and success criteria. The table below tracks sub-project status; each sub-project gets its own dated design doc in `docs/plans/` before implementation.

## Sub-tasks

- [x] **#1 Docs infrastructure & scaffolding** — mdbook skeleton, GitHub Actions deploy to `gh-pages`, `warn(missing_docs)` + `deny(broken_intra_doc_links)`, linkcheck/mermaid/katex preprocessors, doc conventions under `docs/contributing/`. Design: [Docs Infrastructure](../plans/2026-04-06-docs-infrastructure-design.md)
- [x] **#2 API doc retrofit** — `///` comments on every `pub` item in `agx` and `agx-cli`; flipped `warn(missing_docs)` → `deny(missing_docs)` on both crates. Design: [API Doc Retrofit](../plans/2026-04-09-api-doc-retrofit-design.md)
- [x] **#3 Auto-generated reference (`agx-docgen`)** — dev-only crate producing CLI reference via `clap-markdown` and preset reference via `schemars` + custom renderer; wired into mdbook build with drift check. Design: [agx-docgen](../plans/2026-04-11-agx-docgen-design.md)
- [ ] **#4 Algorithm explanations** — sibling `.md` next to each `crates/agx/src/adjust/*.rs` (dehaze, denoise, detail, tone-curves, color-grading, vignette, per-pixel; grain already done). `#![doc = include_str!("module.md")]` on Rust side, `{{#include ...}}` on mdbook side. Each page notes dual CPU/GPU implementation with source links. Includes `crates/agx/src/engine/gpu/README.md` dual-path contributor guide. Blocked on design doc.
- [ ] **#5 Conceptual reference refresh** — move `docs/reference/{color-spaces,grain-algorithm,lut-format}.md` into `docs/book/src/reference/concepts/`; expand with photographic terminology, preset compositional model, render pipeline conceptual overview, image processing basics. Blocked on design doc.
- [ ] **#6 Tutorials** — install guide; "Edit your first photo with the CLI"; "Apply a look to a whole directory with `batch-apply`"; "Compare multiple looks on one image with `multi-apply`". Uses existing `example/images/` and e2e presets. Blocked on design doc.
- [ ] **#7 How-to guides** — "Create your own preset from scratch"; "Extend an existing preset"; "Write and load a custom `.cube` LUT"; "Compose layered looks"; "Match the output of another preset". Blocked on design doc.
- [ ] **#8 Prose explanations** — architecture overview, preset-first philosophy, render pipeline conceptual overview, design decisions and trade-offs. `README.md` and `ARCHITECTURE.md` retain short summaries plus pointers into `docs/book/src/`. Blocked on design doc.
- [ ] **#9 Polish / theme** *(optional)* — custom mdbook theme, logo, landing page styling, OG image. Only if default theme feels inadequate after content lands.

## Considerations

- Sub-projects #4–#8 can run in parallel on separate branches; #1–#3 are complete, so no ordering blockers remain among the content sub-projects.
- Each sub-project gets its own brainstorm → spec → plan → implementation cycle. Write a dated design doc in `docs/plans/` before starting implementation.
- Per the design doc's "Repo discoverability" constraint, all site content must stay as plain markdown files in `docs/book/src/` — agents in the repo need to find docs via file search, not network.
- The "oxiraw" → "AgX" rename in `docs/reference/*.md` happened during #1. The larger move into `docs/book/src/reference/concepts/` is part of #5.

## Related

- [Documentation Initiative Design](../plans/2026-04-06-documentation-initiative-design.md) — umbrella design doc
- [Processing Parity](processing-parity.md) — algorithm docs help compare implementations against reference editors
