# Color-Space-Aware Pipeline + LUT Encoding Design

**Date:** 2026-06-17
**Branch:** `feat/color-space-aware-pipeline`
**Backlog epic:** [pluggable-pipeline.md](../backlog/pluggable-pipeline.md)

## Goal

Deliver the two remaining sub-tasks of the pluggable-pipeline epic as one coherent
change:

1. **Color-space-aware stage insertion** (the substrate) — the CPU executor inserts
   color-space conversions automatically between adjacent active stages based on each
   stage's declared input/output space, instead of relying on hand-placed conversion
   stages in the fixed list.
2. **LUT input-encoding** (the user-facing feature) — a LUT declares the color
   language it was authored in (`srgb` or `linear`), and the pipeline feeds it pixels
   in that language. This lets looks authored against a linear working space port into
   AgX presets, widening the portable preset language.

These ship together by design ("feature + substrate"): the LUT becomes the first
pipeline stage that declares a non-default color space, so the feature *exercises* the
substrate end-to-end rather than the substrate being built on spec.

**In scope:** generalized executor conversion insertion, a hub-based buffer converter,
LUT-as-its-own-stage, a `LutEncoding` type and preset field, GPU parity for the
encoding feature, the full test + doc + e2e update.

**Out of scope (deferred):** stage-level caching (the other epic sub-task — its only
beneficiary is interactive UI editing, which does not exist yet); named log transfer
curves as LUT encodings (a future language built on the same machinery); a GPU-side
executor abstraction (see Future Considerations).

## Decisions

### Approach 1: the LUT rides the substrate

Two realizations were considered. **Approach 2** would keep the LUT fused inside the
per-pixel stage with an encoding-aware sampling bracket and build executor auto-insert
separately — faster and lower-risk, but the substrate would have no current consumer
(the "plumbing ahead of demand" trap). **Approach 1** (chosen) makes the LUT its own
stage that declares its color space, so the executor's auto-insert mechanism is
genuinely used by the feature. The cost — one extra buffer pass when a LUT is present,
gated so it is free otherwise — is negligible against a 4–17 s render.

### Hub-and-spoke conversion model

Conversions route through a single canonical hub, linear Rec.2020 (the engine's working
boundary). `convert(A → B)` is always `A → hub → B`. Each color space defines only two
hops (to-hub, from-hub), reusing the existing sRGB transfer curves and Rec.2020↔sRGB
matrices in `color_space.rs`. This avoids an N×N conversion table and means a future
space (OKLab, a named log curve) only adds its two hops.

`convert(X, X)` is a true no-op (no buffer touch), so same-language neighbors and
skipped stages introduce no floating-point drift.

The composed hops reproduce today's `wrap_lut_lookup` math exactly: `GammaRec2020 → hub
→ SrgbGamma` is `inv-curve → matrix → curve`, identical to the existing per-pixel
bracket. This is what makes the refactor bit-identical for existing sRGB LUTs, and it
retires `wrap_lut_lookup`.

### LUT declares its encoding in the preset

The `[lut]` preset section gains an optional `encoding` field (`"srgb"` | `"linear"`,
default `srgb`). The `.cube` format has no standard encoding marker, and the preset is
the portable, human-readable home for this metadata. Both encodings assume **sRGB
primaries**; a `linear` LUT is sRGB-primaries linear light (the unambiguous counterpart
to today's sRGB-gamma LUTs). Named log curves (LogC, S-Log, …) are deferred precisely
because "log" is not one curve.

### Stage trait evolution: `StageInputs`

The LUT is not part of `Parameters` — it is threaded separately (a large table shared
via `Arc`). So the LUT stage cannot answer "am I active?" or "what is my color space?"
from `params` alone. The trait's query methods (`is_active`, `prepare`,
`input_color_space`, `output_color_space`) change from taking `&Parameters` to taking a
small `StageInputs { params, lut }` bundle. `process` is unchanged (it already receives
the LUT via `RenderContext`). The update to existing stages is mechanical
(`params` → `inp.params`). This is a deliberate trait/invariant change requiring
`ARCHITECTURE.md` + README updates; the architecture structural test stays green.

### CPU/GPU asymmetry is accepted, not fought

The GPU pipeline has no executor — it is a hand-ordered list of compute dispatches, and
the project's standing decision is "GPU stages are self-contained WGSL; CPU is
canonical; GPU only has to produce matching pixels." The substrate (executor
auto-insert) is therefore a **CPU concept**. The GPU gets the *feature* in its existing
shape: the fused `apply_lut()` bracket in `gamma_adjustments.wgsl` becomes
encoding-aware via a `lut_encoding` flag (linear samples in linear sRGB, skipping the
outer curve). The two paths stay output-consistent within the existing GPU/CPU float
tolerance.

This leaves the engines deliberately asymmetric in structure: CPU is the pluggable
reference architecture, GPU is the optimized mirror. That divergence is recorded as a
tracked future concern (see Future Considerations) — as the CPU pipeline grows more
pluggable, hand-mirroring every new stage/space in WGSL becomes a growing drift risk.

## Design

### ColorSpace and the converter

`ColorSpace` already exists (`LinearRec2020`, `GammaRec2020`, `LinearSrgb`, `SrgbGamma`).
A new buffer converter lives in `color_space.rs`:

```rust
/// Convert a pixel buffer in place from one color space to another, routed
/// through the linear Rec.2020 hub. `from == to` is a no-op.
pub fn convert_buffer(buf: &mut [[f32; 3]], from: ColorSpace, to: ColorSpace);
```

Internally: `to_hub(from)` then `from_hub(to)`, each a per-pixel (rayon-parallel)
transform built from existing curves/matrices. Hub relationships:

| Space | to-hub | from-hub |
|---|---|---|
| `LinearRec2020` | identity | identity |
| `GammaRec2020` | inverse sRGB curve | sRGB curve |
| `LinearSrgb` | matrix sRGB→Rec.2020 | matrix Rec.2020→sRGB |
| `SrgbGamma` | inverse sRGB curve, matrix sRGB→Rec.2020 | matrix Rec.2020→sRGB, sRGB curve |

### Stage trait

```rust
pub struct StageInputs<'a> {
    pub params: &'a Parameters,
    pub lut: Option<&'a crate::lut::Lut3D>,
}

