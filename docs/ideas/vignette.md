# Vignette

**Category:** Editing
**Status:** Backlog

## Summary

Darken or lighten the edges/corners of an image relative to the center, simulating optical vignetting from real lenses or adding stylistic emphasis.

## Motivation

Vignetting is one of the most common preset adjustments — it draws the viewer's eye toward the center and adds mood. Nearly every film-look preset includes some degree of vignette. It's a natural fit for the preset-first workflow.

## Parameters

- **amount**: Strength of the effect. Negative darkens edges, positive brightens edges. Range: -100 to +100.
- **midpoint**: How far from the center the falloff begins. 0 = starts at center, 100 = only affects extreme corners. Default: 50.
- **roundness**: Shape of the vignette. 0 = more rectangular (follows frame shape), 100 = perfectly circular. Default: 50.
- **feather**: Softness of the transition. 0 = hard edge, 100 = very gradual. Default: 50.

## Vignette Shapes

Several approaches to computing the falloff shape:

- **Circular**: Radial distance from center, producing a round vignette regardless of aspect ratio. Simple but leaves uneven darkening in the corners of non-square images.
- **Elliptical (aspect-matched)**: Ellipse that matches the image's aspect ratio, so the falloff reaches all four edges evenly. This is what most editors default to — it looks natural because it follows the frame.
- **Elliptical (custom)**: User-specified aspect ratio for the ellipse, independent of the image. Allows creative oval shapes.
- **Rectangular with rounded corners**: Follows the frame shape more closely, with a feathered falloff from a rounded rectangle. Less common but matches optical vignetting from some lens/sensor interactions.

The `roundness` parameter could interpolate between rectangular (frame-following) and circular shapes, with the default being an aspect-matched ellipse.

## Implementation

Per-pixel operation — each pixel's adjustment depends only on its `(x, y)` position relative to the image center. No neighborhood access needed. The weight function is a radial/elliptical falloff shaped by the parameters above.

This is the first position-dependent adjustment (existing adjustments are value-only). The render loop would need to pass pixel coordinates to the vignette function, but no architectural changes are required.

## Preset Example

```toml
[vignette]
amount = -25.0
midpoint = 50.0
roundness = 50.0
feather = 75.0
```
