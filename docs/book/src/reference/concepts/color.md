# Color

Color refers to hue and saturation — the chromatic content of pixels, separately from their brightness. The Color group covers the three knobs that shape AgX's chromatic response: white balance (the global cast), HSL (per-color-band edits), and color grading (three-way wheels for tonal regions).

## White balance

Corrects (or creatively shifts) the color cast of a scene. AgX exposes two parameters:

- **Temperature** — shifts the image along the blue-yellow axis, undoing the warm cast of incandescent lighting or the cool cast of overcast daylight.
- **Tint** — shifts along the green-magenta axis, useful for fluorescent and mixed-lighting scenes.

White balance runs in linear-light RGB because color casts are physical properties of the light source.

## Color temperature

A measurement of the color of light, expressed in Kelvin. Lower temperatures are warmer (orange/red, ~2700K candle, ~3200K tungsten), higher temperatures are cooler (blue, ~6500K daylight, ~10000K shade). When you set a white balance temperature, you're telling AgX what color the light source actually was so it can neutralize the cast.

## HSL

Per-color-band adjustments to hue, saturation, and luminance. AgX divides the color wheel into eight bands (red, orange, yellow, green, aqua, blue, purple, magenta) and lets you shift each band's hue (push reds toward orange), modify its saturation (mute blues), or lift/lower its brightness (darken yellows). HSL is the right tool when one color needs different treatment than the others.

## Color grading

Three-way color grading distributes color shifts across tonal regions: shadows get one color, midtones another, highlights a third, with an optional global wheel layered on top. The "blue shadows + orange highlights" cinematic look is the canonical use; AgX also exposes a balance control to bias the regions.

Color grading sits in RGB (sRGB gamma) and uses luminance to weight the regions.

---

See: [Basic adjustments](../../explanation/basic.md) (white balance section), [HSL](../../explanation/hsl.md), and [Color grading](../../explanation/color-grading.md) for the algorithm-level math behind these knobs.

LUTs also produce color transforms; AgX applies them as part of the color stage. See [LUT format](lut-format.md) for what a LUT is and how AgX handles it.
