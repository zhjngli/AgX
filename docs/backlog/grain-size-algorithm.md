# Grain Size Algorithm Rework

> **Parent epic:** [Processing Parity](processing-parity.md)

Fix the grain size frequency mapping so larger sizes produce coarser grain particles, not low-frequency blobs.

## Sub-tasks

- [x] **Narrow the frequency range** — replaced with blur-based approach (fixed high frequency + Gaussian blur for particle size)
- [x] **Evaluate blur-based approach** — implemented: generate noise at fixed frequency, blur to control particle size
- [x] **Add visual/behavioral tests** — variance tests at all size values, visual verification of goldens
- [x] **Update e2e presets** — grain sizes raised to 35-65 range, goldens regenerated

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
