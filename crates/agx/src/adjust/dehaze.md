<!-- Canonical source: crates/agx/src/adjust/dehaze.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Dehaze removes atmospheric haze using a Dark Channel Prior estimate of
transmission, refined by a guided filter. Negative values re-introduce
haze for atmospheric looks.