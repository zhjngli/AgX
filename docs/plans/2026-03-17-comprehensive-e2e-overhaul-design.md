# Comprehensive E2E Test Overhaul

## Goal

Transform the e2e test suite from basic sanity checks into a comprehensive visual regression system that tests every fixture image against a set of film-inspired looks, exercises the full CLI pipeline, and covers LUT application, preset composition, and EXIF orientation handling.

## Motivation

The current e2e suite has 22 tests but limited coverage:
- Only 5 golden files, all JPEG-sourced
- RAW tests use sanity checks only (no golden comparison)
- Presets are trivial (single-parameter tweaks like `exposure = 1.0`)
- No LUT testing at all
- No preset composition testing in the e2e path
- EXIF orientation is not handled, causing rotated output for some JPEGs
- Golden file naming is inconsistent (`library_jpeg_default.png` vs `cli_jpeg_apply_preset.png`)

The overhaul addresses all of these by building a realistic IMAGE x LOOK test matrix through the CLI, with film-inspired presets that exercise the full engine pipeline.

## Design

### 1. EXIF Orientation Fix

**Problem:** The `image` crate decodes pixel data in raw sensor orientation. EXIF orientation tags tell viewers how to rotate/flip, but when we re-encode to PNG the tag is lost and images appear rotated (observed with `night_architecture.jpg`).

**Fix:** In `crates/agx/src/decode/`, after decoding standard formats (JPEG, PNG, TIFF, BMP, WebP) via the `image` crate, read the EXIF orientation tag and apply the corresponding pixel transformation before returning the image buffer. Use `image::imageops` for rotate90/180/270 and flip operations.

**EXIF reading dependency:** The decode module cannot import from the `metadata` module (architecture rule). Add `kamadak-exif` as a direct dependency of the `agx` crate. Before decoding, open the file with `exif::Reader` to extract the orientation tag (tag `0x0112`). If the tag is missing or unreadable, default to orientation 1 (no transform). This is a lightweight read — only the EXIF header is parsed, not the full image.

EXIF orientation values and their transforms:
| Value | Transform |
|-------|-----------|
| 1 | None (normal) |
| 2 | Flip horizontal |
| 3 | Rotate 180° |
| 4 | Flip vertical |
| 5 | Transpose (flip horizontal + rotate 270°) |
| 6 | Rotate 90° CW |
| 7 | Transverse (flip horizontal + rotate 90°) |
| 8 | Rotate 270° CW |

This only affects standard format decoding — LibRaw already handles orientation for RAW files.

### 2. Film-Inspired Looks

Six looks covering a range of photographic styles:

| Look | Style | Composition | Key Character |
|------|-------|-------------|---------------|
| **Portra 400** | Film emulation | Self-contained | Warm pastels, lifted blacks, soft highlight rolloff, subtle desaturation |
| **Neo Noir** | Cinematic | Self-contained | High contrast, crushed blacks, cool desaturated tones, blue-teal shadows |
| **Blade Runner** | Cinematic | Extends `base_cinematic.toml` | Neon-warm highlights, teal shadows, elevated contrast, orange/teal split |
| **Cinema Warm** | Cinematic | Extends `base_cinematic.toml` | Golden midtones, softened highlights, earthy palette, moderate contrast |
| **Kodachrome 64** | Film emulation | Self-contained | Rich saturation, warm reds/yellows, deep blues, punchy contrast |
| **Nordic Fade** | Editorial/moody | Self-contained | Cool desaturated tones, heavily lifted blacks, faded matte look |

**`base_cinematic.toml`** is a shared base preset, never applied directly. It defines common cinematic foundations: moderate contrast boost, slightly pulled highlights, lifted shadows. Blade Runner and Cinema Warm extend it with their own parameter overrides and distinct LUTs, exercising preset composition in the e2e path.

