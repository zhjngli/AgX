# Sample Content Rework Design

## Problem

The current sample image set was assembled ad-hoc before AgX's HEIC support and wide-gamut working space landed. After sub-project 1 (linear Rec.2020 working space) and the HEIC decode path, the sample set is out of step with what the tool can do:

- `example/images/` is three sRGB-baked PNGs (`night_architecture`, `sunset_river`, `temple_blossoms`) from a single Fuji session. None of it exercises HEIC decode or wide-gamut working space.
- `crates/agx-e2e/fixtures/` is four Fuji RAFs (all dusk / night), two Fuji JPEGs, and two HEICs — of which one is the `synthetic_p3_red.heic` test artifact and the other reuses the `temple_blossoms` scene. No iPhone source, no daylight, no Display P3 native content.
- The current set leans Fuji-only and moody-only. Tonal-range diversity and capability showcase are weak.
- `README.md`, `example/README.md`, and the doc-book tutorial / how-to pages all reference these filenames.

Backlog entry: [`docs/backlog/sample-content-rework.md`](../backlog/sample-content-rework.md). This design doc implements that backlog item.

## Goals

- Curate a coherent set of 13 sample scenes (8 hero + 5 extras) spanning AgX's capability categories.
- Use **native source formats** (HEIC, RAF, JPEG) — not sRGB-baked PNG — so the showcase exercises HEIC decode and the wide-gamut working space directly.
- Pull from the maintainer's own photo library. Zero licensing overhead, full curation control.
- Cascade through every dependent surface in one coordinated PR: `example/images/`, e2e fixtures, e2e goldens, `README.md`, `example/README.md`, doc-book pages, `example/outputs/`.

## Non-goals

- Stock photo sourcing (Unsplash etc.). Own library covers all categories.
- Curating the `example/presets/` set. Independent decision.
- Curating the e2e look set (`crates/agx-e2e/fixtures/looks/`). Out of scope.
- Adding new editing features or pipeline behavior. This is content curation only.
- Pre-converting any source to sRGB PNG for storage in `example/images/`. Defeats the wide-gamut showcase intent.

## Capability categories

Eight categories drive scene selection. Each maps to a distinct AgX strength:

| Category | What it shows |
|----------|---------------|
| Vivid color sunset, Display P3 | Saturation, wide-gamut working space, color grading on iPhone-native content |
| Wide DR / scenic landscape | Highlight recovery, shadow lift, dehaze, atmospheric perspective |
| Bright daylight + texture | Sharpening, detail enhancement, denoise on non-moody content |
| Color science / complementary | HSL, color grading, split-toning |
| Moody / deep shadow | Shadow recovery, tone curves on extreme DR |
| Minimal pattern / gradient | Pure color rendering, gentle curves, P3 sky gradient |
| B&W source | Mono-input pipeline coverage in e2e + showcase B&W → B&W output |
| Soft golden hour / mid-key | Faded film looks, gentle tone curves, atmospheric color |

## Sample pool (13 scenes)

Stored as the committed pool in `example/images/` in native formats. A subset surfaces in `crates/agx-e2e/fixtures/` for test coverage.

### Hero 8 (referenced in README and doc-book pages)

| # | Filename | Source | Category |
|---|----------|--------|----------|
| 1 | `marina_sunset.heic` | iPhone (Display P3) | Vivid color sunset, P3 showcase |
| 2 | `grand_canyon_overlook.raf` | Fuji X-T20 RAW | Wide DR scenic, dehaze demo |
| 3 | `cinque_terre_manarola.raf` | Fuji X-T20 RAW | Bright daylight + texture, vivid colors |
| 4 | `concert_hall.heic` | iPhone (Display P3) | Color science: warm wood + teal spotlight |
| 5 | `mountain_valley.heic` | iPhone (Display P3) | Moody deep shadow, near-monochrome |
| 6 | `sky_moon_wires.heic` | iPhone (Display P3) | Minimal pattern, P3 sky gradient |
| 7 | `geisel_library_bw.jpg` | Fuji X-T20 JPEG (mono) | B&W source |
| 8 | `cinque_terre_window.jpg` | Pixel JPEG | Soft golden hour, mid-key |

