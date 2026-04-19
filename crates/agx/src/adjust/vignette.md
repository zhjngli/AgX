<!-- Canonical source: crates/agx/src/adjust/vignette.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Vignette darkens or brightens pixels as a function of their distance
from a configurable center, shaped by an ellipse or circle with
adjustable feather and roundness. Applied as a per-pixel multiplicative
mask in sRGB gamma space.