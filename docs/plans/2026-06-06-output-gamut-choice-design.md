# Output Gamut Choice (color management SP4) Design

## Problem

AgX edits in a wide internal working space (linear Rec.2020, SP1) and reads
wide-gamut inputs honestly (input ICC, SP3), but the encode step always squashes
the final image into **sRGB** and labels it sRGB (output ICC, SP2). A vivid
Display P3 capture survives the entire pipeline only to be clipped into the
smallest common gamut at the last step.

For a user delivering to a modern screen (iPhone, recent Mac, P3 display) or to a
print workflow (Adobe RGB), that final squash discards saturation the pipeline
worked to preserve. There is currently no way to ask AgX to keep it.

SP4 lets the user choose the output color space at apply time. Default stays
sRGB (universal, unchanged); `p3` and `adobe-rgb` become opt-in targets that
preserve more of the wide-gamut color and label the file accordingly.

Backlog entry: [`docs/backlog/color-management.md`](../backlog/color-management.md),
the "Output gamut choice" sub-project. This design covers that sub-project
end-to-end.

## Scope

In scope:

- A CLI flag `--output-gamut srgb|p3|adobe-rgb` on `apply`, `edit`, `batch-apply`,
  and `batch-edit` (shared via the existing `OutputOpts`). Default `srgb`.
- Convert the rendered linear Rec.2020 image into the chosen output gamut at
  encode, via **fixed per-gamut primary matrices + transfer curves** (no runtime
  color-management engine — see Approach).
- Embed the matching ICC profile (sRGB / Display P3 / Adobe RGB) per output
  format (JPEG APP2, PNG iCCP, TIFF ICCProfile tag), reusing the SP2 embed path.
- Two new committed ICC blobs (Display P3, Adobe RGB), minted offline by the
  existing `agx-profile-gen` dev tool.
- An `icc`-gated test that cross-checks the hand-baked matrices against lcms2 so
  they cannot silently drift.
- E2E goldens for a representative subset of the output-gamut matrix.
- Documentation updates (architecture invariant #4, encode README, profiles
  README, CLI reference, color-management book page, backlog checkbox).

Out of scope:

- **Preset apply-time field for output gamut.** Output gamut is a delivery /
  export concern, not part of the portable "look"; it lives next to `--format`
  and `--quality`, which are also flags, not preset fields. The backlog phrasing
  is "flag **and/or** preset field" — the field is explicitly optional and
  deferred. Revisit if users ask for a set-and-forget per-preset target. (No
  current users; iterate when demand appears.)
- **`multi-apply` output gamut.** That command is a contact-sheet / preview tool
  (PNG, decode-once-render-many); it keeps default sRGB output. Can gain the flag
  later if needed.
- **Smart gamut compression on encode.** Colors outside the chosen output gamut
  are hard-clipped to `[0, 1]`, exactly as sRGB output does today. Smoother
  projection is a separate parked item in `color-management.md`; SP4 does not
  change clipping behavior, it only widens the box the clip happens against.
- **Rec.2020 / HDR output.** No `--output-gamut rec2020` and no PQ/HLG output;
  HDR is parked.
- **Per-image rendering-intent selection.** Not applicable — SP4 uses fixed
  primary matrices, not a CMS transform with an intent.

## Approach

### Conversion mechanism: fixed matrices + curves, not a runtime CMS

The conversion working-space → output gamut is, for these three well-known
boxes, **fully deterministic**: a fixed 3×3 primary matrix plus a fixed transfer
curve, derived from published, frozen standards (sRGB 1996, Adobe RGB 1998,
Display P3 ~2015). There is nothing to compute per image or keep updated. So we
bake the matrices and curves into the code and convert in a single fused pass —
the same shape as today's sRGB encode.

**Decision log — why lcms2 (the SP3 engine) is the wrong tool for the output
side.** lcms2 exists for the *general, unpredictable* case: an input file can
arrive wearing *any* ICC profile — a scanner's LUT-based profile, a custom
printer profile, an oddball curve — none of which reduce to a fixed 3×3. SP3
reads inputs, so it genuinely cannot predict the math ahead of time and must hand
each pixel to lcms2's general machinery. SP4 is the mirror image: **we** choose
the destination, and it is always one of three standard matrix profiles known in
advance. The conversion is therefore precomputable, and routing every output
pixel through a C transform would be pure overhead for no added correctness. It
would also risk perturbing the carefully-tuned default sRGB encode bytes (lcms2's
arithmetic differs slightly from the existing fused path), forcing a needless
golden regeneration. Fixed matrices keep the default path — and its output —
**byte-identical to today.** This matches the backlog's standing Consideration:
"Display P3, Adobe RGB, Rec.2020 can all be handled via fixed primary matrices —
sub-projects 1, 2, and 4 do not need a full ICC engine."

