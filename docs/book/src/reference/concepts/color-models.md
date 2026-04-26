# Color models

A color model is a way of describing a color with numbers. AgX uses three: **RGB**, **HSL**, and **luminance**. Each fits a different kind of operation.

## RGB

RGB describes a color by how much red, green, and blue light it contains. The three channels add together (additive synthesis): pure red plus pure green plus pure blue produces white. RGB is the model the input image already uses (decoded from JPEG, PNG, TIFF, or raw demosaic) and the model AgX outputs.

Operations that respond to channel intensities — white balance (per-channel multipliers), exposure (uniform multiplier), per-channel tone curves, LUT lookup — work in RGB.

## HSL

HSL re-parameterizes RGB into three perceptually meaningful axes:

- **Hue** (0°–360°) — the color's position around the color wheel: red at 0°, yellow at 60°, green at 120°, cyan at 180°, blue at 240°, magenta at 300°.
- **Saturation** (0–1) — how vivid the color is, from gray (0) to fully saturated (1).
- **Luminance** (0–1) — how bright the color is, from black (0) to white (1).

HSL is useful when you want to edit one of these properties without disturbing the others. AgX's HSL pass adjusts hue, saturation, and luminance per color band (red, orange, yellow, green, aqua, blue, purple, magenta) — letting you, for example, deepen blues without darkening reds.

## Luminance

Luminance is a single number representing perceived brightness. Human vision is more sensitive to green than to red or blue, so luminance is a weighted sum:

```text
luma ≈ 0.2126 R + 0.7152 G + 0.0722 B   (Rec. 709 weights, used in sRGB)
```

AgX uses luminance as a weighting signal in places where "how bright is this pixel" matters more than "what color is it":

- The grain weight function fades grain in highlights based on luminance.
- The luma channel of the [tone curve](../../explanation/tone-curves.md) operates on luminance, applying the same brightness curve regardless of hue.
- Tonal-region selectors (highlights, shadows, whites, blacks) define their regions by luminance.

## When AgX uses each model

| Operation | Model | Reason |
|-----------|-------|--------|
| White balance | RGB (linear) | Per-channel multipliers correct color cast at the channel level |
| Exposure | RGB (linear) | Uniform multiplier across all channels |
| Contrast, highlights, shadows, whites, blacks | RGB (sRGB gamma) | Targets defined perceptually; tonal regions are selected by luminance |
| Tone curves (RGB channel) | RGB | Per-channel response shaping |
| Tone curves (luma channel) | Luminance | Brightness shaping that preserves hue and saturation |
| HSL | HSL | Selective edits per color band |
| Color grading | RGB (per tonal region defined by luminance) | Three-way wheels operate on RGB; their region weights come from luminance |
| LUT | RGB (sRGB gamma) | LUT is a 3D RGB→RGB mapping |
| Grain weighting | Luminance | Strength varies with brightness |

## See also

- [Color spaces](color-spaces.md) — the linear-vs-gamma encoding choice that sits underneath the model choice.
- [HSL](../../explanation/hsl.md) — algorithm explanation for the HSL pass.
- [Tone curves](../../explanation/tone-curves.md) — algorithm explanation, including the luma channel.