### Extras 5 (committed, available for swap-in but not headlined)

| # | Filename | Source | Notes |
|---|----------|--------|-------|
| 9 | `film_beach.jpg` | Film scan JPEG | Unique film aesthetic, very soft tones |
| 10 | `ranunculus_field.heic` | iPhone (Display P3) | Alt vivid floral |
| 11 | `stairwell_silhouette.heic` | iPhone (Display P3) | Extreme DR torture test |
| 12 | `foggy_sintra.heic` | iPhone (Display P3) | Atmospheric fog (alt dehaze demo) |
| 13 | `grand_canyon_rainbow.raf` | Fuji X-T20 RAW | Alt Grand Canyon, rainbow + soft pastel sky |

## Storage layout

```
example/images/                              # 13 originals, native formats
example/outputs/                             # pre-rendered PNG demos (existing pattern)

crates/agx-e2e/fixtures/raw/
  cinque_terre_manarola.raf
  grand_canyon_overlook.raf
crates/agx-e2e/fixtures/jpeg/
  geisel_library_bw.jpg
  cinque_terre_window.jpg
crates/agx-e2e/fixtures/heic/
  marina_sunset.heic
  concert_hall.heic
  mountain_valley.heic
  sky_moon_wires.heic
  synthetic_p3_red.heic                      # keep (existing decoder OOG sanity check)
```

E2e subset is 9 source files. The same files are duplicated into `crates/agx-e2e/fixtures/` so the test crate stays self-contained (no cross-crate file references at test time).

## Naming convention

`snake_case.{native_ext}`. Names describe subject, not camera model or capture date.

Camera filenames (e.g. `IMG_4032.HEIC`, `ZLIV2641.RAF`, `PXL_20220525_190217760.jpg`) are renamed when moved into the repo.

## B&W treatment

Both directions covered:

- **B&W source → e2e mono-input coverage.** `geisel_library_bw.jpg` is a Fuji in-camera-mono JPEG. Sits in `crates/agx-e2e/fixtures/jpeg/`. Exercises grayscale-source handling in the e2e matrix.
- **Color → B&W via preset.** `example/outputs/` includes B&W-rendered demos of at least one or two color scenes — continuing the existing pattern of `night_architecture_bw_high_contrast.png`. Strongest candidates: `mountain_valley.heic` (near-monochrome already) and `stairwell_silhouette.heic` (silhouette + harsh contrast).

## Format coverage in e2e

| Format | Count | What it tests |
|--------|-------|----------------|
| RAW (`.raf`) | 2 | LibRaw demosaic → Rec.2020 working space. One daylight + one scenic. |
| JPEG (`.jpg`) | 2 | sRGB JPEG decode. One color + one mono. |
| HEIC Display P3 (`.heic`) | 4 | Display P3 → Rec.2020 wide-gamut path across diverse scenes |
| HEIC synthetic (`.heic`) | 1 | `synthetic_p3_red.heic` — decoder out-of-sRGB-gamut sanity check |

HEIC weighting is intentional. The Display P3 → Rec.2020 path is AgX's newest decode capability and deserves the most fixture coverage. RAW and JPEG paths are mature; two each is enough.

## Cascade — surfaces touched

Single coordinated PR. All of these update together so no surface ends up referencing a deleted filename. Stale golden files (named `<old-scene>_<look>.png`) are removed as part of regenerating the golden directory.

### Content and documentation

