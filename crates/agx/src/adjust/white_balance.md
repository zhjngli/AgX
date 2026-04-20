<!-- Canonical source: crates/agx/src/adjust/white_balance.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

## How it works

White balance shifts the image's color cast along two axes:

- `temperature` moves the image along the warm/cool axis.
- `tint` moves the image along the magenta/green axis.

The implementation works directly on linear-light RGB values, not on
gamma-encoded sRGB values. That matters because white balance is a
change in channel energy, and channel scaling is only proportional to
light when the data are still linear. Doing the math before gamma
encoding keeps the adjustment physically meaningful and makes the later
tone and color conversions behave predictably.

The temperature and tint inputs are mapped to per-channel multipliers:

```text
r_mult = 1 + temperature / 200
b_mult = 1 - temperature / 200
g_mult = 1 - tint / 200
```

Positive temperature increases red and decreases blue, which produces a
warmer result. Negative temperature does the opposite and cools the
image. Positive tint reduces green, which pushes the image toward
magenta. Negative tint increases green.

Those raw multipliers are then normalized so the adjustment preserves
overall brightness:

```text
sum = r_mult + g_mult + b_mult
norm = 3 / sum

output_channel = max(0, input_channel * channel_mult * norm)
```

The normalization rescales the three multipliers so they still average
to 1.0 under the normal operating range of the control. That keeps a
neutral gray from drifting brighter or darker when the user only wants
to shift color balance, but it is a channel-average normalization, not
an exact perceptual-luminance guarantee. The final clamp prevents the
scaled channels from going negative when a requested shift or invalid
upstream value would otherwise push them below zero.
