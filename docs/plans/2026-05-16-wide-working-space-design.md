# Wide Working Space (matrix-only) Design

## Problem

AgX edits everything in linear sRGB. The `ARCHITECTURE.md` core invariant #3 codifies this: no working-space conversion, no ICC profile handling. The current contract is leaving real wins on the table:

- **iPhone HEIC inputs** declare Display P3 / BT.2020 primaries via the HEIF nclx tag. The HEIC decoder reads the tag (see `crates/agx/src/decode/heic.rs`) but currently matrix-converts to linear sRGB at decode time, discarding the wider gamut. Vivid P3 reds, the headline iPhone wide-gamut feature, are squashed before any adjustment touches them.
- **Aggressive edits clip prematurely.** The adjust functions in `crates/agx/src/adjust/` clamp every output to `[0, 1]` after every operation. A contrast push that briefly produces 1.2 gets pinned to 1.0 immediately; a later "recover highlights" sees only 1.0 to work with. Headroom is destroyed mid-pipeline.
- **The pluggable pipeline already exposes a `ColorSpace` enum** (`LinearSrgb`, `SrgbGamma`) and auto-inserts conversion stages between adjacent stages — the scaffolding is ready for wider variants.
- **Backlog items are blocked.** The HEIC "wide-gamut preservation" follow-up, the pluggable-pipeline "color-space-aware stage insertion" sub-task, and several known gaps in `docs/backlog/heic-support.md` explicitly wait on a wider working space.

Backlog entry: [`docs/backlog/color-management.md`](../backlog/color-management.md), specifically the "Wide working space (matrix-only)" sub-project. This design doc covers that sub-project end-to-end.

## Scope

In scope:

- Widen the engine's working space from linear sRGB to linear Rec.2020 (primaries from ITU-R BT.2020, D65 white point). All engine math operates in Rec.2020 primaries.
- Apply the existing sRGB transfer curve at the same point in the pipeline it lives today, but to Rec.2020 linear values. This is the same approach Lightroom uses with Melissa RGB — wider primaries, sRGB-like transfer curve.
- Audit every adjust function for tolerance to values outside `[0, 1]`. Remove aesthetic clamps from intermediate stages; narrow domain-safety clamps to the operation that needs them. Hard clip happens once, at encode.
- Convert at decode boundaries for every input path: standard JPEG/PNG/TIFF (assume sRGB), RAW (LibRaw output → matrix), HEIC (use existing nclx info, convert directly to Rec.2020 instead of squashing to sRGB).
- Convert at the encode boundary: linear Rec.2020 → linear sRGB matrix → sRGB transfer curve → clamp → quantize.
- Wrap LUT lookups with sRGB-gamma ↔ Rec.2020-gamma conversions so existing `.cube` files (universally authored against sRGB-gamma) keep their semantics.
- Extend the `ColorSpace` enum with the new variants needed by the pluggable pipeline.
- Flip `ARCHITECTURE.md` invariant #3 to the new working-space contract.
- Add e2e fixtures that exercise wide-gamut preservation (Display P3 source with vivid colors) and heavy-edit headroom (combined wide-gamut + contrast / saturation push).

Out of scope (logged in [`docs/backlog/color-management.md`](../backlog/color-management.md) for future work):

- **General ICC profile parsing** on input or output. Tracked as the input ICC read sub-project. Adobe RGB JPEGs, scanner TIFFs, and other arbitrarily-tagged inputs continue to be misinterpreted as sRGB after this design ships. The wider working space holds them; the parser to detect them is a separate effort.
- **ICC profile embedding** in output JPEG/TIFF/HEIF. Tracked as the output ICC embed sub-project. Outputs remain unlabeled after this design — downstream tools continue to assume sRGB (the correct guess for our sRGB-only output path).
- **Output gamut choice.** Tracked as the output gamut choice sub-project. Output is always sRGB after this design.
- **BT.2020 PQ / HLG HDR transfer curves.** Genuinely hard — requires scene-vs-display-referred semantics and tone mapping. The HEIC decoder continues to fall back to "treat as sRGB and warn" for HDR HEIC, as documented in [`docs/backlog/heic-support.md`](../backlog/heic-support.md). BT.2020 SDR (gamma-encoded) is in scope.
- **Gamut compression on encode.** Today's hard clip survives. On saturated regions clipped back to sRGB output, this can produce flat-mesa artifacts. Deferred under [`docs/backlog/color-management.md`](../backlog/color-management.md) with a list of candidate algorithms (Reinhard, Hable, ACES gamut compress, OKLab-based chromaticity compression).
- **Per-camera DCP profiles.** Orthogonal — improves raw decode color accuracy, not gamut handling. Tracked under [`docs/backlog/processing-parity.md`](../backlog/processing-parity.md).
- **Out-of-gamut-tolerant HSL** (OKHsl, IPT, or JzAzBz). Today's design clamps at the HSL stage entry — the HSL adjustment specifically loses wide-gamut headroom while every other stage benefits. A perceptually-uniform alternative is the future fix. Tracked under the OOG-tolerant HSL deferred sub-task in [`docs/backlog/color-management.md`](../backlog/color-management.md), including the rejected decompose-and-recombine approach captured so future work doesn't re-litigate it.
- **Configurable working space.** Intentionally opinionated. If a future Hasselblad / medium-format use case demands ProPhoto headroom, that is where to revisit this decision.
- **f32 → f64 working buffer.** Intentionally f32; the precision/memory trade-off is documented below.
- **Per-LUT input-domain metadata.** Industry `.cube` files don't carry it; we don't introduce a new metadata convention.

