# Extend a preset

AgX presets compose. A preset can declare an `extends` reference to a base preset; AgX merges the base with the extending preset, with the extending preset's fields taking precedence. Useful for variants on a single look — one base preset captures the common style, and per-image or per-shoot variants override only what they need to.

## Prerequisites

- AgX installed (see [Install](../install.md)).
- A base preset you want to build on.

## A base preset

Save as `looks/base-warm.toml`:

```toml
[metadata]
name = "Warm base"
version = "1.0"

[tone]
exposure = 0.2
contrast = 10.0

[white_balance]
temperature = 35.0
tint = 5.0
```

## A variant that extends the base

Save as `looks/warm-bright.toml`:

```toml
[metadata]
name = "Warm — brighter"
extends = "base-warm.toml"

[tone]
exposure = 0.7
```

When AgX loads `warm-bright.toml`, it resolves `extends` against the same directory, merges fields recursively, and the variant's `exposure = 0.7` overrides the base's `exposure = 0.2`. Every other field (contrast, temperature, tint) inherits from the base.

Apply the variant:

```bash
agx-cli apply \
  -i example/images/sunset_river.png \
  -p looks/warm-bright.toml \
  -o /tmp/warm-bright.png
```

## Variations

- Override one section, leave the rest untouched. Don't write the `[tone]` or `[white_balance]` keys you want to keep — AgX merges, it doesn't replace.
- Chain extensions. A preset can extend another preset that itself extends a base. AgX walks the chain and merges the whole stack.
- Same field at multiple levels: the last-written value wins, with "last" defined as the most-derived preset.

See the [preset model concept page](../reference/concepts/preset-model.md) for the full merge semantics, including how nested fields like HSL channel arrays merge.

## See also

- [Write your own preset](write-preset.md)
- [Compose layered looks](compose-looks.md) — runtime layering instead of file-time inheritance.
- [Preset model concept page](../reference/concepts/preset-model.md)
