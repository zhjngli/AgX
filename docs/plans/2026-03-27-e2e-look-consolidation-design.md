# E2E Look Consolidation

**Date:** 2026-03-27
**Status:** Draft

## Problem

The e2e test suite has 24 looks, most testing a single feature in isolation (e.g., `dehaze_landscape` only sets dehaze, `grain_silver` only sets grain). This produces 129 golden files and grows by 5–10 per new feature. The looks don't reflect how a photographer actually edits — stacking many adjustments to achieve a cohesive aesthetic.

## Goal

Replace the 24 single-feature looks with ~12 curated, multi-feature looks that resemble real photo presets. Each look exercises many editing features organically. Every feature is covered by at least 2 looks. Golden file count drops from 129 to ~72.

## Look Definitions

### Color Film Stocks

**Portra 400** — Kodak's classic portrait film. Warm skin tones, soft contrast, gentle rolloff in highlights. Slightly lifted shadows, subtle grain.
- Features: LUT, tone, white balance, HSL, vignette, grain (silver), tone curves (gentle S-curve), noise reduction

**Kodachrome 64** — Saturated Kodachrome slide film. Rich reds and blues, punchy contrast, fine grain, sharp rendering.
- Features: LUT, tone, white balance, HSL, grain (fine), detail (clarity + sharpening), vignette

**Cinestill 800T** — Tungsten-balanced cinema stock. Cool overall cast, halation glow around highlights, noticeable grain, urban night feel.
- Features: LUT, tone, white balance (tungsten shift), HSL, color grading (cool shadows, warm highlights), grain (harsh, chromatic), dehaze (slight negative for glow), noise reduction

### B&W Film Stocks

**Tri-X 400** — Kodak's photojournalism workhorse. Punchy midtone contrast, visible grain, gritty character.
- Features: LUT, tone, grain (cubic, heavy), detail (clarity + sharpening), vignette

**T-Max 100** — Kodak's technical fine-grain B&W. Smooth tonal transitions, minimal grain, clean detail.
- Features: LUT, tone, grain (fine, subtle), detail (texture + fine sharpening), tone curves (gentle shadow lift), noise reduction

### Digital B&W

**High-Contrast B&W** — Dramatic digital conversion. Crushed blacks, blown highlights, aggressive clarity.
- Features: LUT, tone, detail (clarity + aggressive sharpening), vignette (heavy), tone curves (steep S-curve)

**Faded B&W** — Lo-fi digital conversion. Lifted blacks, rolled-off highlights, soft and dreamy.
- Features: LUT, tone, tone curves (lifted blacks, rolled highlights), grain (soft), detail (negative texture)

### Digital Creative

**Blade Runner** — Neon sci-fi. Teal and orange color push, high contrast, cinematic grain. Extends base_cinematic.
- Features: extends base_cinematic, LUT, tone, white balance, HSL, color grading (teal shadows, orange highlights), grain (tabular), detail (clarity)

**Neo Noir** — Dark and moody. Deep blacks, desaturated with selective cool accents, heavy vignette.
- Features: LUT, tone, white balance, HSL, color grading (cool global tint), vignette (heavy), grain (harsh), detail (sharpening + high masking)

**Warm Cinema** — Golden hour warmth. Soft highlights, open shadows, warm color grading. Extends base_cinematic.
- Features: extends base_cinematic, LUT, tone, white balance, HSL, color grading (warm midtones, golden highlights), noise reduction, grain (silver, subtle)

**Dune** — Desert sci-fi. Desaturated, warm, sandy atmosphere. Hazy with texture.
- Features: LUT, tone, white balance, HSL (desaturate greens/blues, boost orange/yellow), color grading (warm overall), dehaze (positive, sandy atmosphere), grain (tabular), detail (texture), vignette

**Base Cinematic** — Shared base for Blade Runner and Warm Cinema. Tests preset `extends` feature.
- Features: tone (moderate contrast, pulled highlights, lifted shadows)

## Feature Coverage

