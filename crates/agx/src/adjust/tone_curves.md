<!-- Canonical source: crates/agx/src/adjust/tone_curves.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Tone curves remap pixel values via user-defined spline curves (master RGB,
per-channel R/G/B, and luminance-scaled). AgX uses Fritsch-Carlson
monotone cubic hermite interpolation so user curves never create false
reversals.