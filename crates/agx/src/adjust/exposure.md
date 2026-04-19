<!-- Canonical source: crates/agx/src/adjust/exposure.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Exposure multiplies linear-light pixel values by `2^stops` to shift the
image brighter or darker in stops. Applied in linear space before gamma
encoding.