Every editing feature appears in at least 2 looks:

| Feature | Looks |
|---------|-------|
| Tone (exposure, contrast, highlights, shadows, whites, blacks) | All |
| White Balance | Portra, Kodachrome, Cinestill, Blade Runner, Neo Noir, Warm Cinema, Dune |
| HSL | Portra, Kodachrome, Cinestill, Blade Runner, Neo Noir, Warm Cinema, Dune |
| Color Grading | Cinestill, Blade Runner, Neo Noir, Warm Cinema, Dune |
| Tone Curves | Portra, T-Max 100, High-Contrast B&W, Faded B&W |
| LUT | All except Noop and Base Cinematic |
| Detail (sharpening, clarity, texture) | Kodachrome, Tri-X, T-Max, High-Contrast B&W, Faded B&W, Blade Runner, Neo Noir, Dune |
| Dehaze | Cinestill (negative), Dune (positive) |
| Noise Reduction | Portra, Cinestill, T-Max, Warm Cinema |
| Grain | Portra, Kodachrome, Cinestill, Tri-X, T-Max, Faded B&W, Blade Runner, Neo Noir, Warm Cinema, Dune |
| Vignette | Portra, Kodachrome, Tri-X, High-Contrast B&W, Neo Noir, Dune |
| Preset Extends | Blade Runner, Warm Cinema (both extend base_cinematic) |

## Test Matrix

- **Images**: 6 (2 JPEG, 4 RAW) — unchanged
- **Looks**: 11 (excluding base_cinematic, which is tested via extends)
- **Noop**: 1 per image
- **Total golden files**: ~72 (12 per image × 6 images)

### Test structure (unchanged)
- `ALL_LOOKS`: 11 entries
- `BW_LOOKS`: removed (B&W looks now in ALL_LOOKS, work on all images)
- Per-image test functions: 6 (count unchanged; `night_architecture` switches from `BW_LOOKS` to `ALL_LOOKS`)
- Library pipeline tests: 7 (unchanged)
- Error/batch tests: 3 (unchanged)

## LUT Strategy

- **Keep existing**: portra_400, kodachrome_64, blade_runner, cinema_warm, neo_noir (5)
- **Repurpose**: nordic_fade → Dune, bw_high_contrast → High-Contrast B&W, bw_lofi → Faded B&W, bw_street → Tri-X (4)
- **New**: Cinestill 800T, T-Max 100 (2)
- **No LUT**: base_cinematic (never had one, unchanged)
- Total: 11 LUTs

## Implementation Workflow

### Two-pass tuning
1. **First pass**: Create all preset .toml files with organic, photographer-style parameter values. Generate new LUTs. Render all images against all looks.
2. **Visual review**: Both human and AI review rendered outputs. Identify looks that feel off — too harsh, too subtle, wrong character.
3. **Second pass**: Tune preset values and LUT curves based on review. Re-render.
4. **Commit**: Once looks are approved, regenerate goldens with `GOLDEN_UPDATE=1` and commit.

### Preset value philosophy
Parameter values should feel like a real photographer dialed them in — organic, not mechanical. No round-number-only values. The aesthetic drives the numbers, not the other way around.

## What Changes

- `crates/agx-e2e/fixtures/looks/`: Delete all 25 existing .toml files. Create 12 new ones.
- `crates/agx-e2e/fixtures/looks/luts/`: Rename 4 repurposed LUTs, generate 2 new ones, delete unused ones.
- `crates/agx-e2e/tests/cli_pipeline.rs`: Update `ALL_LOOKS`, remove `BW_LOOKS` and `night_architecture` special case.
- `crates/agx-e2e/fixtures/golden/`: Delete all 129 goldens, regenerate ~72.
- Library and error tests: unchanged.
- Scripts: unchanged.

## What Stays the Same

- Test infrastructure (assert_golden, compare_images, run_image_matrix)
- All 6 test images
- Library pipeline tests
- Error case tests
- e2e.sh / e2e-quick.sh scripts
- Golden comparison tolerances
