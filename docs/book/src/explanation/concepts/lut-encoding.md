# LUT encoding

A `.cube` LUT stores output RGB values for a grid of input RGB values. To sample it correctly, the engine needs to know what color space those input values live in — that is, what the LUT author assumed about the pixel values they were working with. This page explains why encoding matters, what the two choices mean, and when to use each.

## Why encoding matters

A LUT is a lookup table, not a formula. The baked-in mapping between input and output reflects whatever color space the author was working in when they built the LUT. Feed it pixels in the wrong domain and the output is wrong — colors shift, saturation drifts, contrast changes in unexpected ways.

Two encodings cover the common cases:

- **`srgb`** — the LUT author worked with sRGB-gamma pixel values. This is the default and the universal assumption for creative `.cube` LUTs produced in Lightroom, DaVinci Resolve, Capture One, Affinity Photo, and most other editing tools. When a colorist grades a "film look" or exports a LUT from a scoping tool, they are almost always working with display-referred sRGB values.
- **`linear`** — the LUT author worked with linear-light pixel values, using sRGB primaries. This covers LUTs produced by tools that operate in a physical-light pipeline — certain compositing applications, some VFX pipelines, and code-generated LUTs that apply transformations in linear space before baking.

Both encodings assume sRGB primaries. The distinction is only in the transfer curve: gamma-encoded for `srgb`, linear light for `linear`.

## How AgX uses the encoding

AgX's render pipeline operates in Rec.2020 working space. When it reaches the LUT stage, the pixel buffer is not in the LUT's expected domain — it needs to be converted first. The engine does this automatically, based on the encoding declared in the preset:

- For a **`srgb`** LUT, the engine converts the buffer from gamma Rec.2020 to sRGB gamma before sampling, then converts back afterward.
- For a **`linear`** LUT, the engine converts to linear sRGB before sampling, then converts back.

This conversion bracket is transparent to the LUT. From the LUT's perspective, it always receives pixels in the domain it was authored in. Third-party LUTs work out of the box without modification.

## When to use each

For nearly every creative `.cube` LUT — film emulations, color grades, look packages downloaded from the internet — the correct choice is **`srgb`** (the default). If you omit the `encoding` key entirely, `srgb` applies automatically, so existing presets and LUTs continue to work without changes.

Use **`linear`** only when you know the LUT was authored expecting linear-light input values. This is uncommon in practice but does arise with code-generated LUTs or LUTs intended for use in a VFX pipeline's linear-light stage.

When in doubt, `srgb` is the right default. Applying an `srgb` LUT with `linear` encoding (or vice versa) produces obviously wrong results — the image will shift noticeably in contrast and color balance — so testing with a known neutral input makes mismatches easy to spot.

## See also

- [LUT format reference](../../reference/concepts/lut-format.md) — the `.cube` file format, domain, and trilinear interpolation.
- [Preset format reference](../../reference/preset.md) — the `lut.encoding` field.
- [Color spaces](color-spaces.md) — why the engine's working space differs from the LUT's input domain.
