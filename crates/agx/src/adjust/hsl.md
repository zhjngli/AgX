<!-- Canonical source: crates/agx/src/adjust/hsl.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

HSL adjustments shift hue, saturation, and luminance for up to eight
named color bands (red, orange, yellow, green, aqua, blue, purple,
magenta) using cosine-weighted blending. This overview is intentionally brief.
