# Documentation Initiative

The documentation initiative — published mdbook site, rustdoc API reference, auto-generated CLI/preset reference, algorithm explanations co-located with source, tutorials, and how-to guides — has shipped. Sub-projects #1–#8, #10, and #11 are complete; their dated design docs in `docs/plans/` and the git history hold the full record of what was built and why.

**Umbrella design doc:** [Documentation Initiative Design](../plans/2026-04-06-documentation-initiative-design.md)

What remains is optional polish — picked up only if the site feels inadequate once the content has settled.

## Sub-tasks

- [ ] **Custom theme / branding** *(optional)* — custom mdbook theme, logo, landing-page styling, OG image. Only if the default theme feels inadequate after content lands.
- [ ] **Markdown lint tightening** *(optional, follow-up from the markdownlint integration)* — `MD040` retrofit (language tags on fenced code blocks) and table-style normalization across older design docs. Quieter follow-up to the `markdownlint-cli2` gate already wired into `scripts/verify.sh`.

## Related

- [Documentation Initiative Design](../plans/2026-04-06-documentation-initiative-design.md) — umbrella design doc; full sub-project record
- [Processing Parity](processing-parity.md) — the shipped algorithm explanations aid comparison against reference editors