## Working space choice

**Linear Rec.2020, opinionated single choice.**

Candidates considered:

| Space | Gamut size | Holds P3? | Holds Adobe RGB? | "Imaginary" colors? | Notes |
|---|---|---|---|---|---|
| Linear sRGB (today) | smallest | No | No | No | Squashes everything wider. |
| Linear Display P3 | medium | Yes (is P3) | Mostly (misses some Adobe RGB cyans / greens) | No | P3 and Adobe RGB have different shapes; neither fully contains the other. |
| Linear Adobe RGB | medium | Mostly (misses some P3 reds) | Yes (is Adobe RGB) | No | Same shape mismatch — Adobe RGB extends in greens / cyans, P3 in reds. |
| **Linear Rec.2020 (selected)** | wide | Yes, with margin | Yes, with margin | No | Real ITU standard. Used by darktable. |
| Linear ACEScg | wider | Yes | Yes | Minimal | Cinema-grade; slightly wider than Rec.2020. |
| Linear ProPhoto / Melissa | widest | Yes | Yes | ~13% of values | Lightroom's choice. Primaries outside human vision. |

Rec.2020 picked because:

- Encompasses all consumer wide-gamut sources (sRGB, Display P3, Adobe RGB, BT.709, BT.2020 SDR) with margin.
- Well-defined ITU-R BT.2020 standard. Conversion matrices to and from every common space are documented and stable.
- No "imaginary" colors. Tone math has only physically-valid values to reason about. Stage audit is meaningfully easier than it would be against ProPhoto.
- Matches darktable's default, the closest open-source reference.
- Aligns with AgX's preset-first philosophy. A configurable working space means presets become non-portable; a single fixed choice keeps presets portable forever.

**The cost:** a future Hasselblad or scientific-imaging use case might want ProPhoto's headroom. Documented as the revisit-condition; not a current concern.

## Transfer curve choice

**sRGB transfer curve applied to Rec.2020 linear values, at the same pipeline position the existing sRGB conversion lives today.**

Each stage's working-space-half is:

- Stages 1–3 (WB + Exposure, Dehaze, Denoise): linear Rec.2020.
- Stages 5–8 (PerPixelAdjustments, Detail, Grain, Vignette): gamma-encoded Rec.2020 (sRGB transfer curve applied to Rec.2020 linear values).
- Stages 4 (LinearToGamma) and 9 (GammaToLinear): the transfer-curve conversion stages.

Why sRGB transfer curve (rather than e.g. linear-throughout, or a new curve):

- Existing adjust functions in stages 5–8 are anchored around perceptual midpoints (0.5 contrast midpoint, 0.25 shadow split, 0.75 highlight split, sRGB-domain HSL conversion, sRGB-gamma tone curves, sRGB-gamma LUTs). The anchors line up with the *shape* of the sRGB transfer curve. Keeping the same curve shape but applying it on top of Rec.2020 primaries preserves all anchors — no math rewrite needed.
- Linear-throughout (the ACES / cinema-VFX approach) would require rebuilding every anchor: middle gray in linear is ~0.18, not 0.5. Contrast math, highlights/shadows split points, tone curves, LUT semantics, and HSL math would all need redesign. The shift would change the *feel* of every adjustment, even on sRGB-only inputs. Existing AgX presets, LUTs, and goldens would render differently. Out of scope for what is fundamentally a gamut-widening effort.
- Lightroom's Melissa RGB uses this same construction (ProPhoto primaries + D65 + sRGB-like transfer). Industry validation that this composition is sensible.

