# HSL Adjustments

**Category:** Editing
**Status:** Backlog

## Problem / Opportunity

Per-channel HSL (Hue, Saturation, Luminance) control lets users target specific color ranges — shift the hue of oranges, desaturate cyans, brighten greens — without affecting the rest of the image. This is a core feature in every professional photo editor and essential for portrait retouching, landscape work, and creative color grading.

## Key Considerations

- Standard 8-channel model: Red, Orange, Yellow, Green, Aqua, Blue, Purple, Magenta
- Each channel needs smooth falloff into neighboring channels to avoid banding
- Requires conversion to HSL (or HSV/HSB) color space for hue-based targeting
- Hue shifts must wrap around the 0°/360° boundary seamlessly
- Operates in sRGB gamma space (perceptual hue targeting matches what users see)
- Interaction with white balance: WB shifts colors globally, HSL provides per-color fine-tuning

## Related

- [Color Grading](color-grading.md) — broader shadow/midtone/highlight color control
- [Tone Curves](tone-curves.md) — complementary tonal control
- [Color Management](color-management.md) — wider gamuts affect which colors HSL can target
