<!-- Canonical source: crates/agx/src/adjust/grain.rs -->
<!-- If you materially change this prose, verify claims against the CPU
     and GPU implementations listed in the Source section below. -->
<!-- If you materially change the algorithm in code, update this file
     so the explanation and implementation stay in sync. -->

Film grain gives digital images a less sterile surface by reintroducing the kind of irregularity that real sensors and film stocks always have. The effect works because the eye reads small, uneven density changes as texture and physical presence rather than as a perfectly smooth synthetic field.

## How it works

`grain.rs` applies grain in sRGB gamma space after the main tonal and detail work. The core pipeline is:

1. Build a deterministic white-noise field from the render seed.
2. Optionally Gaussian-blur that noise with `sigma` derived from `size`.
3. Compute per-pixel luminance and use it to weight the grain strength.
4. Blend the noise back into each RGB channel with a type-specific amount curve.

The current implementation has three grain presets: `GrainType::{Fine,Silver,Harsh}`. Each preset maps to a fixed internal tuple in `GrainTypeConfig`:

- `contrast`: scales the final noise amplitude.
- `luma_falloff`: controls how quickly grain fades from shadows into highlights.
- `chromatic`: controls how much the RGB channels diverge on saturated pixels.
- `amount_curve`: shapes the user-facing `amount` slider before it reaches the noise.

The presets are intentionally simple:

- `Fine` favors subtle texture: lower contrast, steeper luminance falloff, and the lightest chromatic split.
- `Silver` is the default middle ground: moderate contrast, moderate falloff, and balanced chromatic behavior.
- `Harsh` pushes the effect hardest: highest contrast, the weakest falloff, and the strongest chromatic separation.

Internally, the implementation generates one shared noise field plus three channel-specific noise fields. For neutral pixels, the shared field dominates and the grain stays monochrome. For colorful pixels, the channel-specific fields are mixed in more strongly, so the grain picks up the slight color disagreement that real emulsions tend to show.

## Why we chose it

The 2026-03-27 grain-size rework replaced the earlier frequency-based sizing model with blur-based sizing. That change removed the failure mode where extreme size values collapsed into blotchy low-frequency artifacts. The blur approach keeps the visual result effectively the same for normal settings, but it is much easier to reason about and tune.

The 2026-03-29 chromatic-grain work then moved chromatic variation away from a purely digital RGB-noise model and into the grain type itself. Real film layers are correlated, not independent, so a small amount of per-channel decorrelation is enough. That is why the implementation now uses a mostly shared noise field with only a modest type-specific channel split.

## Parameters and constants

The user-facing `GrainParams` fields are `grain_type`, `amount`, `size`, and `seed`. Everything below is internal and fixed in code.

| Constant | Value | Role | Sensitivity |
|----------|-------|------|-------------|
| `GRAIN_PARAM_MIN` | `0.0` | Lower bound for amount/size validation | None |
| `GRAIN_PARAM_MAX` | `100.0` | Upper bound for amount/size validation | None |
| `GRAIN_DEFAULT_SIZE` | `50.0` | Default grain size when omitted | Low |
| `GRAIN_SIZE_CURVE_EXPONENT` | `1.5` | Shapes the size-to-sigma curve | Medium |
| `GRAIN_LUMINANCE_WEIGHT_SCALE` | `0.5` | Scales luminance falloff sensitivity | Medium |
| `GRAIN_BLUR_SIGMA_THRESHOLD` | `0.3` | Skips blur below this sigma | Low |
| `GRAIN_MAX_SIGMA` | `1.0` | Maximum sigma at size 100 | High |
| `GRAIN_REF_RESOLUTION` | `2000.0` | Reference long-edge resolution for sigma scaling | High |
| `GRAIN_STRENGTH_MULT` | `0.04` | Maps amount to the final modulation strength | High |
| `GRAIN_ADDITIVE_END` | `0.1` | End of the additive-grain shadow region | Medium |
| `GRAIN_MULTIPLICATIVE_START` | `0.2` | Start of the multiplicative-grain midtone region | Medium |
| `GRAIN_ADDITIVE_SCALE` | `0.35` | Scales the additive delta in deep shadows | Medium |
| `GRAIN_FALLOFF_REDUCTION` | `0.4` | Reduces luminance falloff as amount rises | Medium |

| GrainType | `contrast` | `luma_falloff` | `chromatic` | `amount_curve` | Reasoning |
|----------|------------:|---------------:|------------:|---------------:|-----------|
| `Fine` | `0.95` | `2.5` | `0.05` | `0.7` | The softest preset. It keeps contrast low, pushes grain out of highlights, and keeps channel decorrelation barely visible. |
| `Silver` | `1.2` | `1.5` | `0.10` | `0.6` | The default stock-like preset. It balances visible grain with enough chromatic separation to feel filmic without looking digital. |
| `Harsh` | `1.5` | `0.8` | `0.15` | `0.5` | The strongest preset. It preserves grain across more of the tonal range and allows the most visible channel disagreement on saturated pixels. |

## Preset-slider mapping

In a preset TOML file, the `[grain]` block uses the serialized keys `type`, `amount`, `size`, and `seed`; that `type` key corresponds to the Rust `GrainParams.grain_type` field. The current implementation does not expose chromatic as a user slider; chromatic intensity is baked into the selected grain type.

`amount` is not linear. The code raises the normalized slider value to the preset's `amount_curve` before scaling the noise, so low settings stay subtle and the effect ramps in more gently than a straight linear blend would. In practice:

- Low `amount` values keep the effect mostly invisible and are useful for a light texture pass.
- Mid-range values produce the classic visible grain look.
- High values become obvious quickly, especially for `Harsh`, because the amount curve is shallower and the contrast multiplier is higher.

`size` controls the Gaussian blur sigma applied to the noise field. Small values leave the noise nearly unblurred, which reads as fine grain. Larger values increase sigma nonlinearly, so the grain grows coarser without collapsing into the low-frequency blobs that the earlier frequency-based algorithm could produce.

## Source

- **CPU (Rust):** [`crates/agx/src/adjust/grain.rs`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/adjust/grain.rs)
- **GPU (WGSL):**
  - [`grain_noise_gen.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/grain_noise_gen.wgsl)
  - [`grain_apply.wgsl`](https://github.com/zhjngli/AgX/blob/main/crates/agx/src/shaders/grain_apply.wgsl)

The CPU and GPU implementations line up on the user-facing controls, luminance weighting, and the non-chromatic preset behavior, but the current GPU path does not yet implement the CPU chromatic-grain split: the CPU mixes a shared noise field with per-channel chromatic noise, while the GPU applies a single noise field.

## References

[^grain-bias]: 2026-03-23 grain design, 2026-03-27 grain size fix design, and 2026-03-29 chromatic grain design.
