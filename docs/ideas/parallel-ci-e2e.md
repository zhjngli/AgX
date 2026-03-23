# Parallel E2E Tests in CI

Parallelize e2e test execution in GitHub Actions to reduce CI wall-clock time.

## Current State

The `e2e-tests` CI job runs all 9 test functions sequentially in a single job (~5 minutes). Tests are already independent — 6 per-image matrix tests, 1 batch/error test, and 7 library pipeline tests.

## Proposal

Use GitHub Actions matrix strategy to fan out per-image tests into parallel jobs. Each job runs `cargo test -p agx-e2e -- <test_name>`.

```yaml
e2e-tests:
  strategy:
    fail-fast: false
    matrix:
      test:
        - cli_temple_blossoms
        - cli_night_city_blur
        - cli_sunset_river
        - cli_foggy_forest
        - cli_dusk_cityscape
        - cli_night_architecture
        - remaining  # batch, error cases, library tests
```

### Build artifact sharing

Each matrix job would otherwise rebuild `agx-cli` redundantly. Two options:

1. **Upload/download artifact** — a prior job builds `agx-cli --release`, uploads the binary, and each matrix job downloads it. Saves ~6 redundant builds but adds artifact transfer overhead.
2. **Rely on `rust-cache`** — with a shared cache key, each job hits the cache and only links. Simpler but still has some redundant work.

Option 1 is cleaner. The build job already exists conceptually (`fast-checks` compiles everything for clippy).

### Maintenance

The matrix list must stay in sync with the test functions in `cli_pipeline.rs`. If a new image is added to the e2e suite, a matrix entry must be added to the CI workflow. This could be automated with a script that extracts test names, but manual sync is fine given the low churn rate.

## Expected Impact

Wall-clock time for e2e CI drops from ~5 min (sequential) to ~1-2 min (limited by the slowest single image, likely `temple_blossoms` with 23 looks).

## Trade-offs

- More CI runner minutes consumed (parallel jobs overlap, each with setup overhead). Free-tier GitHub Actions has 2000 min/month — need to check consumption.
- Slightly more complex workflow YAML.
- Matrix entries must stay in sync with test code.