**The sRGB transfer curve must be sign-preserving** for negative inputs that arise from heavy edits: `sign(x) * srgb_curve(abs(x))`. Stages 4 and 9 use the same implementation. Verify whatever curve impl we use (`palette` crate or our own) honors this extension; if not, write a small wrapper.

## Clamp policy

**Aesthetic clamps removed; domain-safety clamps narrowed to the operation that needs them; final clamp at encode.**

Today, `crates/agx/src/adjust/` clamps every output to `[0, 1]` after every operation. Most of these clamps are doing aesthetic limiting ("fit in output range") — they enforce a `[0, 1]` convention that the rest of the codebase historically assumed. A few are doing genuine domain safety ("don't feed log a negative" or "don't index a LUT outside its declared range"). The audit separates the two.

Clamps to remove (aesthetic, all in `crates/agx/src/adjust/`):

- `basic_tone.rs` lines 12, 25, 38, 51, 64 — contrast, highlights, shadows, whites, blacks output clamps. The math `(0.5 + (v - 0.5) * factor)` and `(v + adjustment)` is well-defined on any input.
- `color_grading.rs` lines 132, 134, 159, 160, 161, 168, 169, 170 — lift/gamma/gain multiplier output clamps. Multiplication is well-defined on any positive value.
- `dehaze.rs` lines 369, 432 — dehaze output clamps. Atmospheric-light estimate uses the input values directly; verify it tolerates out-of-`[0,1]` input (likely fine for additive recombination, may need a guard for the estimator).
- `tone_curves.rs` lines 120, 190, 262, 263, 264 — curve interpolation output and RGB rescale clamps. Curve evaluation extrapolates linearly outside `[0, 1]`; if that is undesired, replace with a documented out-of-range policy (see below).
- `grain.rs` line 355 — additive noise output clamp. Additive perturbation is well-defined.
- `hsl.rs` saturation/lightness shift clamps inside HSL space (lines 87–88) — kept narrowly (see below).

Clamps to keep (domain-safety, narrowed to the specific operation):

- **HSL stage entry.** The RGB → HSL formula assumes input in `[0, 1]`. Clamp at the RGB → HSL conversion only; the rest of the stage operates on clamped values. Wide-gamut input is lost for the HSL adjustment specifically. Documented limitation; OKHsl-style alternative noted as future work.
- **Tone-curve LUT index lookup.** The 256-bin LUT requires `[0, 255]` index. Clamp the index, not the output. Out-of-range curve domain is extrapolated linearly per the existing piecewise-linear interpolation (lines 120, 190); document that policy.
- **3D LUT bracketing.** The 3D LUT is sampled in `[0, 1]` per channel. Clamp the sample coordinate, not the output. Combined with the LUT-wrapping conversions described below.
- **Power functions on negative values** (sRGB transfer curve in stages 4 and 9). The sign-preserving extension `sign(x) * srgb_curve(abs(x))` is the implementation, not a clamp — but documented as a related safety policy.
- **Grain.rs line 371** (`luma.clamp(0.0, 1.0)` used as a weight). This is a weight calculation, not a value pass-through; clamp keeps it.

**Final clamp lives at encode.** After linear Rec.2020 → linear sRGB → sRGB-gamma, a single `clamp(0.0, 1.0)` before quantization handles the projection from working space to display range. Hard clip is the simplest projection and matches today's behavior; smarter projections (gamut compression) are tracked as a deferred follow-up.

## Per-stage migration plan

Pipeline order after SP1 (sub-project 1):

```
decode → linear Rec.2020 → stage 1: WB + Exposure        (linear Rec.2020)
                            stage 2: Dehaze              (linear Rec.2020)
                            stage 3: Denoise             (linear Rec.2020)
                            stage 4: LinearToGamma       (sRGB transfer curve, sign-preserving)
                            stage 5: PerPixelAdjustments (gamma-encoded Rec.2020)
                            stage 6: Detail              (gamma-encoded Rec.2020)
                            stage 7: Grain               (gamma-encoded Rec.2020)
                            stage 8: Vignette            (gamma-encoded Rec.2020)
                            stage 9: GammaToLinear       (sRGB transfer curve, sign-preserving)
                            → linear Rec.2020 → encode
```

Per-stage changes:

| # | Stage | Working space | Math change? | Clamps removed | Clamps kept (narrowed) | Notes |
|---|---|---|---|---|---|---|
| 1 | WB + Exposure | linear Rec.2020 | No | (audit, mostly clean) | — | Channel multipliers + exposure factor well-defined on any positive value. |
| 2 | Dehaze | linear Rec.2020 | No | output clamps (`dehaze.rs:369, 432`) | atmospheric-light estimator clamps (verify still correct) | Verify dark-channel estimator and atmospheric light tolerate signed values. |
| 3 | Denoise | linear Rec.2020 | No | — | — | À trous wavelet decomposition is linear; tolerates any range. |
| 4 | LinearToGamma | transfer | new variant of existing op | n/a | n/a | Same sRGB transfer curve, sign-preserving for negative inputs from heavy edits. |
| 5 | PerPixelAdjustments | gamma Rec.2020 | No (anchors hold) | contrast, highlights, shadows, whites, blacks, color grading output clamps | **HSL entry clamp** (RGB → HSL formula needs `[0,1]`), **tone-curve LUT index clamp** (256-bin LUT), **3D LUT bracketing** | Biggest stage; bulk of clamp removal. See "LUT wrapping" below for the 3D LUT conversion treatment. |
| 6 | Detail | gamma Rec.2020 | No | sharpening output clamps (audit) | — | USM / clarity / texture are linear filters around mean; tolerate any range. |
| 7 | Grain | gamma Rec.2020 | No | `grain.rs:355` (add-noise output clamp) | `grain.rs:371` (`luma.clamp` used as weight, not value) | Additive noise, well-defined; final clamp moves to encode. |
| 8 | Vignette | gamma Rec.2020 | No | output multiplier clamps (audit) | — | Position-weighted multiplier; tolerates any range. |
| 9 | GammaToLinear | transfer | new variant of existing op | n/a | n/a | Mirror of stage 4. |

### LUT wrapping

`.cube` files in the wild are virtually universally authored for sRGB-gamma input domain. AgX's own LUTs (from `agx-lut-gen`) are sRGB-gamma by construction. The format does not carry color-space metadata — input domain is an author convention, not a file fact.

After the working space widens, gamma-encoded Rec.2020 values are not interchangeable with sRGB-gamma values for wide-gamut inputs. Sampling a sRGB-gamma-authored LUT with Rec.2020-gamma values produces subtly wrong outputs.

**SP1 choice: wrap the 3D LUT lookup with conversions.**

```
gamma Rec.2020 → linear Rec.2020 (transfer inverse)
              → linear sRGB    (3×3 matrix)
              → gamma sRGB     (sRGB transfer)
              → LUT lookup
              → gamma sRGB
              → linear sRGB    (inverse transfer)
              → linear Rec.2020 (3×3 inverse matrix)
              → gamma Rec.2020 (sRGB transfer)
```

Per pixel cost: two 3×3 matrix multiplies + four transfer-curve evaluations bracketing the LUT lookup. Implemented as a fused per-pixel loop. LUT semantics stay portable (existing AgX LUTs keep their meaning; third-party `.cube` imports continue to work).

A per-LUT input-domain metadata extension (so a Rec.2020-gamma-authored LUT could opt out of the wrap) was considered and rejected: industry `.cube` files don't carry it, the optimization would benefit only AgX-internal LUTs, and the optimization can be added later behind the same loader API without changing call sites.

## Decode side

The engine sees linear Rec.2020 f32. Decode converts whatever the input file holds.

| Format | Conversion path |
|---|---|
| Standard JPEG / PNG | `u8 sRGB` → `linear sRGB f32` (existing inverse-transfer step) → `linear Rec.2020 f32` (new 3×3 matrix) |
| Standard TIFF (8/16-bit sRGB) | Same as JPEG; existing decode handles bit depth |
| TIFF (linear) | Skip the inverse transfer; matrix to Rec.2020 |
| RAW (via LibRaw) | LibRaw configured for sRGB output (unchanged) → matrix to Rec.2020 |
| HEIC, BT.709 / sRGB nclx | libheif decode → existing transfer-curve handling → matrix to Rec.2020 |
| HEIC, Display P3 nclx | libheif decode → Display P3 transfer-curve handling → **direct matrix to Rec.2020** (no intermediate sRGB squash) |
| HEIC, BT.2020 SDR nclx | libheif decode → BT.2020 SDR transfer-curve handling → identity primaries to Rec.2020 (BT.2020 primaries == Rec.2020 primaries) |
| HEIC, BT.2020 PQ / HLG (HDR) | **Unchanged** — falls back to "treat as sRGB and warn" as today. PQ/HLG transfer-curve handling stays deferred. |

