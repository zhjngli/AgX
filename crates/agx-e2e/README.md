# agx-e2e

End-to-end test suite for AgX. Tests the full pipeline from decode through engine processing to encode, using golden file comparison to catch regressions.

## Test Structure

### CLI Pipeline (`tests/cli_pipeline.rs`)

Data-driven matrix testing every image against every applicable look preset via CLI subprocess calls. Each image is its own test function, enabling Cargo to parallelize across images.

- **Color images** (5): noop + 6 color looks + 3 B&W looks = 10 goldens each
- **B&W images** (1): noop + 3 B&W looks = 4 goldens
- **Total**: 54 golden file comparisons + batch test + 2 error cases

### Library Pipeline (`tests/library_pipeline.rs`)

Slim API smoke tests (6 tests) covering: noop roundtrip (JPEG + RAW), preset application, direct params, LUT loading, and preset `extends`.

### Golden Comparison

- JPEG: strict (tolerance=2, max_diff_pct=0.0) — deterministic across platforms
- RAW: permissive (tolerance=30, max_diff_pct=10.0) — LibRaw output varies across platforms
- Goldens downscaled to 1024px longest edge to keep repo size manageable
- Regenerate with: `GOLDEN_UPDATE=1 cargo test -p agx-e2e`

## Performance

The suite does heavy pixel processing (decode + render + encode for 54 images). Key optimizations:

- **`[profile.test] opt-level = 2`** in workspace `Cargo.toml` — debug builds are ~14x slower for pixel math (37.7s vs 2.6s per JPEG measured). This applies to the test binary and its dependencies.
- **Release CLI binary** — `scripts/e2e.sh` builds `agx-cli` with `--release`. The test helper `cli_bin()` prefers the release binary at `target/release/agx-cli`, falling back to debug.
- **Per-image test functions** — each image is a separate `#[test]` function so Cargo runs them in parallel across available cores (default = CPU count).

### Known bottleneck

Each CLI subprocess call independently decodes the image, even when the same image is processed with multiple presets. For a RAW file, this means 10 separate LibRaw decode operations per image. A `--multi-preset` CLI flag (see `docs/ideas/multi-preset-cli.md`) would decode once and apply N presets per invocation, reducing RAW decode calls from 50 to 5.

## Running

```bash
# Full e2e suite (builds CLI in release mode)
./scripts/e2e.sh

# Just the tests (assumes CLI already built)
cargo test -p agx-e2e

# Regenerate golden files
GOLDEN_UPDATE=1 cargo test -p agx-e2e
```

## Fixtures

| Directory | Contents |
|-----------|----------|
| `fixtures/jpeg/` | JPEG test images |
| `fixtures/raw/` | RAF (Fujifilm RAW) test images |
| `fixtures/looks/` | Preset TOML files (6 color + 3 B&W + 1 base) |
| `fixtures/looks/luts/` | Generated 33x33x33 .cube LUT files |
| `fixtures/golden/jpeg/` | JPEG golden reference images |
| `fixtures/golden/raw/` | RAW golden reference images |
