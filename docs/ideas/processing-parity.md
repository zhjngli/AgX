# Processing Parity

Understanding and reducing rendering differences between AgX and other photo editors.

## Sub-tasks

- [ ] **Reference audit** — read darktable/RawTherapee source for each adjustment type, document algorithms and compare to ours
- [ ] **Visual comparison tooling** — process the same image with identical parameters in AgX vs Lightroom, diff the output
- [ ] **Configurable base tone curves** — flat, medium contrast, match-Lightroom, etc. (each raw processor applies its own base curve)
- [ ] **Per-camera color profiles (DCP/ICC)** — more accurate starting points for raw conversion
- [ ] **Demosaicing algorithm selection** — LibRaw defaults (AHD/PPG) differ from commercial processors; user-selectable algorithms
- [ ] **Editing algorithm refinement** — adjust weight curves, blending math, or parameter scaling where results diverge from expected behavior

## Considerations

- Rendering differences are expected — there is no single "correct" rendering, only different interpretations.
- Multiple factors contribute: demosaicing algorithm, base tone curves, white balance calculation, exposure mapping, color matrices, highlight handling.
- This is a cross-cutting effort best analyzed across all editing features at once rather than per-feature.
- Normalizing output to match a specific processor is complex (reverse-engineering tone curves and color science).

## Related

- [Color Management](color-management.md) — per-camera profiles improve starting-point accuracy
- [Algorithm Documentation](algorithm-documentation.md) — understanding our algorithms helps compare against references
- [Ecosystem Interop](ecosystem-interop.md) — users importing presets from other tools expect similar results
