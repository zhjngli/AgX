# Multi-Apply & E2E Speed Design

**Date:** 2026-04-05

## Problem

The e2e test suite takes ~3 minutes because each image×preset combination spawns a separate CLI process, re-decoding the source image every time. For 4 RAW images × 12 invocations each, that's ~100s of redundant RAW decoding (~2.5s per decode). This same inefficiency would affect any user wanting to compare multiple presets on a single image.

## Solution

Add a `multi-apply` CLI command that decodes an image once and renders it with multiple presets, optionally in parallel.

**Naming:** `multi-apply` (not `batch-apply`) because the existing `batch-apply` command applies one preset to a directory of images. `multi-apply` applies multiple presets to one image — the inverse operation.

## `multi-apply` CLI Command

### Usage

```bash
agx-cli multi-apply -i <image> -p <preset1> -p <preset2> ... -o <output_dir> [--noop] [--jobs N]
```

### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `-i <image>` | yes | Input image path (JPEG, PNG, or RAW) |
| `-p <preset>` | yes (1+) | Preset TOML file(s). Can be specified multiple times. |
| `-o <output_dir>` | yes | Output directory. Created if it doesn't exist. |
| `--noop` | no | Also render a no-preset (identity) output. |
| `--jobs N` | no | Number of preset renders to run concurrently. Default: 1 (sequential). |

### Output naming

Output files are auto-named from the input image stem and preset stem. Output format is PNG (consistent with the e2e golden format and lossless for comparison):

```
<output_dir>/<image_stem>_<preset_stem>.png
<output_dir>/<image_stem>_noop.png          (if --noop)
```

Example:

```bash
agx-cli multi-apply -i sunset_river.raf -p portra_400.toml -p neo_noir.toml -o out/ --noop
# Produces:
#   out/sunset_river_portra_400.png
#   out/sunset_river_neo_noir.png
#   out/sunset_river_noop.png
```

### Implementation

1. Decode input image once → `Rgb32FImage`
2. If `--noop`: render with default params, encode, drop engine
3. For each preset (sequentially or `--jobs` at a time):
   - Clone the decoded image
   - `Engine::new(image_clone)` → `apply_preset()` → `render()` → encode to output
   - Drop engine (frees the clone)
4. With `--jobs N > 1`: use a thread pool (rayon scope or similar) to render up to N presets concurrently

No new library API needed. `Engine::new()`, `apply_preset()`, and `render()` already exist. The optimization is purely avoiding redundant `decode()` calls and enabling concurrent renders.

### Parallelism and `--jobs`

`--jobs` controls how many renders are queued concurrently. Each render internally uses rayon for data parallelism (P1-P4 parallelization). Since rayon uses a single shared thread pool with work-stealing, multiple concurrent renders don't oversubscribe cores — rayon distributes work across its fixed pool regardless of how many renders are active.

Setting `--jobs` higher than the number of cores is not harmful to CPU utilization — rayon handles it gracefully. The constraint is memory (see below).

### Memory considerations

Each concurrent render requires a clone of the decoded image buffer:

- ~300MB per clone at 26MP (6246×4170, 3 channels × f32)
- `--jobs 1` (default): peak ~600MB (original + one clone)
- `--jobs 4`: peak ~1.5GB (original + 4 clones)
- `--jobs 11` (all presets): peak ~3.6GB (original + 11 clones)

Default `--jobs 1` keeps memory flat and is safe for all machines. Users and CI can increase based on available RAM. This memory concern should be considered alongside the existing batch memory pressure item in the performance backlog (which tracks per-image buffer allocation across pipeline stages, including P3 denoise tripling).

`--jobs` higher than core count is not harmful to CPU — rayon's work-stealing handles it. The constraint is purely memory.

## E2E Test Changes

### Current structure

Each of the 6 `#[test]` functions calls `cli_bin()` 12 times (noop + 11 looks), spawning 12 separate processes per image. Each process re-decodes the image.

### New structure

Each test calls `multi-apply` once with all presets + `--noop`. One process spawn, one decode, N renders. Then asserts golden files for each output.

```rust
fn run_image_matrix(image_path: &str, image_name: &str, ..., looks: &[&str]) {
    let dir = TempDir::new().unwrap();
    let input = fixture_path(image_path);
    let mut cmd = cli_bin();
    cmd.args(["multi-apply", "-i", input.to_str().unwrap(),
              "-o", dir.path().to_str().unwrap(), "--noop"]);
    for look in looks {
        cmd.args(["-p", look_preset_path(look).to_str().unwrap()]);
    }
    let status = cmd.status().expect("failed to run multi-apply");
    assert!(status.success(), "multi-apply should succeed for {image_name}");

    // Assert noop golden
    let noop_output = dir.path().join(format!("{image_name}_noop.png"));
    assert_valid_output(&noop_output);
    assert_golden(&noop_output, &format!("{golden_dir}/{image_name}_noop.png"), tolerance, max_diff_pct);

    // Assert each look golden
    for look in looks {
        let output = dir.path().join(format!("{image_name}_{look}.png"));
        assert_valid_output(&output);
        assert_golden(&output, &format!("{golden_dir}/{image_name}_{look}.png"), tolerance, max_diff_pct);
    }
}
```

Cross-image parallelism remains unchanged — Cargo runs the 6 `#[test]` functions concurrently via `--test-threads`. The e2e scripts can pass `--jobs 2` or `--jobs 4` to `multi-apply` for additional intra-image parallelism.

### Expected speedup

- 4 RAW images × 10 eliminated decodes × ~2.5s = **~100s saved** from decode-once
- Process spawn overhead eliminated (72 process spawns → 6)
- Additional speedup from `--jobs` parallelizing renders within each image
- Full suite target: ~3min → under 1min

## Three Levels of Parallelism

This design introduces level 2. Level 1 already exists. Level 3 can be layered on later as a CI-only change.

| Level | Where | What | Status |
|-------|-------|------|--------|
| 1. Engine | Within each render | rayon data parallelism (P1-P4) | Exists |
| 2. Command | `multi-apply --jobs N` | Multiple preset renders concurrently for one image | This design |
| 3. CI | GitHub Actions matrix | Split test functions across runners | This design |

**Level 3 — CI matrix:**

```yaml
e2e-tests:
  strategy:
    matrix:
      image: [temple_blossoms, night_city_blur, sunset_river, foggy_forest, dusk_cityscape, night_architecture]
  steps:
    - run: cargo test -p agx-e2e --release -- cli_${{ matrix.image }}
```

Each runner handles one image. Wall time becomes the slowest single image instead of the sum. The library tests and error cases run in a separate job (or bundled with the lightest image).

## Scope

### In scope

- `multi-apply` CLI subcommand with `-i`, `-p` (multiple), `-o`, `--noop`, `--jobs`
- Auto-named output files (`<image_stem>_<preset_stem>.png`)
- E2E test refactor to use `batch-apply`
- CI matrix strategy: split e2e tests across parallel runners (one per image)
- Update `e2e.sh` and `e2e-quick.sh` if needed

### Out of scope

- Batch-apply across multiple images (that's `batch-edit`)
- Library-level multi-preset API (current library API is sufficient)
- Dehaze parallelization (tracked separately)
- Memory profiling under batch load (tracked in performance backlog)