pub trait Stage: Send + Sync {
    fn name(&self) -> &'static str;
    fn input_color_space(&self, inp: &StageInputs) -> ColorSpace;
    fn output_color_space(&self, inp: &StageInputs) -> ColorSpace;
    fn is_active(&self, inp: &StageInputs) -> bool;
    fn prepare(&mut self, inp: &StageInputs);
    fn process(&self, ctx: &mut RenderContext) -> Result<(), AgxError>;
}
```

### Executor (CpuPipeline)

`CpuPipeline::new()` drops the two hand-placed conversion stages. The fixed list
becomes the real stages only:

1. `WhiteBalanceExposureStage` (Linear → Linear)
2. `DehazeStage` (Linear → Linear)
3. `DenoiseStage` (Linear → Linear)
4. `PerPixelAdjustmentsStage` (Gamma → Gamma) — **LUT handling removed**
5. `LutStage` (encoding space → same) — **new**
6. `DetailStage` (Gamma → Gamma)
7. `GrainStage` (Gamma → Gamma)
8. `VignetteStage` (Gamma → Gamma)

`execute()` loop:

```
current = LinearRec2020
build StageInputs from params + lut
prepare() each active stage
for each active stage:
    if current != stage.input_color_space(inp):
        convert_buffer(buf, current, stage.input_color_space(inp))   # timed
        current = stage.input_color_space(inp)
    stage.process(ctx)                                               # timed
    current = stage.output_color_space(inp)
if current != LinearRec2020:
    convert_buffer(buf, current, LinearRec2020)                     # timed
```

Inserted conversions are timed under the `profiling` feature
(`convert: <from>→<to>`). A debug assertion confirms the buffer ends in
`LinearRec2020`.

### LutStage and LutEncoding

```rust
// crate::lut
pub enum LutEncoding { Srgb, Linear }   // default Srgb; serde

pub struct Lut3D {
    // …existing fields…
    pub encoding: LutEncoding,           // default Srgb (back-compat)
}
```

`LutStage` (new `engine/stages/lut.rs`):

- `is_active(inp)` = `inp.lut.is_some()`
- `input_color_space(inp)` = `output_color_space(inp)` = encoding → space
  (`Srgb → SrgbGamma`, `Linear → LinearSrgb`); falls back to `GammaRec2020` when no LUT
  (never runs in that case).
- `process` calls `lut.lookup` directly on the buffer — the executor has already
  converted pixels into the LUT's space, so there is **no internal wrap**.

`PerPixelAdjustmentsStage` drops its `lut_fn` wiring entirely.

### Preset wiring

```rust
pub(crate) struct LutSection {
    pub(crate) path: Option<String>,
    pub(crate) encoding: Option<LutEncoding>,   // new; default srgb at load
}
```

After parsing the `.cube`, the loader sets `lut.encoding` from the section
(default `Srgb`). Invalid encoding strings are a load error. The field flows into the
JSON schema / generated preset reference via the existing `schemars` derives.

### GPU

- `GpuParameters` gains a `lut_encoding: u32` field (0 = srgb, 1 = linear).
- `gamma_adjustments.wgsl` `apply_lut()` branches on it: srgb keeps today's bracket;
  linear samples in linear sRGB (gamma→linear Rec.2020 → matrix → sample → matrix →
  linear→gamma Rec.2020, i.e. without the outer sRGB curve).
- The dispatcher (`gpu/stages/gamma_adjustments.rs`) sets the flag from the LUT's
  encoding. No new compute passes — the GPU LUT stays fused.

## File Layout

```
crates/agx/src/
    color_space.rs                 -- + convert_buffer (hub model); retire wrap_lut_lookup
    lut/mod.rs                     -- + LutEncoding, Lut3D.encoding
    engine/
        mod.rs                     -- Stage trait + StageInputs; doc
        pipeline.rs                -- executor auto-insert; drop hand-placed conv stages
        stages/
            mod.rs                 -- + lut re-export
            lut.rs                 -- NEW LutStage
            per_pixel.rs           -- remove LUT handling
            color_space_conversion.rs -- retire/trim (conversions now via convert_buffer)
        gpu/
            params.rs              -- + lut_encoding
            stages/gamma_adjustments.rs -- set lut_encoding flag
    shaders/gamma_adjustments.wgsl -- encoding-aware apply_lut
    preset/mod.rs                  -- LutSection.encoding + load mapping
