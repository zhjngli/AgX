# Parallel CI E2E

Parallelize e2e test execution in GitHub Actions to reduce CI wall-clock time.

## Sub-tasks

- [ ] **Add build artifact job** — build `agx-cli --release` once and upload the binary for matrix jobs to download. Each matrix job currently rebuilds the CLI.
- [x] **Fan out per-image tests** — `e2e-tests` job in `.github/workflows/ci.yml` uses a matrix strategy with one job per image (`cli_temple_blossoms`, `cli_night_city_blur`, etc.) plus a `misc` rollup for batch / error cases / library tests. Wall-clock time now bounded by the slowest single image, not the sum of all images.
- [x] **Verify CI minute consumption** — implicit via running the parallel matrix on every PR. Free-tier limits not yet hit.

## Current State

Matrix fan-out is live. The remaining win is build-artifact sharing: today each matrix job runs `cargo build --release -p agx-cli` independently, so the CLI is rebuilt 7x per CI run. A prior job could build once and upload the binary for the matrix to download.

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

- [Performance](performance.md) — render-level optimizations (orthogonal)
