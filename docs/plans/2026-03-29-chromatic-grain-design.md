# Chromatic Grain: Type-Driven Color Variation

**Date:** 2026-03-29
**Status:** Approved
**Branch:** `fix/grain-size-algorithm` (continuation of grain rework)

## Problem

The current `chromatic` parameter (0-100) generates fully independent per-channel white noise buffers, producing digital-looking RGB confetti. This looks nothing like real film grain color variation. Only one preset uses the parameter (cinestill_800t at 25.0), and the results are not visually pleasing.

## Industry Context

| Editor | Chromatic grain approach |
|--------|------------------------|
| **Lightroom/ACR** | No chromatic grain at all — luminance only |
| **Capture One** | Implicit in grain type — no user slider |
| **darktable** | No chromatic grain |
| **DaVinci Resolve** | Per-channel independent noise (same as our current approach) |
| **Boris FX** | Separate R/G/B sliders — most "digital" approach |

Most pro photo editors either skip chromatic grain entirely or bake it into the grain type. The explicit per-channel slider approach (Resolve, Boris) is considered the most digital-looking.

## Film Grain Color Science

Color film has three emulsion layers (blue/green/red sensitive), each with its own silver halide crystals. All layers are exposed to the same light, so their grain patterns are strongly correlated — but not identical. Each layer has slightly different crystal sizes and sensitivities, producing:

- A shared luminance grain pattern (all layers mostly agree)
- Small per-layer deviations where the layers respond slightly differently
- The result: subtle warm/cool shifts at grain boundaries, not random color speckles

This means film grain color variation is **correlated with the luminance grain**, not independent.

## Solution: Type-Driven Correlated Chromatic Grain

### API change

Remove `chromatic` from the user-facing `GrainParams`. The user controls: `grain_type`, `amount`, `size`, `seed`. Each grain type internally defines its own chromatic intensity. This matches the Capture One model.

### Algorithm

Per-channel noise is derived from the shared luminance noise plus a small independent perturbation, rather than being fully independent:

```
channel_noise[idx] = shared[idx] * (1 - chromatic) + independent[idx] * chromatic
```

At low chromatic values (0.03-0.15), the per-channel noise is 85-97% shared luminance, with only a small independent component. This produces the "film layers slightly disagreeing" look rather than RGB confetti.

### Saturation-scaled application

The chromatic perturbation is scaled by the pixel's own saturation (chroma), computed in sRGB gamma space (the same space grain operates in):

```
pixel_chroma = max(R,G,B) - min(R,G,B)
effective_chromatic = grain_type.chromatic * pixel_chroma
```

This ensures:
- **Black and white photos:** No color shifts (pixel_chroma ≈ 0 for grayscale pixels)
- **Vivid colors:** Full chromatic grain effect
- **Pastels/muted colors:** Proportionally reduced chromatic variation

No image-level BW detection is needed — it's handled per-pixel.

### Per-type chromatic values

| Grain Type | Chromatic | Character |
|------------|-----------|-----------|
| Fine | 0.03 | Very subtle — clean, modern film |
| Silver | 0.05 | Subtle — classic film look |
| Soft | 0.05 | Subtle — gentle color variation |
| Tabular | 0.08 | Moderate — visible on vivid colors |
| Cubic | 0.12 | Noticeable — bold color film |
| Harsh | 0.15 | Most noticeable — pushed film |

These are blend factors in [0, 1] (not on the old user-facing 0-100 scale). Starting points — final values determined by visual grid search tuning (same process used for luminance grain constants).

### Why all types get some chromatic

Even traditionally "BW" grain types (Fine, Silver, Harsh) get a non-zero chromatic value. This is safe because:
- On BW images, pixel_chroma ≈ 0 zeroes out the effect automatically
- On color images, it adds appropriate film-like variation
- The grain type represents grain *character*, not image color mode

### Memory and optimization

With all grain types having chromatic > 0, the luminance-only fast path (`chroma_blend == 0.0`) is effectively eliminated for all grain applications. This means:

- **Before:** Most grain calls used 1 noise buffer (luminance-only). Only cinestill_800t used 4 buffers.
- **After:** All grain calls use 4 noise buffers (1 shared + 3 per-channel perturbation). Peak during blur: ~5 buffers.
- Per buffer at 24MP: ~92MB. Peak ~460MB for grain step.
- This is comparable to the detail pass buffer allocations and acceptable for the image sizes this pipeline handles.

The luminance-only code path is removed (dead code since all types have chromatic > 0).

### Backward compatibility

No concern — no official release yet. Remove `chromatic` from `GrainParams` and all presets that use it.

## Files Changed

| File | Change |
|------|--------|
| `crates/agx/src/adjust/grain.rs` | Remove `chromatic` from `GrainParams`. Add `chromatic` to `GrainTypeConfig`. Remove luminance-only fast path. Scale per-channel perturbation by pixel chroma. Update blending to use correlated noise. Rewrite `apply_grain_buffer_chromatic_shifts_channels_differently` test for new type-driven behavior. |
| `crates/agx/src/engine/mod.rs` | Remove `chromatic` from `PartialGrainParams`, its `merge()`, `materialize()`, and `From<&GrainParams>` impl. |
| `crates/agx/src/preset/mod.rs` | Remove `chromatic` range validation from preset validation function. |
| `crates/agx/src/adjust/README.md` | Remove `chromatic (0-100)` from `GrainParams` documentation. |
| `crates/agx-e2e/fixtures/looks/cinestill_800t.toml` | Remove `chromatic = 25.0` line |
| `crates/agx-cli/src/main.rs` | Remove `--grain-chromatic` CLI flag (if it exists) |
| Golden files | Regenerate all grain-using preset goldens |

## Testing

### Unit tests
- Grayscale pixels get no color shift regardless of grain type chromatic value
- Color pixels get per-channel variation proportional to their saturation
- Higher chromatic types produce more inter-channel variance than lower ones
- Existing luminance grain tests continue to pass

### Visual tuning
- Grid search across chromatic values (0.03-0.20) on color test images
- Verify color shifts look like film (subtle warm/cool) not digital (RGB speckle)
- Verify BW test images are unaffected

### E2E
- Regenerate goldens after final tuning
- cinestill_800t preset updated (chromatic field removed)

## References

- [Grain size fix design](2026-03-27-grain-size-fix-design.md) — parent design for the grain rework
- Film grain color science: color negative film layers (cyan/magenta/yellow dye clouds) develop independently but are spatially correlated from the same exposure
