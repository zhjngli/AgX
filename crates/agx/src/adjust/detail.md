<!-- Canonical source: crates/agx/src/adjust/detail.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

The detail pass covers sharpening (high-pass on luminance, unsharp-mask
style), clarity (mid-frequency local contrast), and texture (fine-scale
local contrast). All three operate on luminance only to avoid colored
halos.