The RAW path is deliberately conservative. LibRaw can output Rec.2020 directly (`output_color = 6`), saving one matrix multiply per pixel. The Rec.2020-output code path inside LibRaw is less well-trodden than its sRGB path; SP1 stays with sRGB output → matrix to keep the comparison surface small. Switching to LibRaw's native Rec.2020 output is recorded as a perf follow-up.

The HEIC path drops today's `apply_matrix` squash-to-sRGB for Display P3 and BT.2020 SDR inputs. The matrices in `crates/agx/src/decode/heic.rs` get replaced with their Rec.2020 destinations (P3 → Rec.2020 and identity for BT.2020 primaries).

ICC profile parsing is **not** added in this design. Inputs without nclx-style format-native signals (i.e., JPEG, PNG, TIFF) continue to be interpreted as sRGB. Adobe RGB JPEGs and similar wait on the input ICC read sub-project.

## Encode side

Single fixed path:

```
linear Rec.2020 f32
       │  3×3 matrix
       ▼
linear sRGB f32          (values may be < 0 or > 1)
       │  sRGB transfer curve (sign-preserving)
       ▼
sRGB-gamma f32           (still possibly outside [0, 1])
       │  clamp(0.0, 1.0)
       ▼
sRGB-gamma f32 ∈ [0, 1]
       │  × 255 + round
       ▼
sRGB u8
```

The conversion lives in `crates/agx/src/encode/`, replacing the existing `linear_to_srgb_rgb8` helper (added recently to fuse the previous decode/encode round-trip allocations). Renamed to a name that makes the input contract obvious — likely `encode_linear_rec2020_to_srgb_rgb8` or similar. Single fused per-pixel pass: matrix → gamma curve → clamp → quantize.

Output format coverage:

- JPEG, PNG (u8 sRGB), TIFF (u8/u16 sRGB): same chain, format-specific quantization step.
- HEIC encode is **not** in scope (tracked under [`docs/backlog/heic-support.md`](../backlog/heic-support.md)).

No ICC profile is embedded after this design — outputs ship unlabeled (same as today). Downstream tools continue to assume sRGB, which is the correct guess for our sRGB-only output. ICC embed is the output ICC embed sub-project.

## Library API contract change

`agx-photo`'s public decode and encode helpers change documented contract:

| Function | Before | After |
|---|---|---|
| `decode_standard`, `decode_raw`, `decode_heic` (returning `Rgb32FImage`) | Linear sRGB f32 | Linear Rec.2020 f32 |
| Engine `Engine::new(image)` / `render()` input + output | Linear sRGB f32 | Linear Rec.2020 f32 |
| Encode entry points (`linear_to_srgb_rgb8` and similar) | Linear sRGB f32 input | Linear Rec.2020 f32 input (renamed) |

Type signatures are unchanged; the documented working-space contract changes materially. **For external consumers of `agx-photo` (currently 0.1.x): this is a breaking behavior change.** Bump to 0.2.0 at next release per pre-1.0 SemVer.

The contract change is documented in the engine, decode, and encode module READMEs and surfaced in the next release notes.

## Working buffer precision

**Continue with f32.** Considered:

| Type | Mantissa bits | Steps in `[0, 1]` | Memory per 26 MP RGB | Notes |
|---|---|---|---|---|
| f16 | 10 | ~1,024 | 156 MB | Insufficient for compute; risk of banding under heavy edits. Fine for storage. |
| **f32 (selected)** | 23 | ~8 M | 312 MB | Industry standard for photo working buffers. |
| f64 | 52 | ~4.5 × 10¹⁵ | 624 MB | 2× memory; SIMD throughput halves; no visible quality benefit. |

f32 cumulative rounding across the nine-stage pipeline is on the order of `1e-6`, four orders of magnitude below the 8-bit output quantization step (~`4e-3`). No banding, no precision issue. f64 would double the memory cost for invisible benefit.

The precision floor doesn't change with the wider working space — Rec.2020 in f32 has the same dynamic range and the same rounding behavior as sRGB in f32. The new wider gamut occupies the same numerical range as the old narrower gamut.

## ColorSpace enum + conversion utilities

`crates/agx/src/engine/mod.rs::ColorSpace` extends from:

```rust
pub enum ColorSpace {
    LinearSrgb,
    SrgbGamma,
}
```

to:

