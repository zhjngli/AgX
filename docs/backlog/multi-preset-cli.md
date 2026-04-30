# Multi-Preset CLI

Add a CLI mode that decodes an image once and applies multiple presets in a single invocation, producing one output file per preset.

## Sub-tasks

- [ ] **Implement `multi-apply` subcommand** — decode once, apply N presets, write N output files
- [ ] **Directory-based mode** — apply all presets in a directory to one input image
- [ ] **Update e2e test harness** — optionally use multi-apply in `run_image_matrix()` to cut test time

## Motivation

The e2e test suite spawns a separate CLI subprocess for each preset applied to an image. For a RAW file, this means LibRaw decoding (the slowest step) runs 12 times for the same image. A multi-preset mode would decode once and apply N presets, reducing decode calls from 48 to 4 for the full RAW test matrix.

## Possible Interface

```bash
# Apply multiple presets to the same image, one output per preset
agx multi-apply \
  -i photo.raf \
  --preset portra_400.toml --output portra.png \
  --preset neo_noir.toml --output noir.png

# Or: directory-based (apply all presets in a directory)
agx multi-apply \
  -i photo.raf \
  --preset-dir looks/ \
  --output-dir results/
```

## Considerations

- The engine already supports re-rendering with different parameters from the same decoded image (always-re-render-from-original invariant).
- LUT files referenced by presets would need to be loaded per-preset, but image decode is the bottleneck, not LUT loading.
- This is primarily a CLI convenience — the library API already supports this pattern (decode once, create multiple engines or reconfigure between renders).
- Could also benefit batch workflows outside of testing.

## Related

- [Performance](performance.md) — reducing redundant decodes is a performance win
- [Parallel CI E2E](parallel-ci-e2e.md) — both reduce e2e test wall-clock time
