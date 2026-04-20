<!-- Canonical source: crates/agx/src/adjust/exposure.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

## How it works

Exposure is measured in stops. One stop means a factor of two in light
intensity, so the adjustment converts the slider value into a multiplier
with:

```text
factor = 2^stops
```

That gives the expected photographic behavior:

- `0` stops -> `1.0`x, no change
- `+1` stop -> `2.0`x, twice as bright
- `-1` stop -> `0.5`x, half as bright

The code applies that multiplier to each linear-light channel value and
clamps the result at zero:

```text
output = max(0, input * factor)
```

The clamp keeps the result valid when an invalid upstream value would
otherwise push a channel below zero. The exposure multiplier itself is
always positive because it comes from `2^stops`.

Exposure runs in linear space before gamma encoding because stops are a
ratio of light, not a ratio of display-encoded values. If the same
multiplier were applied after gamma encoding, the adjustment would skew
the midtones and no longer behave like a true photographic exposure
change. Applying it before gamma encoding preserves the intended
brightness relationship and lets the later sRGB encoding happen after
the light-level math is done.
