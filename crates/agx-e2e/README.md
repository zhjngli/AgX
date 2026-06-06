# agx-e2e

End-to-end test suite for AgX. Tests the full pipeline from decode through engine processing to encode, using golden file comparison to catch regressions.

## Test Structure

### CLI Pipeline (`tests/cli_pipeline.rs`)

Data-driven matrix testing every image against every applicable look preset via CLI subprocess calls. Each image is its own test function, enabling Cargo to parallelize across images.

- **Color images** (8): noop + 12 curated looks = 13 goldens each
- **B&W images** (1): noop + 4 B&W looks = 5 goldens
- **Total**: 109 golden file comparisons + batch test + 2 error cases

### Library Pipeline (`tests/library_pipeline.rs`)

Slim API smoke tests (8 tests) covering: noop roundtrip (JPEG + RAW + HEIC), preset application, direct params, LUT loading, and preset `extends`.

### Golden Comparison

- JPEG: strict (tolerance=2, max_diff_pct=0.0) — deterministic across platforms
- RAW: permissive (tolerance=100, max_diff_pct=25.0) — LibRaw demosaicing varies across platforms; heavy looks (e.g. neo_noir) amplify differences
- HEIC: moderate (tolerance=10, max_diff_pct=1.0) — libheif/libde265 decode is mostly deterministic, but cross-platform version jitter surfaces at LUT-amplified boundary pixels
- Goldens downscaled to 1024px longest edge to keep repo size manageable
- Regenerate with: `GOLDEN_UPDATE=1 cargo test -p agx-e2e`

## Performance

The suite does heavy pixel processing (decode + render + encode for 109 image × look combinations). Key optimizations:

- **`[profile.test] opt-level = 2`** in workspace `Cargo.toml` — debug builds are ~14x slower for pixel math (37.7s vs 2.6s per JPEG measured). This applies to the test binary and its dependencies.
- **Release CLI binary** — `scripts/e2e.sh` builds `agx-cli` with `--release`. The test helper `cli_bin()` prefers the release binary at `target/release/agx`, falling back to debug.
- **Per-image test functions** — each image is a separate `#[test]` function so Cargo runs them in parallel across available cores (default = CPU count).

### Decode amortization

The harness uses the `multi-apply` subcommand so each image is decoded once and rendered against all presets in a single CLI invocation. For a RAW file this turns 12 separate LibRaw decode operations into 1 per test function.

## Running

```bash
# Full e2e suite (builds CLI in release mode)
./scripts/e2e.sh

# Just the tests (assumes CLI already built)
cargo test -p agx-e2e

# Regenerate golden files
GOLDEN_UPDATE=1 cargo test -p agx-e2e

# Regenerate the committed ICC-tagged binary fixtures (run any/all generators)
cargo test -p agx-e2e --test generate_fixtures -- --ignored \
  gen_adobe_rgb_gradient gen_prophoto_gradient gen_adobe_rgb_tiff
```

## Fixtures

| Directory | Contents |
|-----------|----------|
| `fixtures/jpeg/` | JPEG test images (incl. `adobe_rgb_gradient.jpg`, tagged with an embedded Adobe RGB ICC profile to exercise the input-ICC read path via the JPEG APP2 marker) |
| `fixtures/png/` | PNG test images. `prophoto_gradient.png` is tagged with an embedded ProPhoto RGB ICC profile (iCCP chunk), exercising PNG ICC extraction and the widest-gamut input conversion in the suite. |
| `fixtures/tiff/` | TIFF test images. `adobe_rgb_gradient.tiff` is tagged with an embedded Adobe RGB ICC profile (ICCProfile tag 0x8773), exercising TIFF ICC-tag extraction. |
| `fixtures/raw/` | RAF (Fujifilm RAW) test images |
| `fixtures/heic/` | HEIC test images. The four iPhone captures embed a Display P3 ICC profile (no nclx tag), so they decode via the input-ICC read path; `synthetic_p3_red.heic` carries nclx Display P3 primaries and exercises the matrix path instead. |
| `fixtures/looks/` | Preset TOML files (8 color + 4 B&W + 1 base) |
| `fixtures/looks/luts/` | Generated 33x33x33 .cube LUT files |
| `fixtures/golden/jpeg/` | JPEG golden reference images |
| `fixtures/golden/png/` | PNG golden reference images |
| `fixtures/golden/tiff/` | TIFF golden reference images |
| `fixtures/golden/raw/` | RAW golden reference images |
| `fixtures/golden/heic/` | HEIC golden reference images |