```rust
pub enum ColorSpace {
    LinearRec2020,   // working space for stages 1–3 and as buffer between decode/encode
    GammaRec2020,    // working space for stages 5–8 (sRGB transfer on Rec.2020 linear values)
    LinearSrgb,      // kept for encode-side intermediate
    SrgbGamma,       // kept for encode-side final stage; also LUT-wrap intermediate
}
```

The pluggable-pipeline executor's debug-build color-space assertions extend to the new variants.

Conversion utilities (a new small module, e.g. `crates/agx/src/engine/color_space.rs`):

- `LINEAR_REC2020_TO_LINEAR_SRGB: [[f32; 3]; 3]` — compile-time constant matrix from documented BT.2020 / sRGB primaries.
- `LINEAR_SRGB_TO_LINEAR_REC2020: [[f32; 3]; 3]` — inverse.
- `LINEAR_REC2020_TO_LINEAR_P3`, etc. — for HEIC decode of P3 sources.
- `LINEAR_BT2020_TO_LINEAR_REC2020` — identity matrix (BT.2020 primaries == Rec.2020 primaries) but exists for symmetry.
- `apply_matrix_3x3(buf: &mut [[f32; 3]], m: &[[f32; 3]; 3])` — fused per-pixel multiplication, used by decode boundary, encode boundary, and LUT-wrap.
- `srgb_curve_signed(x: f32) -> f32` and inverse — sign-preserving sRGB transfer for stages 4, 9, encode, and LUT-wrap.

The matrices are derived from published BT.2020 / sRGB / Display P3 primaries (xy chromaticity coordinates) plus the standard D65 white point. They're tested against published reference values (BT.2020 Annex 1 sample points).

## GPU pipeline migration

`crates/agx/src/engine/gpu/` implements the same pipeline in WGSL compute shaders behind the `gpu` feature flag. `ARCHITECTURE.md` invariant #5 requires the GPU pipeline to produce near-identical output to the CPU pipeline. Migrating only the CPU side would break this invariant for wide-gamut inputs, so the GPU work lands alongside the CPU work in SP1.

GPU work in scope:

- **WGSL helpers for matrices and the sign-preserving sRGB transfer curve.** The Rec.2020 ↔ sRGB matrices, Display P3 ↔ Rec.2020 matrix, and `srgb_curve_signed` get WGSL implementations as shared helper functions (e.g. `agx_matrix3x3`, `agx_srgb_curve_signed`). Compile-time constants live in WGSL `const` declarations matching the Rust `[[f32; 3]; 3]` values.
- **Per-stage shader audit.** WGSL stage shaders closely mirror the CPU adjust functions. The same aesthetic-clamp removal applies (per-pixel `clamp(value, 0.0, 1.0)` calls at stage outputs). Domain-safety clamps are similarly narrowed (HSL RGB → HSL conversion entry, tone-curve LUT index, 3D LUT bracketing).
- **WGSL LUT wrap.** The 3D LUT sample stage gets the same gamma Rec.2020 ↔ gamma sRGB conversion bracket as the CPU implementation. Two matrix multiplies plus four transfer-curve evaluations around the texture sample call.
- **Stage 4 / 9 transfer curves.** WGSL implementations of the sign-preserving sRGB curve, used at the linear-to-gamma and gamma-to-linear stage boundaries.
- **Engine input/output contract.** The GPU pipeline accepts linear Rec.2020 f32 and emits linear Rec.2020 f32. Decode and encode remain CPU-only; the buffer handed to the GPU is already in working space, and the buffer read back is in working space.

GPU-specific testing:

- The existing CPU/GPU output parity tests extend across the new fixtures (sRGB neutral, sRGB heavy edit, P3 HEIC neutral, P3 HEIC heavy edit). Tolerance bands remain at their existing CPU/GPU-comparison values.
- Per-stage GPU vs CPU buffer comparison test on the same input catches shader-level drift before it propagates through the pipeline.

The GPU migration adds linear work to the implementation plan but no architectural complexity beyond what's already in the CPU pipeline — each adjust function's GPU shader counterpart updates in lockstep with the CPU change. The risk surface is the same as the CPU audit plus the WGSL-specific concern that some operations behave subtly differently on GPU (e.g. transcendental function precision); the existing CPU/GPU parity test suite has historically caught these.

## Testing strategy

Unit tests in `crates/agx/src/adjust/` and `crates/agx/src/engine/`:

