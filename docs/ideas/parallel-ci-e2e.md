# Parallel CI E2E

Parallelize e2e test execution in GitHub Actions to reduce CI wall-clock time.

## Sub-tasks

- [ ] **Add build artifact job** — build `agx-cli --release` and upload the binary for matrix jobs to download
- [ ] **Fan out per-image tests** — use GitHub Actions matrix strategy to run each image test in parallel
- [ ] **Verify CI minute consumption** — free-tier GitHub Actions has 2000 min/month; parallel jobs use more total minutes

## Current State

The `e2e-tests` CI job runs all 9 test functions sequentially in a single job. Tests are already independent — 6 per-image matrix tests, 1 batch test, and 7 library pipeline tests.

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

Each matrix job would otherwise rebuild `agx-cli` redundantly. A prior job builds `agx-cli --release`, uploads the binary, and each matrix job downloads it. Saves ~6 redundant builds.

### Maintenance

The matrix list must stay in sync with the test functions in `cli_pipeline.rs`. Manual sync is fine given the low churn rate.

## Considerations

- Wall-clock time drops from sequential to limited by the slowest single image.
- More CI runner minutes consumed (parallel jobs overlap, each with setup overhead).
- Slightly more complex workflow YAML.

## Related

- [Multi-Preset CLI](multi-preset-cli.md) — reduces per-image decode calls (orthogonal)
- [Performance](performance.md) — render-level optimizations (orthogonal)
