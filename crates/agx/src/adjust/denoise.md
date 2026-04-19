<!-- Canonical source: crates/agx/src/adjust/denoise.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Noise reduction uses the à trous wavelet decomposition to separate
high-frequency noise from structural image content, then thresholds
the noise subband before reconstruction. This overview is intentionally brief.