- Per adjust function whose clamp was removed: an out-of-`[0,1]` input test that asserts sensible behavior (not NaN, monotonic where expected, sign-preserving for the relevant ops).
- Per stage that retains a narrowed clamp: assert the clamp is at the right operation (entry to a sub-op) and not at the stage's output.
- `srgb_curve_signed`: `srgb_curve_signed(-x) == -srgb_curve_signed(x)` for x in `[0, 2]`.
- Matrix round-trip: `M_inverse · M · v ≈ v` to within float epsilon for `(LinearRec2020 ↔ LinearSrgb)` and `(LinearRec2020 ↔ LinearP3)`.
- Matrix accuracy: a handful of BT.2020-spec sample-point inputs produce the spec-documented outputs.

Pipeline-level integration tests:

- sRGB JPEG → neutral params → sRGB out: byte-identical or float-rounding-close to today. Verifies the sRGB → Rec.2020 → sRGB matrix path is conservative.
- sRGB JPEG → heavy contrast / saturation push → sRGB out: drift expected, goldens regenerated. Verifies clamp policy change.
- Display P3 HEIC → neutral params → sRGB out: vivid colors visibly preserved relative to today's squash-to-sRGB baseline. Golden compared.
- Display P3 HEIC + heavy edits → sRGB out: the headline-win case. Vivid reds survive into the encode-time clip rather than being clamped at stage 1's output.

Architectural test in `crates/agx/tests/architecture.rs`:

- Assert engine's documented input/output contract equals decode's documented output contract equals encode's documented input contract. Caught by grep + assertion against module documentation; catches future contract drift.

New e2e fixtures in `crates/agx-e2e/`:

- A synthetic Display P3 HEIC with at least one out-of-sRGB-gamut vivid color region (P3 brake-light red region). Encoded sRGB golden.
- A wide-gamut input + heavy-edit fixture exercising headroom and clamp policy together. Encoded sRGB golden.
- Existing matrix re-baselined: all current goldens regenerate once.

Performance benchmarks (per `2026-04-01-render-performance-analysis.md`-style measurement):

- Render time on the canonical 26 MP fixture before vs after.
- Expected drag, broken down by source:
  - **Decode boundary matrix multiply** (linear sRGB → linear Rec.2020 or equivalent per source format): ~10 fmul + 6 fadd per pixel. At 26 MP × 8-core rayon with SIMD, on the order of 1–2% of render time.
  - **Encode boundary matrix multiply** (linear Rec.2020 → linear sRGB): same shape, similar 1–2%.
  - **LUT wrap** (active only when a LUT is applied): 2 matrix multiplies + 4 transfer-curve evaluations per pixel. At 26 MP this is the dominant new cost — estimated ~100–300 ms on a modern multi-core CPU.
  - **Stage 4 / 9 transfer curves**: unchanged from today (same cost on Rec.2020 values as on sRGB values).
- Estimated range: **5–15% for LUT-active workflows, 1–5% for LUT-free workflows.** Pre-measurement estimates; the implementation will benchmark on the canonical 26 MP fixture and document the honest value, per the precedent set by `2026-05-01-render-io-buffer-reduction-design.md` (the "savings were ~0 in apply workflow" note).
- If the regression exceeds 10%, flag SIMD-the-matrix-multiply as a follow-up sub-task under [`docs/backlog/performance.md`](../backlog/performance.md). Do not block this design on it unless the regression exceeds 25%.

## Documentation updates

Per the cross-cutting-change checklist established in `CLAUDE.md` (developer workflow → Design step), every doc that needs an update during implementation:

| File | Update |
|---|---|
| `ARCHITECTURE.md` invariant #3 | Replace "sRGB only — no working-space conversion, no ICC profile handling" with the new contract: "Working space is linear Rec.2020 for stages 1–3 and gamma-encoded Rec.2020 for stages 5–8; decode converts inputs into linear Rec.2020, encode converts linear Rec.2020 to sRGB output. ICC profile handling is still out of scope at this revision." |
| `crates/agx/src/engine/README.md` | New section "Working space contract": primaries, transfer split, value-range expectations per stage, clamp policy. |
| `crates/agx/src/decode/README.md` | Per-format conversion notes: standard (assume sRGB → Rec.2020), TIFF, RAW (LibRaw sRGB → Rec.2020), HEIC (nclx → Rec.2020 direct, including BT.2020 SDR; PQ/HLG fall-back unchanged). |
| `crates/agx/src/encode/README.md` | Encode pipeline: linear Rec.2020 → linear sRGB matrix → sRGB gamma → clamp → quantize. Single fixed path. |
| `crates/agx/src/adjust/README.md` | New section "Clamp policy": aesthetic vs domain-safety clamps; rule that clamps are scoped to the operation that needs them; final clamp at encode. |
| `crates/agx/src/lut/README.md` | LUT input domain assumption is sRGB-gamma; engine wraps lookups with conversions. |
| `docs/book/src/explanation/concepts/color-spaces.md` (new) | End-user page: "Color spaces in AgX — what working space we use and why it matters for wide-gamut photos." Cover: what is a working space, why Rec.2020, what this means for iPhone HEIC photos, what hasn't changed for sRGB-only workflows, what's still deferred. Diataxis explanation quadrant; conforms to `docs/contributing/documentation-conventions.md`. |
| `crates/agx/src/adjust/basic_tone.md` | Note: operates in gamma-encoded Rec.2020; perceptual anchors (0.5, 0.25, 0.75) hold because the transfer curve shape matches sRGB-gamma. |
| `crates/agx/src/adjust/hsl.md` | Note: operates in gamma-encoded Rec.2020 with `[0,1]` clamp on entry to the RGB → HSL conversion. Out-of-`[0,1]` input is clipped before HSL conversion; OOG-tolerant HSL is a future improvement. |
| `crates/agx/src/adjust/color_grading.md` | Note: gamma-encoded Rec.2020; lift/gamma/gain clamps removed. |
| `crates/agx/src/adjust/tone_curves.md` | Note: gamma-encoded Rec.2020; tone-curve LUT index clamp documented as domain-safety; out-of-`[0,1]` extrapolation policy documented. |
| `basic_tone.rs`, `color_grading.rs`, `tone_curves.rs`, `hsl.rs` | Inline comments where math anchors live: brief note "midpoint = 0.5 in gamma-encoded working space; transfer curve shape matches sRGB-gamma, so anchors carry over." |

The adversarial review at the end of implementation verifies each line of this table was followed.

## Risks and mitigations

- **LibRaw native Rec.2020 output path quality is unknown.** SP1 stays with LibRaw → sRGB → matrix to avoid the unknown. Switching to LibRaw's `output_color = 6` is recorded as a perf follow-up that can land independently.
- **Existing AgX-shipped LUTs / presets shift subtly** after clamp removal. Expected behavior. E2e goldens catch the drift; the drift is the intended improvement (less premature clipping).
- **Performance regression** from LUT-wrap conversions and encode-side matrix multiply. Measured during implementation; honest recording per past precedent. SIMD as follow-up if material.
  - **Measured render-time impact:** On the 26 MP `sunset_river.raf × blade_runner.toml` benchmark (CPU PNG, release build, median of 5 runs), the wall-clock delta against the sRGB-only baseline was −0.19% (within noise). User-mode CPU time rose ~10% (matrix-multiply overhead is real) but rayon parallelism absorbs it within the existing budget. SIMD of the boundary matrices is logged as a future optimization under [`docs/backlog/performance.md`](../backlog/performance.md) if single-threaded throughput becomes a constraint.
- **BT.2020 SDR is a new code path** with no existing fixture. Add a synthetic BT.2020 SDR HEIC fixture during implementation. PQ / HLG HDR remains on the "treat as sRGB and warn" fallback.
- **Sign-preserving sRGB curve in third-party impls.** The `palette` crate's sRGB transfer functions may or may not extend to negative inputs. Verify behavior; write a small wrapper if needed.
- **Dehaze atmospheric-light estimator may assume `[0,1]`.** Audit the estimator code path; the dark-channel computation is robust to additive recombination with signed values, but the maximum-finding pass may behave unexpectedly on values > 1. Mitigation: clamp the estimator's intermediate `[0, 1]` range without clamping pixel values.

## Future work captured in the backlog

The following are out of scope for this design but tracked elsewhere for visibility:

- The output ICC embed, input ICC read, and output gamut choice sub-projects of [`docs/backlog/color-management.md`](../backlog/color-management.md) follow this design.
- Gamut compression on encode (smart clip) is captured in the deferred section of `docs/backlog/color-management.md`.
- BT.2020 PQ / HLG (HDR) transfer-curve handling continues to be deferred under [`docs/backlog/heic-support.md`](../backlog/heic-support.md) "Known gaps".
- LibRaw `output_color = 6` perf optimization — to be filed as a sub-task under [`docs/backlog/performance.md`](../backlog/performance.md) when SP1 lands.
- OOG-tolerant HSL (OKHsl as the recommended approach) — captured under the deferred section of [`docs/backlog/color-management.md`](../backlog/color-management.md). The rejected decompose-and-recombine alternative is captured there too so future work doesn't reconsider it.