Each look is a `.toml` preset that combines parameter adjustments with an optional LUT reference. The preset parameters handle tonal shaping (exposure, contrast, highlights, shadows, HSL) while the LUT handles non-linear color character that parameters alone cannot achieve.

#### File structure

```
fixtures/
  looks/
    portra_400.toml           # self-contained, references luts/portra_400.cube
    neo_noir.toml             # self-contained, references luts/neo_noir.cube
    blade_runner.toml         # extends base_cinematic.toml, references luts/blade_runner.cube
    cinema_warm.toml          # extends base_cinematic.toml, references luts/cinema_warm.cube
    kodachrome_64.toml        # self-contained, references luts/kodachrome_64.cube
    nordic_fade.toml          # self-contained, references luts/nordic_fade.cube
    base_cinematic.toml       # shared base, not applied directly
    luts/                     # subdirectory of looks/ (LUT paths resolve relative to preset file)
      portra_400.cube         # 33x33x33
      neo_noir.cube
      blade_runner.cube
      cinema_warm.cube
      kodachrome_64.cube
      nordic_fade.cube
```

Note: LUTs live under `looks/luts/` because the preset loader resolves `[lut] path` relative to the preset file's parent directory. Presets reference them as `path = "luts/portra_400.cube"`.

The old `fixtures/presets/` directory (`warm_exposure.toml`, `high_contrast.toml`, `hsl_vibrant.toml`) is removed. The new looks supersede them with more comprehensive parameter coverage.

### 3. LUT Generation

Each look gets a 33x33x33 `.cube` file (35,937 RGB entries). These are generated programmatically by a dev script and committed as fixtures.

#### Generator tool

A small Rust binary crate at `crates/agx-lut-gen/` (workspace member, `publish = false`). Run via `cargo run -p agx-lut-gen -- --output-dir crates/agx-e2e/fixtures/looks/luts/`. It defines each look's transform chain, iterates the 33^3 grid, and writes standard `.cube` files. This is a dev-only tool — not part of the library or CLI.

#### Transform building blocks

| Transform | What it does | Example use |
|-----------|-------------|-------------|
| Lift/gamma/gain | Shift shadows (lift), midtones (gamma), highlights (gain) per channel | Portra: lift blue shadows, warm gain |
| Saturation scale | Multiply saturation globally or per-luminance range | Nordic Fade: desaturate globally |
| Hue rotation | Rotate hue in specific luminance/saturation ranges | Neo Noir: push shadows toward teal |
| Channel crossfeed | Mix small amounts of one channel into another | Kodachrome: red into green for warmth |
| Contrast curve | S-curve or film-shoulder response per channel | Neo Noir: hard S-curve, Portra: soft shoulder |

Each look combines 2-4 of these transforms to build its color character.

#### Per-look transform design

**Portra 400:**
- Soft film-shoulder highlight curve (gentle rolloff, no hard clipping)
- Lift blue channel in shadows (shadow_lift_b ≈ 0.03-0.05)
- Slight global desaturation (saturation_scale ≈ 0.85-0.90)
- Warm highlight gain (gain_r slightly above 1.0, gain_b slightly below)
- Goal: organic, flattering skin tones with warm pastel quality

**Neo Noir:**
- Aggressive S-curve (steep midtone contrast, crushed blacks, compressed highlights)
- Strong teal shadow push (hue rotation in low-luminance region toward cyan/teal)
- Heavy global desaturation (saturation_scale ≈ 0.4-0.5)
- Slight cool overall shift (lift_b ≈ 0.02)
- Goal: high-contrast monochromatic feel with cold undertone

**Blade Runner:**
- Orange/teal color split: warm shadow lift (lift_r, slight lift_g) + cool highlight gain inverted — actually: teal shadows (lift_b, lift_g elevated) + warm/orange highlights (gain_r elevated, gain_b reduced)
- Moderate S-curve for contrast
- Slightly elevated saturation in warm tones
- Goal: neon cyberpunk atmosphere, warm faces against cool backgrounds

