# Color Grading

**Category:** Editing
**Status:** Backlog

## Problem / Opportunity

3-way color wheels (shadows, midtones, highlights) with global tint allow users to apply cinematic color grades — teal shadows with orange highlights, cool blue midtones, etc. This is the standard creative color grading tool in Lightroom, Capture One, and DaVinci Resolve.

## Key Considerations

- Each wheel controls hue and saturation for its tonal range (shadows, midtones, highlights)
- Global tint provides an overall color shift on top of per-range grading
- Tonal range boundaries should use smooth crossfades (no hard cuts between shadow and midtone regions)
- Must define the color model for the wheel — polar (angle=hue, distance=saturation) is standard UX
- Pipeline placement: after basic tone adjustments, potentially before or after LUT depending on workflow
- Interacts with white balance (WB corrects, color grading creates)

## Related

- [HSL Adjustments](hsl-adjustments.md) — per-color targeting (complementary tool)
- [Tone Curves](tone-curves.md) — tonal control without color shifts
- [Film and Grain](film-and-grain.md) — film emulation often combines with color grading
