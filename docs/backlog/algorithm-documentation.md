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

## Research: WGSL documentation tooling

WGSL has no `include_str!` equivalent, so the documentation-in-code philosophy doesn't naturally extend to shaders. Before implementing sub-project #4, research these tools/approaches to see if they can close the gap:

- **`wgsl_to_wgpu`** — generates Rust types from WGSL struct definitions. Could make WGSL the source of truth for `GpuParameters` layout, and carry doc comments from WGSL into generated Rust types that appear in rustdoc. Evaluate whether it's mature enough and whether its generated code is clean enough to use.
- **`wgsl-analyzer`** — LSP for WGSL. May have doc comment support or structured metadata extraction that could feed into a doc pipeline.
- **`naga`** — the shader compiler underlying wgpu. Parses WGSL into an IR. Could potentially extract struct definitions, entry points, and comments programmatically for doc generation.
- **Custom `agx-docgen` extension** — extend the existing docgen crate to parse WGSL files and generate shader reference pages (entry points, buffer bindings, struct layouts) for mdbook. Would give WGSL a documentation surface without requiring WGSL-native doc tooling.
- **Structured WGSL header comments** — convention where each `.wgsl` file starts with a structured comment block (algorithm name, canonical doc path, buffer bindings summary) that a simple parser can extract. Low-tech but keeps documentation discoverable from the shader side.
- **mdbook `{{#include}}` with WGSL snippets** — mdbook's built-in `{{#include}}` supports line ranges and anchor comments. Algorithm explanation pages could pull relevant WGSL math directly from shader source into the prose (e.g., showing the grain blending formula from `grain_apply.wgsl` alongside the Rust equivalent). Research whether mdbook preprocessors like `mdbook-embed` offer better ergonomics for this. Scope overlaps with sub-projects #1 and #4.

## Related

- [Processing Parity](processing-parity.md) — algorithm docs help compare our implementations against reference editors
