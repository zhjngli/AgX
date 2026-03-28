# Processing Parity

Understanding and reducing rendering differences between AgX and other photo editors, and verifying that each editing algorithm produces correct, artifact-free results.

## Sub-tasks

### Bug fixes

- [ ] **[Grain size algorithm rework](grain-size-algorithm.md)** — grain size mapping produces low-frequency blobs at moderate sizes instead of fine grain particles (detailed investigation and proposed fixes in linked doc)

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

## Considerations

- Rendering differences are expected — there is no single "correct" rendering, only different interpretations.
- Multiple factors contribute: demosaicing algorithm, base tone curves, white balance calculation, exposure mapping, color matrices, highlight handling.
- The per-feature verification is a cross-cutting effort best done across all editing features together rather than piecemeal.
- Normalizing output to match a specific processor is complex (reverse-engineering tone curves and color science). The goal is correctness and quality, not pixel-perfect matching.

## Related

- [Color Management](color-management.md) — per-camera profiles improve starting-point accuracy
- [Algorithm Documentation](algorithm-documentation.md) — understanding our algorithms helps compare against references
- [Ecosystem Interop](ecosystem-interop.md) — users importing presets from other tools expect similar results
