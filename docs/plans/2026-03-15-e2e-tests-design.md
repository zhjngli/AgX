# E2E Test Suite Design

**Goal:** Add a heavyweight end-to-end test suite that processes real RAW and JPEG files through both the library API and CLI, compares output against committed golden reference files, and runs automatically in CI on PRs.

## Motivation

The existing test suite uses synthetic 4x4 and 64x64 PNG images. These catch API-level regressions but don't exercise the real decode-process-encode pipeline with actual camera files. The e2e suite provides assurance that:

- Real RAF and JPEG files process without errors
- Pipeline output is visually reasonable (sanity checks on pixel values)
- New features don't silently break existing processing (golden file comparison)

## Architecture

### New workspace crate: `crates/agx-e2e/`

A dedicated test crate, separate from the fast unit/integration tests.

```
crates/agx-e2e/
├── Cargo.toml
├── src/
│   └── lib.rs              # Shared test utilities (comparison, fixture paths)
├── tests/
│   ├── library_pipeline.rs # Library API e2e tests
│   └── cli_pipeline.rs     # CLI binary e2e tests
└── fixtures/
    ├── raw/                # 3-4 small Fuji RAF files (various lighting)
    ├── jpeg/               # 2-3 JPEG files
    ├── presets/            # Preset TOML files for test scenarios
    └── golden/             # Reference output images (PNG format)
```

**Dependencies (all `[dev-dependencies]` since this is a test-only crate):**
- `agx = { path = "../agx", features = ["raw"] }` (library under test, with raw support)
- `agx-cli = { path = "../agx-cli" }` (so `env!("CARGO_BIN_EXE_agx-cli")` resolves the CLI binary)
- `image` (pixel-level comparison)
- `tempfile` (output directories)

### Test fixtures (committed to repo)

- 3-4 small Fuji RAF files covering different lighting (daylight, indoor, high contrast, low light)
- 2-3 JPEG files for the non-raw path
- A few preset TOML files exercising different adjustment combinations (exposure, white balance, HSL, contrast)
- Target: ~50-100MB total fixture data

### Golden file comparison

Each test processes a fixture file with specific parameters, then compares the output against a committed golden reference image.

**Golden file format:** All golden files are PNG (lossless), regardless of what the CLI would normally emit. This ensures deterministic comparison across platforms.

**Comparison method:** Pixel-by-pixel comparison with a per-channel tolerance (e.g., max difference of 2 per channel per pixel). This absorbs platform-level floating point differences without masking real regressions.

**Regenerating goldens:** Set `GOLDEN_UPDATE=1` environment variable to write new golden files instead of comparing. Workflow:
1. Make an intentional pipeline change
2. Run `GOLDEN_UPDATE=1 cargo test -p agx-e2e`
3. Review the golden diffs visually
4. Commit the updated goldens

**Comparison utility** in `src/lib.rs`:
- `compare_images(actual: &Path, golden: &Path, tolerance: u8) -> Result<(), ComparisonError>`
- `ComparisonError` reports: number of differing pixels, max channel difference, percentage of pixels that differ
- `fixture_path(relative: &str) -> PathBuf` — resolves paths relative to the fixtures directory
- `golden_path(name: &str) -> PathBuf` — resolves paths relative to the golden directory

### Test categories

**1. Library pipeline tests** (`tests/library_pipeline.rs`)

Test the core `agx` library API directly: decode → Engine → set params → render → encode.

Test cases:
- Process each RAF fixture with default parameters (neutral processing)
- Process each JPEG fixture with default parameters
- Apply exposure adjustment (+1 stop) to a RAF
- Apply warm white balance to a RAF
- Apply a preset to a RAF
- Apply HSL adjustments to a JPEG

Each test:
- Asserts output dimensions match input
- Asserts output file size > 0
- Asserts directional sanity (e.g., +1 exposure → higher average brightness)
- Compares against golden file

**2. CLI pipeline tests** (`tests/cli_pipeline.rs`)

Test the `agx` CLI binary with real files via `std::process::Command`, using `env!("CARGO_BIN_EXE_agx-cli")` to locate the binary (enabled by the `agx-cli` dev-dependency).

Test cases:
- `agx edit -i <raf> --exposure 1.0 -o <out>` — basic RAW edit
- `agx apply -i <jpeg> -p <preset> -o <out>` — preset application to JPEG
- `agx batch-edit --input-dir <dir> --output-dir <dir>` — batch processing of mixed RAW/JPEG directory
- `agx edit -i <corrupt-file> -o <out>` — error case: corrupt/unsupported file should fail gracefully

Each test:
- Asserts CLI exits with expected status (0 for success, non-zero for error cases)
- Asserts output file exists with expected format (for success cases)
- Compares against golden file (for success cases)

## Scripts

### `scripts/e2e.sh`

Runs the full e2e suite. Separate from `verify.sh` to keep the fast path fast.

```bash
#!/bin/bash
set -euo pipefail
echo "=== E2E Tests (cargo test -p agx-e2e) ==="
cargo test -p agx-e2e
echo "E2E PASSED"
```

`verify.sh` remains unchanged (fast unit + integration + lint).

## CI (GitHub Actions)

### `.github/workflows/ci.yml`

Runs on pull requests to main:

1. **Fast checks job:**
   - `cargo fmt --check`
   - `cargo clippy -p agx -p agx-cli -- -D warnings`
   - `cargo test -p agx`
   - `cargo test -p agx-cli`

2. **E2E tests job:**
   - `apt-get install libraw-dev`
   - `cargo test -p agx-e2e`

Both jobs run in parallel. PRs require both to pass.

## Local workflow

- `cargo test` — fast unit + integration (no e2e)
- `./scripts/verify.sh` — fast verification (format, lint, unit, integration, doc links)
- `./scripts/e2e.sh` — heavy e2e suite (run before merging or after completing a feature)
- `GOLDEN_UPDATE=1 cargo test -p agx-e2e` — regenerate golden files after intentional pipeline changes

## Documentation

Update `ARCHITECTURE.md` to include `agx-e2e` as a workspace member (test-only crate, not part of the library/CLI dependency graph).

## Growth pattern

When a new feature is added (e.g., tone curves, sharpening), add a corresponding e2e test case:
1. Add a test that applies the new feature to a fixture file
2. Generate and commit the golden output
3. Add sanity checks appropriate to the feature
