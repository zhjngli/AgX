# Processing Parity

Understanding and reducing rendering differences between AgX and other photo editors, and verifying that each editing algorithm produces correct, artifact-free results.

Two purposes sit under this epic. The first is **correctness** — each adjustment should produce artifact-free results that hold up against open-source references (darktable, RawTherapee). The second, sharpened by the preset-language direction, is **cross-engine consistency**: when an AgX preset is exported to another engine, that engine should reproduce the AgX look faithfully — a recognizable approximation, not the pixel-identical match the Considerations below rule out. Both reduce to the same prerequisite: knowing exactly how AgX renders each parameter.

## Sub-tasks

### Bug fixes

- [x] **Grain size algorithm rework** — fixed: replaced frequency-based sizing with blur-based approach (fixed high-frequency noise + Gaussian blur for particle size)

### Per-feature verification

Verify each editing algorithm against open-source references (darktable, RawTherapee source) and visually compare output against Lightroom/Capture One. For each feature: audit the algorithm, diff outputs with identical parameters, refine where results diverge.

- [ ] Exposure
- [ ] Contrast
- [ ] Highlights / Shadows
- [ ] Whites / Blacks
- [ ] White balance
- [ ] HSL adjustments
- [ ] Color grading (3-way lift/gamma/gain)
- [ ] Tone curves
- [ ] Vignette
- [ ] Detail pass (sharpening, clarity, texture)
- [ ] Dehaze
- [ ] Noise reduction
- [ ] Grain

### Raw processing

- [ ] **Configurable base tone curves** — flat, medium contrast, match-Lightroom, etc. (each raw processor applies its own base curve before user adjustments)
- [ ] **Per-camera color profiles (DCP/ICC)** — more accurate starting points for raw conversion
- [ ] **Demosaicing algorithm selection** — LibRaw defaults (AHD/PPG) differ from commercial processors; user-selectable algorithms

### Tooling

- [ ] **Visual comparison tooling** — process the same image with identical parameters in AgX vs Lightroom, diff the output
- [ ] **Reference audit** — read darktable/RawTherapee source for each adjustment type, document algorithms and compare to ours

## Parked

- **Grain color tint per type.** Real film stocks have characteristic color biases in grain (Portra skews warm/orange, Fuji skews green). A per-`GrainType` directional color tint applied to the noise, gated on pixel saturation so BW/desaturated pixels stay neutral, could map naturally (Fine = neutral, Silver = slight warm, Harsh = cooler/green). Distinct from existing chromatic grain (random per-channel divergence) — this is a systematic directional shift. Surfaced as a future enhancement when the grain-size frequency rework shipped.

## Considerations

- Rendering differences are expected — there is no single "correct" rendering, only different interpretations.
- Multiple factors contribute: demosaicing algorithm, base tone curves, white balance calculation, exposure mapping, color matrices, highlight handling.
- The per-feature verification is a cross-cutting effort best done across all editing features together rather than piecemeal.
- Normalizing output to match a specific processor is complex (reverse-engineering tone curves and color science). The goal is correctness and quality, not pixel-perfect matching.

## Related

- [Color Management](color-management.md) — per-camera profiles improve starting-point accuracy
- [Documentation Initiative](documentation-initiative.md) — the shipped algorithm explanations aid comparison against reference editors
- [Ecosystem Interop](ecosystem-interop.md) — cross-engine consistency is the same calibration problem; a faithful preset export depends on knowing exactly how AgX renders each parameter
