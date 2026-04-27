# Compose layered looks

AgX can layer multiple presets in a single render. Each preset is applied in order, and later presets override earlier ones at the field level. Useful when you have a base "exposure correction" preset, a colour-style preset, and a finishing-touch preset, and want to apply them as a stack.

## Prerequisites

- AgX installed (see [Install](../install.md)).
- Two or more `.toml` presets you want to layer.

## Steps

Use `--presets` (note the plural) on `agx-cli apply`:

```bash
agx-cli apply \
  -i example/images/sunset_river.png \
  --presets example/presets/golden-hour.toml \
            example/presets/high-contrast.toml \
  -o /tmp/composed.png
```

AgX merges the presets left-to-right. The output is the same as if you had `extends`-chained the second on top of the first, but without committing a merged preset to disk.

## Variations

Mix and match looks until you find a stack you like. Once you do, capture it as an `extends`-chain in a single preset file (see [Extend a preset](extend-preset.md)) so the look becomes reproducible without a long command line.

Layered application versus `extends` inheritance — when to use which:

- `extends` is right when the layering is a property of the look itself — "these are the canonical Portra adjustments and any Portra variant builds on this." Inheritance is captured in the preset file and travels with it.
- `--presets` layering is right when the layering is a property of the invocation — "for this batch I want to stack X and Y, but they're independent looks." The composition is in the command line.

## See also

- [Extend a preset](extend-preset.md) — file-time inheritance.
- [Write your own preset](write-preset.md)
- [Apply a preset to a folder](batch-apply.md) — using a single preset across many images.
- [Preset model concept page](../reference/concepts/preset-model.md)
