<!-- Canonical source: crates/agx/src/adjust/vignette.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Vignette applies a position-dependent multiplicative mask in sRGB gamma
space. The mask is centered on the image midpoint and scales each pixel
by a factor derived from its normalized distance from that center. The
`amount` parameter controls strength: negative values darken edges,
positive values brighten them, and `0.0` is an identity transform.

The geometry is defined by shape. AgX exposes only two user-facing
controls here: `amount` and `shape`. Terms like roundness and center
offset are descriptive, not separate sliders in the current API.
`elliptical` keeps the falloff matched to the image aspect ratio, so
the four edges reach the same normalized distance at their midpoints.
`circular` uses the same radius on both axes, which makes the falloff
perfectly round. On a non-square image, that leaves the short edges less
vignetted than the elliptical mode at their midpoints, while the
corners extend farther beyond the circle boundary and get the strongest
effect. In both cases the center offset is fixed at zero: the effect is
anchored to the image center rather than a movable focal point.

Feather is likewise descriptive here, not a separate exposed parameter.
It names the soft transition between the untinted center and the edge
of the vignette. AgX implements that softness with a simple falloff
curve instead of a separate blur pass: it computes `base = clamp(1 -
d², 0, 1)` and then squares it to get `factor = base²`. That gives a
smooth edge rolloff with a slightly stronger core and avoids a hard ring
at the boundary.

The final per-pixel multiplier is `1 + strength * (1 - factor)`, where
`strength = amount / 100.0`. A factor of `1.0` at the center produces a
multiplier of `1.0`, so the midpoint stays unchanged. As the factor
drops toward `0.0` near the edges, the multiplier approaches `1.0 +
strength`, which darkens or brightens the border uniformly across RGB.

On the CPU path, `VignettePrecomputed::new` caches the image-center
coordinates, the per-axis reciprocal scales, and the normalized
strength. `apply_vignette_pre` then uses those cached values for each
pixel, turning `(x, y)` into a vignette weight with only a few floating
point operations before multiplying the channels and clamping the
result. That keeps the hot path small while preserving the same mask
math for single-pixel and buffer-based calls. The GPU shader uses the
same mask equation, but it recomputes the geometry terms per invocation
instead of sharing a `VignettePrecomputed` struct.
