<!-- Canonical source: crates/agx/src/adjust/color_grading.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Color grading mixes hue/saturation color wheels into shadows, midtones,
and highlights separately, DaVinci-lift-gamma-gain style. The
sRGB-gamma-space implementation uses smooth luminance masks per zone.
