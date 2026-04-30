# Write your own preset

An AgX preset is a TOML file. Every section maps to one stage of the [render pipeline](../reference/concepts/render-pipeline.md), and every field is optional — fields you leave out get their default (no-op) value.

## Prerequisites

- A text editor.
- One sample image you want to test against.

## A minimal preset

Save this as `my-look.toml`:

```toml
[metadata]
name = "My first preset"
version = "1.0"
author = "Your Name"

[tone]
exposure = 0.3
contrast = 12.0
highlights = -25.0
shadows = 20.0

[white_balance]
temperature = 30.0
tint = 5.0
```

Apply it:

```bash ignore
agx apply -i example/images/sunset_river.png -p my-look.toml -o /tmp/my-look.png
```

Open `/tmp/my-look.png`. Tweak any value — re-run — see the change.

## Adding more sections

Each section corresponds to a stage of the render pipeline. The full set:

- `[tone]` — basic adjustments (exposure, contrast, highlights, shadows, whites, blacks). See the [basic adjustments explanation](../explanation/algorithms/basic.md).
- `[white_balance]` — temperature and tint shifts.
- `[hsl]` — per-color hue / saturation / luminance adjustments. See [HSL](../explanation/algorithms/hsl.md).
- `[color_grading]` — split-toning across shadows / midtones / highlights. See [color grading](../explanation/algorithms/color-grading.md).
- `[tone_curve]` — RGB and per-channel tone curves. See [tone curves](../explanation/algorithms/tone-curves.md).
- `[detail]` — sharpening, clarity, texture. See [detail pass](../explanation/algorithms/detail.md).
- `[dehaze]` — haze removal. See [dehaze](../explanation/algorithms/dehaze.md).
- `[noise_reduction]` — luminance and color denoise. See [noise reduction](../explanation/algorithms/denoise.md).
- `[grain]` — film grain simulation. See [grain](../explanation/algorithms/grain.md).
- `[vignette]` — corner darkening or lightening. See [vignette](../explanation/algorithms/vignette.md).
- `[lut]` — apply a `.cube` LUT. See [authoring a custom LUT](custom-lut.md).

Every field's type, valid range, and default is documented in the [preset format reference](../reference/preset.md), generated from the source schema.

## Iterating

AgX presets are plain text — keep your preset under version control. When you're happy with a preset, commit it. When you're auditioning variants, keep them in a `looks/` directory and use [multi-apply](multi-apply.md) to compare side-by-side.

## See also

- [Extend an existing preset](extend-preset.md) — build a variant on top of a base preset.
- [Compose layered looks](compose-looks.md) — combine presets in one render.
- [Preset format reference](../reference/preset.md)
- [Preset model concept page](../reference/concepts/preset-model.md)