- `example/images/` — replace 3 PNGs with 13 native-format originals.
- `example/README.md` — full rewrite of Images and Quick start sections.
- `example/outputs/` — regenerate pre-rendered PNG demos, including 1–2 B&W renderings.
- `crates/agx-e2e/fixtures/raw/` — drop all 4 current RAFs, add 2 new.
- `crates/agx-e2e/fixtures/jpeg/` — drop both current, add 2 new.
- `crates/agx-e2e/fixtures/heic/` — drop `temple_blossoms.heic`, add 4 new (keep `synthetic_p3_red.heic`).
- `crates/agx-e2e/fixtures/golden/` — delete all stale goldens, regenerate against new fixtures via `GOLDEN_UPDATE=1 cargo test -p agx-e2e`.
- `README.md` — update Sample Images table (filenames, descriptions, preset pairings).
- `docs/book/src/introduction.md` — sample filename references in image embeds.
- `docs/book/src/images/` — replace the 3 PNG copies that `introduction.md` embeds (this is a separate set from `example/images/`, scoped to the book build).
- `docs/book/src/tutorials/getting-started.md` — `agx apply` example commands.
- `docs/book/src/how-to/{write-preset,extend-preset,compose-looks,multi-apply,custom-lut}.md` — filename refs in commands and prose.
- `docs/book/src/assets/tutorials/` + `docs/book/src/assets/how-to/` — regenerate embedded thumbnails (before/after, multi-apply grid, batch-apply grid) against new sources.
- `docs/backlog/sample-content-rework.md` — check off sub-tasks, then delete the file per `docs/backlog/README.md` convention.

Architecture-level docs (`ARCHITECTURE.md`, per-module `README.md` files) need no updates. No module contracts, dependencies, or invariants change.

### Code, tests, CI, scripts

Filename changes ripple into hardcoded references in test code, CI matrix, and helper scripts. These must update in the same PR or the build breaks.

