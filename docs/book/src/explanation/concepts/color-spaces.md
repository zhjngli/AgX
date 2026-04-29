# Color spaces

The render pipeline does math on pixel values in two related color spaces: linear sRGB and sRGB gamma. This page explains why each stage runs in the space it runs in, what working sRGB-only costs, and what wider gamut support would unlock.

If you want to look up a conversion formula or the per-stage color-space table, see the [color spaces reference](../../reference/concepts/color-spaces.md).

## Why physical operations run in linear space

Physical operations operate on light intensities. They produce correct results only when the values represent linear light.

- **Exposure** simulates changing the amount of light hitting the sensor. Doubling the light means doubling the linear value. The formula `value * 2^stops` only works correctly in linear space.
- **White balance** adjusts the relative intensity of color channels to correct for the color temperature of the light source. This is a physical property of light, so it must operate on linear (physically proportional) values.
- **Dehaze** restores local contrast where atmospheric haze has reduced it. Haze is an additive optical phenomenon — it adds an offset to the physical light reaching the sensor. The math that recovers the original signal works in linear light, not in the perceptually-encoded representation.
- **Noise reduction** smooths sensor noise. Sensor noise is a property of the linear-light signal, not the perceptual one; smoothing in linear space avoids creating perceptual artifacts that gamma-space smoothing would introduce.

## Why perceptual operations run in sRGB gamma space

Perceptual operations operate on what the image *looks like*. They produce correct results only when the values represent perceptual brightness.

- **Contrast** pushes values away from or toward a midpoint. The "midpoint" that looks right is the perceptual midtone (sRGB 0.5), not the physical midpoint (linear 0.5, which looks very bright).
- **Highlights, shadows, whites, blacks** target specific tonal regions. These regions are defined by how they *look* on screen, which means they're defined in the perceptual space.

If you applied contrast in linear space, the result would look wrong: the midpoint would be too bright, and shadows would get crushed while highlights barely change.

## Why LUTs run in sRGB gamma space

LUTs are created by colorists while looking at a screen displaying sRGB. When a colorist tweaks a film emulation LUT, they're working with pixel values as they appear on screen (sRGB gamma). The input-output mapping in the LUT corresponds to sRGB values, not linear light values.

Applying a LUT designed for sRGB input to linear values would produce incorrect colors. AgX applies LUTs in sRGB gamma space, which is correct for the vast majority of creative `.cube` LUTs.

## Working sRGB-only

AgX currently works exclusively in sRGB color space — the standard color space for displays, web, and consumer photography. This is a deliberate scoping decision, not a placeholder.

Working in sRGB constrains the gamut to what an sRGB display can show. The trade-off is simplicity and a clean match to the input format that consumer cameras and editing software already use. Wider working spaces (Adobe RGB, ProPhoto RGB, Display P3) can hold colors that would clip in sRGB, at the cost of more careful color management — gamut handling, ICC profile reading, profile embedding, and out-of-gamut clipping policies.

For the current scope:

- Decoded images (JPEG, PNG, TIFF) are assumed to be sRGB.
- No ICC profile reading or embedding.
- No wide-gamut support.

## What wider color spaces would add

Future versions may add wider working spaces:

- **Adobe RGB** — wider gamut for professional print workflows (more greens and cyans).
- **ProPhoto RGB** — very wide gamut used internally by Lightroom for lossless editing.
- **Display P3** — Apple's wide-gamut display standard.
- **ICC profile handling** — read embedded profiles from input images, embed profiles in output.
- **Log input LUTs** — support for LUTs designed for video log curves (S-Log3, LogC).

Each of these would require gamut-aware color math at every stage and a policy for handling out-of-gamut values.

## See also

- [Color spaces reference](../../reference/concepts/color-spaces.md) — definitions, conversions, per-stage table.
- [Render pipeline](render-pipeline.md) — why pipeline order matters in concert with the color-space rule.
