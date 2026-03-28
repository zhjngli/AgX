# Grain Size Algorithm Rework

> **Parent epic:** [Processing Parity](processing-parity.md)

Fix the grain size frequency mapping so larger sizes produce coarser grain particles, not low-frequency blobs.

## Sub-tasks

- [ ] **Narrow the frequency range** — change `base_freq = 0.1 * (0.02).powf(size / 100.0)` to keep even size=100 in "visible grain" territory (e.g., `0.1 * (0.15).powf(size / 100.0)`)
- [ ] **Evaluate blur-based approach** — keep base frequency high and control particle size by blurring the noise output with a small kernel (1-3px radius), mimicking real film grain scaling
- [ ] **Add visual/behavioral tests** — verify grain at all sizes looks like grain, not clouds; test on uniform areas (sky, walls) where blob artifacts are most visible
- [ ] **Update e2e presets** — remove the grain size ≤30 cap once the algorithm produces good results at higher sizes

## Problem

The current grain size parameter maps to simplex noise frequency via an exponential curve:
```
base_freq = 0.1 * (0.02).powf(size / 100.0)
```

This shifts the entire frequency spectrum downward as size increases. At moderate values (size=40-50), the noise becomes low-frequency cloud-like blobs rather than visible film grain. Real film grain (as in Lightroom or Capture One) increases the size of individual grain particles (1-5 pixels) while keeping the overall noise high-frequency and localized.

### Current behavior

- size=0: base_freq=0.1 — fine, dense grain (good)
- size=25: base_freq≈0.047 — starting to get soft
- size=50: base_freq≈0.014 — visible low-frequency blobs
- size=100: base_freq=0.002 — huge cloud-like splotches

## Related

- `crates/agx/src/adjust/grain.rs` — current implementation
- `docs/plans/2026-03-23-grain-design.md` — original grain design spec
