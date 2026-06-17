# Color spaces

AgX's render pipeline does color math in a wider working space than either the input image or the final output. The pipeline runs in **linear Rec.2020** for physical-light operations and **gamma-encoded Rec.2020** for perceptual operations. This page explains why those two spaces were chosen, what they unlock for wide-gamut inputs, and what's intentionally deferred.

If you want to look up a conversion formula, a matrix, or the per-stage color-space table, see the [color spaces reference](../../reference/concepts/color-spaces.md).

## Why physical operations run in linear space

Physical operations operate on light intensities. They produce correct results only when the values represent linear light.

- **Exposure** simulates changing the amount of light hitting the sensor. Doubling the light means doubling the linear value. The formula `value * 2^stops` only works correctly in linear space.
- **White balance** adjusts the relative intensity of color channels to correct for the color temperature of the light source. This is a physical property of light, so it must operate on linear (physically proportional) values.
- **Dehaze** restores local contrast where atmospheric haze has reduced it. Haze is an additive optical phenomenon — it adds an offset to the physical light reaching the sensor. The math that recovers the original signal works in linear light, not in the perceptually-encoded representation.
- **Noise reduction** smooths sensor noise. Sensor noise is a property of the linear-light signal, not the perceptual one; smoothing in linear space avoids creating perceptual artifacts that gamma-space smoothing would introduce.

The gamut for these operations is **linear Rec.2020**. The core point (physical math needs linear values) is unchanged from any other linear working space; what Rec.2020 changes is the volume of colors the math can carry without clipping.

## Why perceptual operations run in gamma space

Perceptual operations operate on what the image *looks like*. They produce correct results only when the values represent perceptual brightness.

- **Contrast** pushes values away from or toward a midpoint. The "midpoint" that looks right is the perceptual midtone (~0.5 in gamma space), not the physical midpoint (linear 0.5, which looks very bright).
- **Highlights, shadows, whites, blacks** target specific tonal regions. These regions are defined by how they *look* on screen, which means they're defined in the perceptual space.

If you applied contrast in linear space, the result would look wrong: the midpoint would be too bright, and shadows would get crushed while highlights barely change.

The gamut for these operations is **gamma-encoded Rec.2020**. AgX reuses the sRGB transfer-curve shape — the same piecewise function that maps linear 0.18 to roughly 0.5 in gamma space — but applies it to Rec.2020 linear values rather than sRGB linear values. The result: the 0.5 perceptual midtone keeps the same meaning a colorist would expect, just over a wider gamut. A sign-preserving variant of the curve handles the small negative components that arise when wide-gamut inputs (or aggressive edits) push values just outside the Rec.2020 cube on intermediate matrix conversions.

## Why LUT sampling requires a conversion bracket

Creative `.cube` LUTs are almost universally authored in the sRGB-gamma domain. A colorist building a film emulation or grade works with pixel values as they appear on an sRGB display; the input–output mapping baked into the LUT corresponds to sRGB-gamma values, not Rec.2020 values.

To preserve portability — so third-party LUTs work out of the box — AgX converts the buffer to the LUT's declared encoding before sampling and back afterward. For the default `srgb` encoding this is: gamma Rec.2020 → linear Rec.2020 → linear sRGB → sRGB gamma → LUT sample → reverse. For `linear`-encoded LUTs (less common, used in physical-light pipelines), the bracket stops at linear sRGB instead of going through the transfer curve.

The LUT module itself stays gamut-agnostic; the engine handles the wide-working-space round trip on every pixel. See [LUT encoding](lut-encoding.md) for the design rationale behind the two encoding choices.

## What this means for Display P3 photos

The wide working space matters most for inputs that already carry wide-gamut color. iPhone HEIC captures are typically tagged Display P3. Without a wide working space, AgX would have to squash those to sRGB at decode time, discarding wide-gamut information before any adjustment ran.

With linear Rec.2020 as the working space, Display P3 is matrix-converted directly into the working space (P3 is a subset of Rec.2020). Vivid reds, saturated greens, and other wide-gamut colors flow through the entire pipeline unclamped; the final clamp to display gamut happens only at encode.

For sRGB-only workflows — the historical case — the math is functionally unchanged. Input sRGB widens to Rec.2020 at decode, edits happen in the wider space, and encode brings the result back. The end-to-end output looks the same (modulo float rounding) unless the input was actually wide-gamut.

## What hasn't changed

The sRGB perceptual-curve shape stays. The 0.5 midpoint, the 0.25 and 0.75 region splits, and the same tone-slider math carry over unchanged. Existing presets and `.cube` LUTs continue to work without modification. Output is still 8-bit sRGB JPEG, PNG, or TIFF; the wide working space is an internal detail.

## What's intentionally deferred

A few related capabilities are out of scope:

- **ICC profile reading from input images.** Only the NCLX color-primaries tag is consulted today; embedded ICC profiles are not parsed.
- **Wide-gamut output.** Encoded output is sRGB. Rec.2020 or P3 output containers are not produced.
- **HDR transfer curves.** PQ and HLG inputs fall back to "treat as sRGB" at decode with a stderr warning.

Color management is an evolving area; further work is tracked in the project backlog.

## See also

- [Color spaces reference](../../reference/concepts/color-spaces.md) — definitions, conversions, per-stage table.
- [Render pipeline](render-pipeline.md) — where in the pipeline the linear↔gamma conversions happen.
- [LUT encoding](lut-encoding.md) — why the LUT sampling space differs from the pipeline working space.