**Not reinventing the wheel.** "Fixed matrices" does not mean hand-deriving color
math and hoping it is right. The matrices are generated/verified **once, offline,
using lcms2 itself** (and the published primaries), then committed as constants
and pinned by an `icc`-gated test that re-derives them through lcms2 on every CI
run. We get library-grade correctness with the speed and stability of constants.
Two pieces already exist in the tree: `LINEAR_P3_TO_LINEAR_REC2020` (the inverse
direction) and the Adobe RGB profile used in `decode::icc` tests — so this is
largely filling in the reverse directions of math already present and proven.

### Per-gamut recipe

Each `OutputGamut` maps to a (matrix, transfer-curve, ICC-blob) triple:

| Gamut | Rec.2020 → target matrix | Transfer curve | ICC blob |
|-------|--------------------------|----------------|----------|
| `Srgb` (default) | `LINEAR_REC2020_TO_LINEAR_SRGB` (existing) | `srgb_curve_signed` (existing) | `SRGB_V4_ICC` (existing) |
| `DisplayP3` | `LINEAR_REC2020_TO_LINEAR_P3` (new — inverse of the existing P3→Rec.2020 matrix) | `srgb_curve_signed` (Display P3 uses the sRGB transfer curve) | `DISPLAY_P3_V4_ICC` (new) |
| `AdobeRgb` | `LINEAR_REC2020_TO_LINEAR_ADOBE_RGB` (new) | `adobe_rgb_curve_signed` (new — gamma 2.19921875 ≈ 563/256, sign-preserving) | `ADOBE_RGB_V4_ICC` (new) |

Key facts pinned here:

- **Display P3 = DCI-P3 primaries + D65 white + the sRGB transfer curve.** It
  reuses `srgb_curve_signed`; only the primary matrix differs from sRGB.
- **Adobe RGB (1998) = Adobe primaries + D65 + a pure gamma 563/256 curve**
  (≈ 2.19921875). This is the one new curve. It is implemented sign-preserving
  (`sign(x) · |x|^(1/2.19921875)`), matching the existing `srgb_curve_signed`
  convention for out-of-range values from heavy edits.
