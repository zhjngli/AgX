# Detail

Detail refers to the micro-level texture and edge structure of an image — everything that controls *how sharp*, *how clean*, and *how clear* the small-scale content reads. The Detail group covers four knobs: sharpening (edge contrast), clarity (mid-frequency local contrast), dehaze (long-range local contrast), and noise reduction (the inverse direction).

## Sharpening

Boosts the contrast of edges to make them appear crisper. AgX implements sharpening as an unsharp-mask variant: blur the image, subtract the blur from the original to isolate edges, scale the difference, and add it back. The amount slider controls the gain; the radius controls how local the edges are.

Over-sharpening produces halos around high-contrast edges and amplifies sensor noise — sharpening is best applied with restraint and ideally after noise reduction.

## Clarity / structure

Boosts contrast at the mid-frequency scale — larger than edges (where sharpening operates) but smaller than the whole image. Clarity makes textures pop: stone, fabric, foliage, weathered surfaces. Negative clarity produces a dreamy soft-focus effect.

Clarity is sometimes called "structure" or "texture" in other editors. AgX's detail pass exposes both clarity and a separate texture parameter that target slightly different frequency ranges.

## Dehaze

Removes (or adds) the low-contrast, low-saturation veil that haze, fog, or smog produces over distant subjects. Dehaze increases local contrast and saturation in regions the algorithm identifies as hazy, and decreases them in regions that look clear. Negative dehaze adds the veil back, simulating mist or atmosphere.

Dehaze is the longest-range of the local-contrast tools — it operates over much larger image regions than sharpening or clarity.

## Noise reduction

Reduces the random pixel-to-pixel variation that high-ISO sensors and pushed exposures introduce. AgX separates noise reduction into two channels:

- **Luminance noise** — variation in brightness; appears as grain-like speckling.
- **Chroma noise** — variation in color; appears as red/green/blue blotches in shadows.

Chroma noise is usually more objectionable and easier to remove without losing detail; luminance noise reduction trades sharpness for smoothness, so it's best used in moderation.

---

See: [Detail pass](../../explanation/detail.md) (sharpening + clarity), [Dehaze](../../explanation/dehaze.md), and [Noise reduction](../../explanation/denoise.md) for the algorithm-level math behind these knobs.