**Cinema Warm:**
- Golden midtone push via channel crossfeed (small red→green mix for warmth)
- Soft highlight compression (film shoulder, less aggressive than Neo Noir)
- Warm overall shift (gamma biased toward amber)
- Moderate contrast, no crushed blacks
- Goal: golden hour warmth, nostalgic, earthy

**Kodachrome 64:**
- Per-channel S-curves: strong red and blue curves, moderate green
- Slight red→green crossfeed for characteristic warmth
- Elevated saturation (saturation_scale ≈ 1.10-1.15)
- Punchy shadows (shadow lift minimal, blacks stay deep)
- Goal: vivid, saturated, iconic Kodachrome punch

**Nordic Fade:**
- Inverse S-curve (lifted blacks ≈ 0.08-0.12, compressed highlights ≈ 0.85-0.90)
- Cool hue rotation globally (slight shift toward blue-green)
- Heavy desaturation (saturation_scale ≈ 0.55-0.65)
- Slight green channel elevation in midtones
- Goal: muted, faded editorial look with cool undertone

#### Visual validation loop

LUTs and presets must not be generated blindly from math alone. Each look goes through an iterative visual validation process:

1. **Generate** the initial LUT and preset from the transform design above.
2. **Apply** the look to a representative test image (e.g., `temple_blossoms.jpg` for warm/color looks, `night_city_blur.raf` for dark/contrast looks).
3. **Inspect** the rendered output visually. Check: does it match the intended mood? Are highlights preserved? Are shadows crushed appropriately or too aggressively? Do skin tones (if present) look natural? Is the color cast what we wanted?
4. **Iterate** on the transform parameters if the result doesn't match the intended style. Adjust curves, shift tint values, tweak saturation scales until the output looks right.
5. **Cross-check** against a second image to confirm the look generalizes (doesn't only work on one photo).

This loop happens during implementation, not as an automated test. The goal is that by the time a look is committed, its golden files represent aesthetically intentional output — not just "the algorithm ran without crashing."

#### Future improvement

More complex LUT generation with measured spectral film data or empirical matching to real film stocks is out of scope. A separate design doc can define algorithms for higher-fidelity film emulation.

### 4. Test Matrix & Golden Files

**Matrix:** 6 images x 7 states (noop + 6 looks) = **42 golden files**.

#### Golden file organization

```
fixtures/golden/
  jpeg/
    temple_blossoms_noop.png
    temple_blossoms_portra_400.png
    temple_blossoms_neo_noir.png
    temple_blossoms_blade_runner.png
    temple_blossoms_cinema_warm.png
    temple_blossoms_kodachrome_64.png
    temple_blossoms_nordic_fade.png
    night_architecture_noop.png
    night_architecture_portra_400.png
    ...  (14 JPEG goldens total)
  raw/
    night_city_blur_noop.png
    night_city_blur_portra_400.png
    ...
    dusk_cityscape_nordic_fade.png
    ...  (28 RAW goldens total)
```

**Naming convention:** `<image_name>_<look_name>.png`. Consistent, grep-friendly, immediately identifies the source image and applied style.

#### Tolerance strategy

| Category | Per-channel tolerance | Max diff percentage | Rationale |
|----------|----------------------|--------------------:|-----------|
| JPEG goldens | 2 | 0.0% | Deterministic via `image` crate, any diff is a regression |
| RAW goldens | 30 | 10.0% | LibRaw output varies across platforms/versions |

These RAW thresholds may need tuning after observing actual cross-platform diffs. They can be tightened over time.

**Future improvement:** Platform-specific golden directories (`golden/raw/macos/`, `golden/raw/linux/`) for tight RAW comparison. Flagged for later.

#### Changes to comparison utilities

Extend `compare_images` and `assert_golden` in `src/lib.rs` to accept `max_diff_pct: f64`:

```rust
pub fn assert_golden(actual: &Path, golden_name: &str, tolerance: u8, max_diff_pct: f64)
```

The existing `compare_images` already returns a `ComparisonError` with `diff_percentage`. The change is in `assert_golden`: instead of failing on any `Err`, it catches the error and checks `err.diff_percentage <= max_diff_pct`. If within the percentage threshold, the comparison passes. If above, it panics with the full diff stats. JPEG callers pass `(2, 0.0)` (any differing pixel fails), RAW callers pass `(30, 10.0)` (up to 10% of pixels may exceed tolerance 30).

Starting RAW tolerance at 30/10% is deliberately permissive to avoid false CI failures. These values should be tightened based on observed cross-platform diffs once we have data.

### 5. Test Code Structure

#### CLI pipeline tests (`tests/cli_pipeline.rs`) — primary matrix

Data-driven approach using const arrays:

```rust
const JPEG_IMAGES: &[&str] = &[
    "jpeg/temple_blossoms.jpg",
    "jpeg/night_architecture.jpg",
];

const RAW_IMAGES: &[&str] = &[
    "raw/night_city_blur.raf",
    "raw/sunset_river.raf",
    "raw/foggy_forest.raf",
    "raw/dusk_cityscape.raf",
];

const LOOKS: &[&str] = &[
    "portra_400",
    "neo_noir",
    "blade_runner",
    "cinema_warm",
    "kodachrome_64",
    "nordic_fade",
];
```

Test categories:
1. **Noop** — each image through `agx-cli edit` with no adjustment flags. Golden: `<image>_noop.png`.
2. **Look matrix** — each image x each look via `agx-cli apply -p <look>.toml`. Golden: `<image>_<look>.png`.
3. **Error cases** — corrupt file, nonexistent input (kept from current suite).
4. **Batch test** — one batch-edit test to verify the batch workflow still works.

The CLI tests exercise the full pipeline: argument parsing → preset loading (including `extends` resolution) → LUT loading → decode → engine render → encode.

#### Library pipeline tests (`tests/library_pipeline.rs`) — slim smoke tests

Minimal API verification, not a matrix:
- One noop decode-render-encode per format (JPEG, RAW) — verifies the public API works
- One `apply_preset()` call with a look — verifies preset loading through library API
- One direct `params_mut()` manipulation — verifies programmatic parameter setting
- One LUT load and apply — verifies `Lut3D::load()` and engine LUT application

These exist to verify the library API surface, not to duplicate the CLI matrix.

### 6. Cleanup

- Remove `fixtures/presets/` directory (old presets replaced by looks)
- Remove old golden files (replaced by new naming scheme under `golden/jpeg/` and `golden/raw/`)
- Remove redundant library pipeline tests (matrix coverage moves to CLI)

### 7. Summary of Changes

| Area | Change |
|------|--------|
| `crates/agx/src/decode/` | EXIF orientation handling for standard formats |
| `crates/agx-e2e/src/lib.rs` | Add `max_diff_pct` to golden comparison |
| `crates/agx-e2e/fixtures/looks/` | 6 look presets + 1 shared base |
| `crates/agx-e2e/fixtures/looks/luts/` | 6 generated 33x33x33 `.cube` files |
| `crates/agx-e2e/fixtures/golden/` | 42 golden files in `jpeg/` and `raw/` subdirs |
| `crates/agx-e2e/tests/cli_pipeline.rs` | Data-driven IMAGE x LOOK matrix |
| `crates/agx-e2e/tests/library_pipeline.rs` | Slim API smoke tests |
| `crates/agx-lut-gen/` | Dev-only binary crate for `.cube` file generation |
| `crates/agx-e2e/fixtures/presets/` | Removed (replaced by looks) |

### 8. Out of Scope — Flagged for Future

- **Platform-specific golden directories** for tighter RAW comparison
- **Complex LUT generation** with measured spectral film data (separate design doc)
- **HEIC format support** (logged in ideas backlog)