```

## Testing

1. **Bit-identical regression (primary guarantee).** Capture render output for
   representative param + sRGB-LUT presets before the refactor; assert exact equality
   after. Proves the executor + LutStage reproduce today's pixels for existing presets.
2. **Unit tests.**
   - `convert_buffer`: identity no-op; each space pair equals the hub composition;
     round-trips; known-value checks.
   - Executor: inserts a conversion only when active neighbors disagree; lands on
     `LinearRec2020`; correct under skipped stages (LUT-only; all-gamma-inactive).
   - `LutStage`: active iff a LUT is present; space follows encoding; samples in the
     declared space.
   - `LutEncoding` serde; preset `[lut] encoding` parsing (srgb / linear / omitted→srgb
     / invalid→error).
3. **GPU consistency.** New CPU-vs-GPU test for a linear-encoded LUT within the existing
   tolerance, beside the current sRGB-LUT consistency tests.
4. **Existing tests** pass untouched (`engine.render()` interface unchanged).
5. **e2e.** Generate a linear-authored `.cube` via `agx-lut-gen`, add a look preset
   referencing it with `encoding = "linear"`, regenerate goldens across the image×look
   matrix. **Every existing sRGB-LUT golden must remain byte-for-byte unchanged** — the
   bit-identical proof at the e2e level. `e2e-quick.sh` then full `e2e.sh`.

## Documentation Updates

- **`ARCHITECTURE.md`** — executor auto-inserts conversions; `Stage` trait signature
  change (`StageInputs`); LUT-as-stage; the recorded CPU/GPU asymmetry decision.
- **`engine/README.md`** — rewrite pipeline-order, Stage-trait, and extension-guide
  sections; add `LutStage`; document the auto-insert rule and that conversions are never
  hand-placed.
- **`lut` module README** — `LutEncoding` and the field.
- **Preset reference / book** — `[lut] encoding` field; a book **explanation** page on
  LUT encodings (sRGB vs linear, the portable-language angle), following
  `documentation-conventions.md` (one quadrant per page, no internal refs).
- **Code comments** where the working-space invariant is anchored (executor loop,
  `convert_buffer`).
- **This design doc** — Future Considerations records the accepted asymmetry.
- **Backlog** — parked `(epic candidate)` item for the GPU divergence (below).

## Invariants

- **Output is linear Rec.2020.** The executor guarantees the final hop back to the hub.
- **Fixed stage order.** The list is hardcoded; the LUT slot is deterministic
  (after per-pixel, before detail) — preserving preset compatibility.
- **Conversions are executor-inserted, never hand-placed.** Stages declare their space;
  the executor routes. `convert_buffer`'s hub composition is the single source of
  conversion truth.
- **Stages may depend on the LUT** via `StageInputs` (trait contract).
- **Adjust module stays pure.** No pipeline awareness, no state.

## Future Considerations

- **GPU pipeline does not share the pluggable architecture.** The substrate is a CPU
  executor concept; the GPU remains a hand-ordered mirror. Each new CPU stage or color
  space must be hand-reimplemented and hand-ordered in WGSL, with no executor or
  auto-insert equivalent. As the CPU pipeline grows more pluggable, this manual
  mirroring is a growing source of CPU/GPU drift and consistency risk. No design yet for
  a GPU-side executor or a shared stage description both paths consume. Tracked as an
  `(epic candidate)` in the pluggable-pipeline backlog.
- **Named log transfer curves as LUT encodings.** `LutEncoding` can grow (LogC, S-Log3,
  Cineon, …) using the same hub-and-spoke machinery — each is a new language with two
  hub hops. Deferred until a concrete curve is needed, because "log" is not one curve.
- **Stage-level caching.** The other pluggable-pipeline sub-task; its only beneficiary
  is interactive UI editing, which does not exist yet. Stays deferred.

## References

- [Pluggable pipeline backlog epic](../backlog/pluggable-pipeline.md)
- [Original pluggable pipeline design](2026-04-01-pluggable-pipeline-design.md)
- [Color management epic](../backlog/color-management.md)
- [ARCHITECTURE.md](../../ARCHITECTURE.md)