- Quantization (`quantize_u8`, the `[0,1]` clamp + round that mirrors the `image`
  crate's `normalize_float`) is unchanged and shared across all three gamuts. The
  hard clip against the chosen gamut is exactly today's behavior, just against a
  wider box for P3/Adobe.

## API and module layout

### `OutputGamut` enum (`crates/agx/src/encode/mod.rs`)

```rust
/// Output color space (gamut + transfer curve + embedded ICC) for encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputGamut {
    /// sRGB — universal, default. Output is byte-identical to pre-SP4.
    #[default]
    Srgb,
    /// Display P3 — DCI-P3 primaries, D65, sRGB transfer curve.
    DisplayP3,
    /// Adobe RGB (1998).
    AdobeRgb,
}
// + impl Display ("srgb"/"p3"/"adobe-rgb") and FromStr.
```

The core crate has **no `clap` dependency**, so `OutputGamut` implements
`std::str::FromStr` + `Display` (accepting `srgb` / `p3` / `adobe-rgb`) — exactly
the pattern `VignetteShape` and `GrainType` already use. The CLI binds the flag
to this type via `default_value_t = OutputGamut::Srgb`, letting clap parse through
`FromStr`. No serde — the enum is not part of the preset schema.

### Color math (`crates/agx/src/color_space.rs`)

Add the two `Rec.2020 → target` matrices and the Adobe RGB curve:

```rust
pub const LINEAR_REC2020_TO_LINEAR_P3: [[f32; 3]; 3] = /* inverse of LINEAR_P3_TO_LINEAR_REC2020 */;
pub const LINEAR_REC2020_TO_LINEAR_ADOBE_RGB: [[f32; 3]; 3] = /* derived offline via lcms2 */;

/// Adobe RGB (1998) transfer curve, sign-preserving. Encode: sign·|x|^(1/2.19921875).
pub fn adobe_rgb_curve_signed(x: f32) -> f32;
```

(Exact constants are produced offline and pinned by the cross-check test below.)

### Generalized encode (`crates/agx/src/encode/mod.rs`)

`encode_linear_rec2020_to_srgb_rgb8` becomes parameterized on the (matrix, curve)
pair rather than hardcoding sRGB:

```rust
fn encode_linear_rec2020_to_rgb8(
    linear_rec2020: &Rgb32FImage,
    matrix: &[[f32; 3]; 3],
    curve: fn(f32) -> f32,
) -> RgbImage;
```

The fused matrix → curve → `quantize_u8` traversal is unchanged. Calling it with
`(LINEAR_REC2020_TO_LINEAR_SRGB, srgb_curve_signed)` reproduces the current
function exactly (byte-for-byte). A thin mapping `OutputGamut → (matrix, curve)`
selects the pair. The existing public `encode_linear_rec2020_to_srgb_rgb8` name
can be kept as a thin sRGB wrapper for source compatibility, or replaced — an
implementation detail for the plan.

### ICC blob selection (`crates/agx/src/encode/icc.rs`)

```rust
pub(crate) const SRGB_V4_ICC: &[u8] = include_bytes!("profiles/srgb_v4.icc");        // existing
pub(crate) const DISPLAY_P3_V4_ICC: &[u8] = include_bytes!("profiles/display_p3_v4.icc"); // new
pub(crate) const ADOBE_RGB_V4_ICC: &[u8] = include_bytes!("profiles/adobe_rgb_v4.icc");   // new

pub(crate) fn icc_for(gamut: OutputGamut) -> &'static [u8];
```

### EncodeOptions + embed plumbing (`crates/agx/src/encode/mod.rs`)

- `EncodeOptions` gains `output_gamut: OutputGamut` (defaults to `Srgb` via the
  enum's `Default`).
- `encode_to_file_with_options` picks the (matrix, curve) pair and the ICC blob
  from `options.output_gamut`.
- The three embed sites currently hardcoding `SRGB_V4_ICC` take the selected blob
  instead: `inject_jpeg_icc_and_exif` and `inject_png_icc_and_exif` gain an
  `icc: &[u8]` parameter; the inline TIFF `write_tag(Tag::IccProfile, …)` uses
  `icc_for(gamut)`.

### CLI (`crates/agx-cli/src/lib.rs`)

- `OutputOpts` gains `--output-gamut` (bound via `OutputGamut`'s `FromStr` +
  `Display`, `default_value_t = OutputGamut::Srgb`).
- `OutputOpts::encode_options()` sets `output_gamut`. Because `apply`, `edit`,
  and the batch commands all flatten `OutputOpts`, the flag reaches every
  relevant subcommand automatically.
- Batch path (`crates/agx-cli/src/batch.rs`): `run_batch_apply` / `run_batch_edit`
  currently thread `quality` + `format` individually; extend them to also carry
  the gamut (or pass `EncodeOptions` through). Confirm the exact signature during
  implementation.

No new modules, no new cross-module edges; `OutputGamut` and the matrices live in
modules (`encode`, `color_space`) that the CLI and encode path already depend on.

## Architecture invariants

`ARCHITECTURE.md` core invariant #4 currently states output **always** embeds
sRGB. SP4 generalizes it to the chosen gamut while preserving the sRGB default:

> **Encoded output self-identifies with its color space** — every JPEG, PNG, and
> TIFF embeds an ICC profile matching the selected output gamut
> (`--output-gamut`, default sRGB), chosen from a fixed set (`SRGB_V4_ICC`,
> `DISPLAY_P3_V4_ICC`, `ADOBE_RGB_V4_ICC` in `crates/agx/src/encode/icc.rs`).
> Pixel data is encoded into that same gamut via a fixed primary matrix + transfer
> curve, so the embedded profile names the color space the pixels actually live
> in. The embed is unconditional given a gamut and does not depend on input
> metadata. The default (sRGB) is byte-identical to pre-SP4 output.

Invariant #3's half-sentence "encode converts linear Rec.2020 to sRGB output"
updates to "…to the chosen output gamut (default sRGB)."

These refine the output-labeling contract; engine math and the working space are
unchanged.

## Testing

Unit tests (`encode` + `color_space`, the cross-check gated on `icc`):

- **lcms2 cross-check (drift guard, your requested verification).** Behind
  `#[cfg(feature = "icc")]`, build the Display P3 and Adobe RGB profiles via
  lcms2, transform a set of known linear Rec.2020 colors, and assert the
  hand-baked `LINEAR_REC2020_TO_LINEAR_{P3,ADOBE_RGB}` matrices (with the matching
  curve) land within tolerance. If a future edit perturbs a constant, this fails.
- **Default unchanged (zero-churn pin).** Assert
  `encode_linear_rec2020_to_rgb8(img, SRGB matrix, srgb_curve_signed)` is
  byte-identical to the pre-SP4 sRGB output for a fixed input (guards the headline
  promise).
- **Matrix round-trips.** `LINEAR_REC2020_TO_LINEAR_P3` is the inverse of the
  existing `LINEAR_P3_TO_LINEAR_REC2020` within epsilon; same for Adobe RGB once
  its inverse is available (or assert via the lcms2 cross-check).
- **Per-gamut ICC embed.** For each gamut and each format (JPEG/PNG/TIFF), assert
  the embedded ICC equals the expected blob — the SP2 `encode_*_embeds_srgb_v4_icc`
  tests generalize to a per-gamut parameterized form.
- **`agx-profile-gen` self-tests** for each new blob: v4, RGB color space, display
  class, expected size range, and an lcms2 round-trip parse — mirroring the
  existing sRGB blob tests.

E2E (`crates/agx-e2e/`): extend the matrix with a representative subset rather
than the full cross-product. Render the existing wide-gamut Display P3 HEIC
source at `--output-gamut p3` and `--output-gamut adobe-rgb`, commit goldens.
This exercises each output box end-to-end (vivid source → wider output) while
keeping the matrix from ballooning. Exact fixture wiring follows the established
e2e generator pattern; confirm against the harness during implementation.

Architecture tests (`crates/agx/tests/architecture.rs`): no new cross-module
edges expected; verify during implementation.

## Documentation updates

Implementation-phase checklist; the adversarial review at the end verifies each.

Code-level:

- `crates/agx/src/encode/mod.rs` — `OutputGamut` doc; module doc updated from
  "converts to sRGB" to "converts to the chosen output gamut, default sRGB."
- `crates/agx/src/color_space.rs` — doc the two new matrices (source: derived via
  lcms2, pinned by the cross-check test) and `adobe_rgb_curve_signed`.
- `crates/agx-profile-gen/src/main.rs` — module doc covers all three blobs.

Module READMEs:

- `crates/agx/src/encode/README.md` — replace "every output embeds sRGB" with the
  per-gamut selection (default sRGB); note the embed is still unconditional given
  a gamut.
- `crates/agx/src/encode/profiles/README.md` — list the two new blobs + regen.

Architecture:

- `ARCHITECTURE.md` — invariant #4 reworded (above); invariant #3 half-sentence
  tweaked.

Book content (`docs/book/src/`):

- CLI reference (regenerated via `agx-docgen`) gains `--output-gamut`.
- `docs/book/src/explanation/color-profiles.md` — extend with the output side:
  the user can choose the delivery gamut; default sRGB. Current-state prose, no
  project-internal references.
- Add a brief how-to ("export for a wide-gamut display") if it fits the existing
  how-to structure — confirm placement against the Diataxis layout during
  implementation.

Asset licensing:

- `docs/contributing/asset-licensing.md` — the two new blobs are lcms2-generated
  and inherit the same MIT posture as the sRGB blob; add them to the inventory if
  that doc enumerates blobs.

Backlog:

- `docs/backlog/color-management.md` — check the SP4 sub-task boxes; when SP4 is
  the last open sub-project, follow the epic-completion procedure.

## Definition of Done

Per `CLAUDE.md`:

1. `./scripts/verify.sh` passes (with and without the `icc` feature — the
   cross-check test is gated, the rest of SP4 must compile and pass without it).
2. `./scripts/e2e-quick.sh` passes.
3. `./scripts/e2e.sh` passes in CI (full matrix, including the new output-gamut
   goldens).
4. Default sRGB output proven byte-identical (unit + e2e goldens unmoved for
   sRGB).
5. `ARCHITECTURE.md` invariants #4 (and #3 half-sentence) updated.
6. `encode/README.md` and `profiles/README.md` updated.
7. This design doc lives at `docs/plans/2026-06-06-output-gamut-choice-design.md`.
8. `explanation/color-profiles.md` extended with the output side.
9. Backlog SP4 boxes checked.
10. PR title: `feat: output gamut choice (color management SP4)`.

## Considerations

- **Default stays free and identical.** No flag → sRGB → the existing fused path,
  same constants, same bytes. No sRGB golden moves; P3/Adobe goldens are net-new.
- **Display P3's transfer curve is sRGB's.** The only thing separating Display P3
  output from sRGB output is the primary matrix; the curve and quantization are
  shared. Easy to get subtly wrong (e.g. assuming a 2.2 gamma) — pinned by the
  lcms2 cross-check.
- **Hard clip unchanged.** Out-of-output-gamut colors clip to `[0,1]`, as today.
  P3/Adobe simply clip against a larger box, so fewer colors clip. Smooth gamut
  compression remains parked.
- **Reuses SP2/SP3 infrastructure.** The embed path is SP2's; the offline
  blob-generation recipe and the lcms2 dependency are SP2/SP3's. SP4 adds two
  constants-worth of math and one CLI flag — deliberately small.
- **lcms2 only at build/test/offline time for output.** Unlike SP3 (lcms2 at
  decode runtime), SP4's runtime output path calls no C: matrices and curves are
  pure Rust constants. The `icc` feature is needed only for the cross-check test,
  not for `--output-gamut` to work.
