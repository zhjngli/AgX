<!-- Canonical source: crates/agx/src/adjust/grain.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Film grain is simulated by convolving white noise with a Gaussian kernel
sized by the grain-size parameter, then modulating by per-pixel
luminance to mimic film's mid-tone-dominant grain response. This overview is intentionally brief.
