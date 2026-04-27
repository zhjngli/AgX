# Sample Content Rework

Curate a stronger set of sample images for AgX and refresh everything that depends on them in one coordinated pass — `example/images/`, the e2e fixture goldens, the README hero images, and the tutorial / how-to embedded thumbnails.

The current sample images (`night_architecture.png`, `sunset_river.png`, `temple_blossoms.png`) work but are not curated for showcasing the tool. A stronger set would better represent AgX's capabilities and serve as the visual anchor across the README, the docs site, and the e2e suite.

## Sub-tasks

- [ ] **Select replacement / additional sample images.** Aim for tonal range diversity: high-contrast scene, deep-shadow scene, soft-light scene, vivid-colour scene. Pull from a permissive licence source (Unsplash or similar) or shoot/curate originals. Document attribution.
- [ ] **Replace `example/images/`.** Swap files, keep `snake_case.{png,jpg}` naming convention.
- [ ] **Regenerate e2e fixture goldens** (`crates/agx-e2e/fixtures/golden/`) so the e2e suite continues to pass against the new sample inputs. Use `GOLDEN_UPDATE=1 cargo test -p agx-e2e`.
- [ ] **Update README hero images.** The Sample Images table in `README.md` references specific filenames and pairings — update.
- [ ] **Regenerate tutorial / how-to thumbnails** under `docs/book/src/assets/tutorials/` and `docs/book/src/assets/how-to/` against the new images.
- [ ] **Update `example/README.md`** to describe the new image set.

## Considerations

- The cascade is intentional: e2e goldens, README, and docs all reference `example/images/` and need to update together to avoid broken renders or stale documentation. A single PR for the whole rework is the cleanest path.
- Preserve the existing preset filenames (`golden-hour.toml`, `moody-dark.toml`, etc.) unless those are also being curated. The presets are independent from the images — only their pairing changes.
- Out of scope: changing the `example/presets/` set itself, changing the e2e look set under `crates/agx-e2e/fixtures/looks/`. Those are separate decisions.

## Related

- [`example/README.md`](../../example/README.md) — current sample content manifest.
- [Documentation Initiative](documentation-initiative.md) — sub-projects 6 and 7 ship the first versions of the tutorial / how-to thumbnails; this rework regenerates them against new inputs.
