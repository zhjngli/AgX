<!-- Canonical source: crates/agx/src/adjust/basic_tone.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Basic tone sliders (contrast, highlights, shadows, whites, blacks) each
remap a single channel value in sRGB gamma space, with sensitivity
weights that localize their effect to the relevant luminance band.
