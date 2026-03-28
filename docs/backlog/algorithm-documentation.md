# Algorithm Documentation

Human-readable reference docs explaining each image processing algorithm's math, paper references, and constant choices.

## Sub-tasks

- [ ] **Dark Channel Prior and atmospheric scattering model** (dehaze) — `crates/agx/src/adjust/dehaze.rs`
- [ ] **Guided filter** (dehaze refinement) — `crates/agx/src/adjust/dehaze.rs`
- [ ] **Unsharp mask and frequency separation** (detail pass) — `crates/agx/src/adjust/detail.rs`
- [ ] **À trous wavelet decomposition** (noise reduction) — `crates/agx/src/adjust/denoise.rs`
- [ ] **Simplex noise and grain modeling** (grain simulation) — `crates/agx/src/adjust/grain.rs`
- [ ] **Fritsch-Carlson monotone cubic interpolation** (tone curves) — `crates/agx/src/adjust/mod.rs`
- [ ] **3-way lift/gamma/gain luminance weighting** (color grading) — `crates/agx/src/adjust/mod.rs`

## Considerations

- One document per algorithm or group of related algorithms. Could live in `docs/algorithms/`.
- Include: intuition, math (kept accessible), paper references, why specific constants/thresholds were chosen.
- Reference source code locations where each algorithm is implemented.
- Keep separate from code comments — this is explanatory prose for contributors, not API docs.

## Related

- [Processing Parity](processing-parity.md) — algorithm docs help compare our implementations against reference editors