- `crates/agx-e2e/tests/cli_pipeline.rs` — rename per-scene test functions (`cli_temple_blossoms`, `cli_sunset_river`, `cli_foggy_forest`, `cli_dusk_cityscape`, `cli_night_city_blur`, `cli_night_architecture`, `cli_temple_blossoms_heic`) to match the new fixture set. Update the hardcoded fixture path and the scene identifier (golden filename prefix) inside each.
- `crates/agx-e2e/tests/library_pipeline.rs` — hardcoded `fixture_path("jpeg/temple_blossoms.jpg")`, `fixture_path("raw/night_city_blur.raf")`, `fixture_path("heic/temple_blossoms.heic")` calls. Repoint to surviving fixtures.
- `crates/agx-cli/tests/validate.rs` — `apply_with_unknown_field_prints_warning_to_stderr` references `example/images/sunset_river.png` to copy as a real-image test input. Repoint to a new sample (`cinque_terre_window.jpg` is the simplest substitute — sRGB JPEG, no HEIC decode dependency for the cli crate's test). Adjust the temp-file extension to match.
- `scripts/e2e-quick.sh` — hardcoded `cli_temple_blossoms` test name in the JPEG-matrix smoke. Repoint to the new JPEG test name (likely `cli_cinque_terre_window` or `cli_geisel_library_bw`).
- `scripts/profile.sh` — hardcoded fixture paths `sunset_river.raf`, `dusk_cityscape.raf`, `foggy_forest.raf`, `temple_blossoms.jpg`, plus the golden filename `golden/raw/sunset_river_noop.png`. Repoint to surviving fixtures and golden names.
- `.github/workflows/ci.yml` — the per-scene matrix in the e2e job lists test function names (`cli_temple_blossoms`, `cli_night_city_blur`, `cli_sunset_river`, `cli_foggy_forest`, `cli_dusk_cityscape`, `cli_night_architecture`). Update the matrix to the new test names so each surviving fixture stays in the per-scene CI shard.

## Trade-offs

**Repo size.** Native formats are bigger than sRGB PNG:

- 3 RAFs × ~30 MB ≈ 90 MB
- 5 HEICs × ~4 MB ≈ 20 MB
- 4 JPEGs × ~10 MB ≈ 40 MB
- E2e subset duplication ≈ 80 MB

Estimated total committed: ~200 MB of image assets. The dominant new cost. One-time addition — the sample set is static once curated; no churn-driven history bloat.

Considered and rejected: pre-converting HEICs to sRGB PNG. Smaller, but defeats the showcase intent — would hide the Display P3 → Rec.2020 capability the rework is partly motivated by.

**Golden count.** 9 e2e source files × ~8 looks ≈ 72 goldens. Slightly above the current 65. Acceptable; well under the threshold where the e2e suite starts feeling slow.

**B&W source overlap.** The B&W JPEG `geisel_library_bw.jpg` is Fuji in-camera mono. The decoder has no special mono path — it reads as sRGB JPEG with R=G=B values. So it tests no new code path. Kept anyway because B&W → preset output is part of the showcase story (covers real user workflow), and a visually distinct fixture gives golden comparison a sanity-check signal.

**Pixel JPEG as a source.** `cinque_terre_window.jpg` is a Pixel phone JPEG. Standard sRGB JPEG decode — no different from Fuji JPEG decode. Kept as content variety, not as a new code-path probe.

**Two Grand Canyon raws.** `grand_canyon_overlook.raf` and `grand_canyon_rainbow.raf` are the same general scene but cover different demos: the first has visible atmospheric haze for dehaze testing, the second has a sky-spanning rainbow for color preservation. Keeping both costs ~30 MB extra. Worth it.

## Documentation and code-surface updates

Per CLAUDE.md, cross-cutting changes enumerate updates as a checklist. Adversarial review at end of implementation can verify against this.

### Docs and content

- [ ] `example/README.md` — full rewrite of Images and Quick start sections
- [ ] `README.md` — Sample Images table (filenames, descriptions, preset pairings)
- [ ] `docs/book/src/introduction.md` — sample filename references in image embeds
- [ ] `docs/book/src/images/` — replace 3 PNG copies embedded by `introduction.md`
- [ ] `docs/book/src/tutorials/getting-started.md` — `agx apply` example commands
- [ ] `docs/book/src/how-to/write-preset.md` — input file refs
- [ ] `docs/book/src/how-to/extend-preset.md` — input file refs
- [ ] `docs/book/src/how-to/compose-looks.md` — input file refs
- [ ] `docs/book/src/how-to/multi-apply.md` — input file refs
- [ ] `docs/book/src/how-to/custom-lut.md` — input file refs
- [ ] `docs/book/src/assets/tutorials/*.jpg` — regenerate against new images
- [ ] `docs/book/src/assets/how-to/*.jpg` — regenerate against new images
- [ ] `crates/agx-e2e/fixtures/golden/*` — delete stale, regenerate via `GOLDEN_UPDATE=1 cargo test -p agx-e2e`
- [ ] `docs/backlog/sample-content-rework.md` — check off + delete when complete

### Code, tests, CI, scripts

- [ ] `crates/agx-e2e/tests/cli_pipeline.rs` — rename per-scene test functions, update fixture paths and golden prefixes
- [ ] `crates/agx-e2e/tests/library_pipeline.rs` — repoint hardcoded `fixture_path(...)` calls
- [ ] `crates/agx-cli/tests/validate.rs` — repoint `example/images/sunset_river.png` reference to a new sample
- [ ] `scripts/e2e-quick.sh` — repoint `cli_temple_blossoms` test name
- [ ] `scripts/profile.sh` — repoint hardcoded fixture paths and golden filename
- [ ] `.github/workflows/ci.yml` — update e2e per-scene CI matrix to new test names

## Verification

- `./scripts/verify.sh` passes — `markdown-lint`, `book-linkcheck`, `back-links`, `sibling-md-clean`, `book-no-internal-refs`, `doc-links` all green
- `./scripts/e2e-quick.sh` passes — fast smoke against new fixtures
- `./scripts/e2e.sh` passes — full golden comparison
- Visual sanity: hero scenes look right when rendered with at least one preset each
- README rendered locally — Sample Images table image previews resolve

## Related

- [`docs/backlog/sample-content-rework.md`](../backlog/sample-content-rework.md) — backlog item this implements
- [`docs/plans/2026-05-16-wide-working-space-design.md`](2026-05-16-wide-working-space-design.md) — sub-project 1, prerequisite (delivered the Display P3 → Rec.2020 decode this rework now showcases)
- [`docs/plans/2026-05-13-heic-support-design.md`](2026-05-13-heic-support-design.md) — HEIC support design (provides the format this rework leans on for showcase content)
