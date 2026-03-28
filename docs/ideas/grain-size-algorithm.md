# Grain Size Algorithm Rework

**Category:** Editing — Neighborhood Operations
**Status:** Backlog

## Problem

The current grain size parameter maps to simplex noise frequency via an exponential curve:
```
base_freq = 0.1 * (0.02).powf(size / 100.0)
```

This shifts the entire frequency spectrum downward as size increases. At moderate values (size=40-50), the noise becomes low-frequency cloud-like blobs rather than visible film grain. Real film grain (as in Lightroom or Capture One) increases the size of individual grain particles (1-5 pixels) while keeping the overall noise high-frequency and localized.

## Current Behavior

- size=0: base_freq=0.1 — fine, dense grain (good)
- size=25: base_freq≈0.047 — starting to get soft
- size=50: base_freq≈0.014 — visible low-frequency blobs
- size=100: base_freq=0.002 — huge cloud-like splotches

## Proposed Fix

Narrow the frequency range so that even size=100 stays in "visible grain" territory:
```
base_freq = 0.1 * (0.15).powf(size / 100.0)
```
This gives: size=0 → 0.1, size=50 → 0.039, size=100 → 0.015 — much tighter range.

Alternatively, consider a fundamentally different approach: keep the base frequency high and instead control grain "particle size" by blurring the noise output with a small kernel (1-3 pixel radius), which more closely mimics how real film grain scales.

## Testing

When reworking the algorithm, add visual/behavioral tests:
- Verify grain at size=100 still looks like grain, not clouds
- Verify grain at size=0 vs size=100 differ in particle size, not frequency character
- Compare against reference images from Lightroom/Capture One at equivalent settings
- Test on large uniform areas (sky, walls) where blob artifacts are most visible

## Related

- `crates/agx/src/adjust/grain.rs` — current implementation
- `docs/plans/2026-03-23-grain-design.md` — original grain design spec
