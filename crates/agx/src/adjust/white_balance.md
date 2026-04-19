<!-- Canonical source: crates/agx/src/adjust/white_balance.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

White balance shifts the image's color cast along temperature (warm↔cool)
and tint (magenta↔green) axes in linear space. Channel multipliers are
normalized to preserve overall brightness. This overview is intentionally brief.
