# Multi-Preset CLI Mode

## Summary

Add a CLI mode that decodes an image once and applies multiple presets in a single invocation, producing one output file per preset.

## Motivation

The e2e test suite currently spawns a separate CLI subprocess for each preset applied to an image. For a RAW file, this means LibRaw decoding (the slowest step) runs 10 times for the same image. A multi-preset mode would decode once and apply N presets, reducing decode calls from 50 to 5 for the full RAW test matrix.

Measured impact: a single JPEG decode + render takes 2.6s in release mode. RAW is significantly slower due to LibRaw demosaicing. Eliminating redundant decodes would cut e2e test time roughly in half.

## Possible Interface

```bash
# Apply multiple presets to the same image, one output per preset
agx-cli multi-apply \
  -i photo.raf \
  --preset portra_400.toml --output portra.png \
  --preset neo_noir.toml --output noir.png

# Or: directory-based (apply all presets in a directory)
agx-cli multi-apply \
  -i photo.raf \
  --preset-dir looks/ \
  --output-dir results/
```

## Considerations

- The engine already supports re-rendering with different parameters from the same decoded image (always-re-render-from-original invariant).
- LUT files referenced by presets would need to be loaded per-preset, but image decode is the bottleneck, not LUT loading.
- This is primarily a CLI convenience — the library API already supports this pattern (decode once, create multiple engines or reconfigure between renders).
- Could also benefit batch workflows outside of testing.
