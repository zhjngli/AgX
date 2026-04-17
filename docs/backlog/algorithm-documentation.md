# Algorithm Documentation

Human-readable reference docs explaining each image processing algorithm's math, paper references, and constant choices. Each algorithm has both a CPU (Rust) and GPU (WGSL) implementation that follow the same math — document the algorithm once, reference both source locations.

## Sub-tasks

- [ ] **Dark Channel Prior and atmospheric scattering model** (dehaze) — CPU: `crates/agx/src/adjust/dehaze.rs`, GPU: `crates/agx/src/shaders/dehaze_*.wgsl` + `crates/agx/src/engine/gpu/stages/dehaze.rs`
- [ ] **Guided filter** (dehaze refinement) — CPU: `crates/agx/src/adjust/dehaze.rs`, GPU: `crates/agx/src/shaders/dehaze_guided_coeffs.wgsl` / `dehaze_box_filter.wgsl` / `dehaze_fma.wgsl`
- [ ] **Unsharp mask and frequency separation** (detail pass) — CPU: `crates/agx/src/adjust/detail.rs`, GPU: `crates/agx/src/shaders/detail_*.wgsl` + `blur_*.wgsl`
- [ ] **À trous wavelet decomposition** (noise reduction) — CPU: `crates/agx/src/adjust/denoise.rs`, GPU: `crates/agx/src/shaders/denoise_*.wgsl`
- [ ] **Simplex noise and grain modeling** (grain simulation) — CPU: `crates/agx/src/adjust/grain.rs`, GPU: `crates/agx/src/shaders/grain_*.wgsl`
- [ ] **Fritsch-Carlson monotone cubic interpolation** (tone curves) — CPU: `crates/agx/src/adjust/mod.rs`, GPU: `crates/agx/src/shaders/gamma_adjustments.wgsl`
- [ ] **3-way lift/gamma/gain luminance weighting** (color grading) — CPU: `crates/agx/src/adjust/mod.rs`, GPU: `crates/agx/src/shaders/gamma_adjustments.wgsl`
- [ ] **GPU dual-path contributor guide** — `crates/agx/src/engine/gpu/README.md` documenting dual-path architecture, how to add new adjustments to both paths, and `GpuParameters` ↔ WGSL `Params` struct mapping

## Considerations

- One document per algorithm or group of related algorithms, living as sibling `.md` files next to `crates/agx/src/adjust/*.rs` per the [documentation initiative design](../plans/2026-04-06-documentation-initiative-design.md).
- Include: intuition, math (kept accessible), paper references, why specific constants/thresholds were chosen.
- Document the algorithm implementation-agnostically. Each explanation page includes a "Source" section listing both CPU (Rust) and GPU (WGSL) source locations.
- Keep separate from code comments — this is explanatory prose for contributors and curious users, not API docs.

## Related

- [Processing Parity](processing-parity.md) — algorithm docs help compare our implementations against reference editors
