# Input ICC Read (color management SP3) Design

## Problem

AgX assumes every non-HEIF input is sRGB. `decode_standard` linearizes pixels
with the sRGB transfer curve and matrix-converts linear sRGB → linear Rec.2020,
ignoring any embedded ICC profile. HEIF decode (`decode/heic.rs`) reads nclx
primaries but explicitly punts on embedded ICC profiles (rICC/prof), warning and
falling back to sRGB.

This is wrong for any wide-gamut input that identifies itself with an ICC
profile rather than nclx tags:

- **Adobe RGB JPEGs** (the common DSLR / Lightroom-export case) decode as if
  they were sRGB. Saturated colors land at the wrong coordinates in the working
  space — greens and reds are pulled in, the whole image is subtly desaturated
  and hue-shifted.
- **ProPhoto RGB TIFFs** (Lightroom 16-bit exports) decode badly wrong, because
  ProPhoto's primaries are far wider than sRGB.
- **Tagged-sRGB inputs** happen to survive (the assumption matches), but only by
  luck.

SP1 widened the working space to linear Rec.2020 specifically so wide-gamut
inputs survive editing. SP2 made output self-identify as sRGB. SP3 closes the
remaining gap on the **input** side: honor the embedded ICC profile instead of
guessing.

Backlog entry: [`docs/backlog/color-management.md`](../backlog/color-management.md),
the "Input ICC read" sub-project. This design covers that sub-project end-to-end.

## Scope

In scope:

- Parse embedded ICC profiles from JPEG (APP2), PNG (iCCP), TIFF (`ICCProfile`
  tag 0x8773), and HEIF (rICC/prof color-profile box).
- Convert input pixels into the engine working space (linear Rec.2020) using the
  parsed profile, via the LittleCMS (`lcms2`) color-management engine.
- Fall back to "assume sRGB" when no profile is present, when the `icc` feature
  is disabled, or when a profile is malformed — preserving current behavior
  byte-for-byte in the no-profile case.
- A new `icc` Cargo feature gating the `lcms2` dependency, enabled by `agx-cli`.
- One new e2e fixture: an Adobe RGB JPEG with goldens.
- Documentation updates (architecture invariant, decode README, existing
  color-profiles book page, backlog checkboxes).

Out of scope:

- **Output gamut choice** (`--output-gamut p3|adobe-rgb`). Tracked as SP4. Output
  remains sRGB after this ships; only the input side gains ICC awareness.
- **CMYK input** beyond best-effort. The upstream decoders (`image`, libheif)
  emit RGB/gray buffers; AgX does not surface CMYK pixel data, so CMYK JPEGs are
  out of practical reach regardless of lcms2's capability. Not a target.
- **HDR PQ/HLG transfer handling.** Parked in `color-management.md` and
  `heic-support.md`; SP3 does not change the BT.2020-transfer punt.
- **Soft proofing / destination-ICC preview.** Parked; depends on SP3 but is a
  separate feature.
- **Per-image rendering-intent selection.** SP3 uses a fixed intent (relative
  colorimetric, no black point compensation); exposing intent as a knob is
  future work if demand surfaces.

## Approach

### Engine choice: lcms2 (LittleCMS)

LittleCMS is the de facto reference color-management engine for open-source
photo software — GIMP, Krita, darktable, RawTherapee, digiKam, Inkscape all use
it. It correctly handles every ICC profile class (matrix/TRC display profiles,
LUT-based A2B/B2A scanner and printer profiles, all rendering intents). For a
photo editor, it is the industry-standard choice; browser-grade subset engines
(`qcms`) deliberately omit the LUT-based and CMYK cases that a photo tool can
encounter.

Three factors make lcms2 the clear pick here:

1. **Standard for the role.** It is what comparable open-source photo editors
   use for exactly this input-ICC-read job.
2. **Already in the workspace.** `agx-profile-gen` (the SP2 dev tool) already
   depends on `lcms2 = "6"` to synthesize the output sRGB profile. Same crate,
   same maintainer, MIT-licensed — consistent with AgX's MIT/Apache posture and
   the SP2 asset-licensing rigor.
3. **No new install burden.** Unlike libheif/libraw (linked from system
   libraries via pkg-config in `build.rs`), the `lcms2` crate vendors and builds
   its own LittleCMS C source. Enabling the `icc` feature does not require the
   user to install anything.

The cost, accepted consciously: lcms2 is a C library, so the `icc` feature adds
a compiled C dependency to the core library at runtime, not just dev tooling.
AgX already links two optional C libraries, so the pattern is established; the
feature gate keeps it off for consumers who don't want it.

Rejected alternatives:

- **`qcms` (pure Rust).** No C dependency, but browser-subset coverage (matrix/
  TRC only, falls back on the exotic profiles a photo tool can hit), MPL-2.0
  weak-copyleft license, and the non-standard choice for this role.
- **Hand-rolled matrix/TRC parsing.** Parse `rXYZ`/`gXYZ`/`bXYZ` colorant tags
  and `rTRC`/`gTRC`/`bTRC` curves ourselves, build the conversion matrix
  directly. Covers the common matrix-profile case with zero dependencies, but
  reimplements — poorly and with subtle-bug risk — what lcms2 already does, and
  still can't handle LUT-based profiles. Not worth it given lcms2 is already
  present.

### Conversion model

The conversion target is the engine working space: **linear Rec.2020**. The
cleanest realization is a single lcms2 transform straight to that space:

- Construct, once, a synthetic **linear Rec.2020 destination profile** (Rec.2020
  primaries, D65 white, linear TRC) — lcms2 builds RGB profiles from primaries +
  gamma. Cache it for the process lifetime.
- For each input, build an lcms2 transform from the parsed input profile to that
  destination profile and run it over the decoded pixel buffer in one pass,
  producing linear Rec.2020 f32 directly. lcms2 handles input-TRC linearization
  and primary conversion internally.
- Rendering intent: **relative colorimetric** (no black point compensation) —
  the standard photographic default. Gamut mapping rarely triggers on input
  conversion anyway, because the Rec.2020 working space contains essentially all
  practical input gamuts, so BPC would add nothing and is left off to keep the
  lcms2 call minimal.

This replaces, for the ICC-present path, the existing "sRGB curve + fixed
sRGB→Rec.2020 matrix" step. The no-profile path keeps that existing step
unchanged.

### Per-format ICC extraction

The conversion logic is format-agnostic; each decoder is responsible only for
pulling the raw ICC bytes out of its container and handing them to the shared
converter:

- **JPEG** — APP2 `ICC_PROFILE` marker chunks, reassembled. Read via `img-parts`
  (already a dependency, already used on the encode side to *write* these chunks).
- **PNG** — `iCCP` chunk, via `img-parts`.
- **TIFF** — `ICCProfile` tag (0x8773), via the `tiff` crate (already a direct
  dependency since SP2).
- **HEIF** — raw color-profile box (rICC/prof). `heic.rs` already detects the
  profile type and currently warns + falls back to sRGB; SP3 reads the raw
  profile bytes (libheif `heif_image_handle_get_raw_color_profile`) and routes
  them through the converter. nclx remains the fast path; the ICC path is used
  when nclx is absent or the declared profile type is rICC/prof.

## API and module layout

New file `crates/agx/src/decode/icc.rs`, gated `#[cfg(feature = "icc")]`:

```rust
//! Input ICC profile parsing and conversion into the engine working space
//! (linear Rec.2020), backed by LittleCMS (lcms2).

/// Convert a decoded RGB f32 buffer from the color space described by
/// `icc_bytes` into linear Rec.2020, in place. Returns `Err` (caller falls
/// back to the sRGB path) on a malformed or unsupported profile.
pub(crate) fn convert_to_working_space(
    buf: &mut Rgb32FImage,
    icc_bytes: &[u8],
) -> Result<()>;
```

(The cached linear-Rec.2020 destination profile lives here as a process-lifetime
singleton. Exact caching mechanism and lcms2 call sequence are an implementation
detail for the plan.)

`decode/mod.rs` — `decode_standard` gains ICC extraction and dispatch:

- Extract ICC bytes (img-parts for JPEG/PNG, `tiff` crate for TIFF).
- If `icc` feature on and bytes present: try `icc::convert_to_working_space`; on
  error, warn and fall through.
- Else: existing sRGB → linear Rec.2020 path, unchanged.

`decode/heic.rs` — the rICC/prof branch of `probe_source_color_space` (or its
caller) reads raw profile bytes and routes through `icc::convert_to_working_space`
instead of warning + sRGB.

`Cargo.toml` (agx):

```toml
[dependencies]
lcms2 = { version = "6", optional = true }

[features]
icc = ["dep:lcms2", "dep:bytemuck"]
```

(`bytemuck` backs the zero-copy reinterpret of the `Rgb32FImage` backing
buffer as `[[f32; 3]]` for the in-place lcms2 transform.)

`Cargo.toml` (agx-cli): add `icc` to the agx feature list
(`features = ["raw", "validate", "heic", "icc"]`). Add `icc` to the docs.rs
metadata feature set as well.

The `icc` feature is intentionally **not** added to the library's `default`
(which stays `["gpu"]`), matching how `raw` and `heic` are handled: enabled by
the CLI so the shipped tool color-manages inputs out of the box, opt-in for
library consumers who want to avoid the C dependency.

## Architecture invariants

`ARCHITECTURE.md` core invariant #3 (working space) currently states the decode
contract as "all back-ends land in linear Rec.2020, input assumed sRGB." SP3
refines the input-space half:

> Input color space is determined by the embedded ICC profile when present
> (parsed via lcms2 behind the `icc` feature) and assumed sRGB otherwise. All
> decode back-ends still land in linear Rec.2020; only the means of getting
> there changes when a profile is present.

This complements rather than alters the working-space contract — the engine math
is unchanged; SP3 only makes the decode step honest about where its pixels came
from.

## Testing

Unit tests in `decode::icc::tests` (gated on the `icc` feature):

- **Known-profile conversion.** Synthesize an Adobe RGB profile (lcms2), feed a
  known pixel, assert the linear Rec.2020 output matches a hand/tool-computed
  value within tolerance.
- **Fallback is byte-identical.** The no-profile decode path produces output
  bit-for-bit identical to the pre-SP3 sRGB path (guards against accidental
  regression for the common tagged-sRGB / untagged case).
- **Malformed profile never panics.** Garbage ICC bytes → `Err` → caller falls
  back to sRGB; no crash. (Pins the "scanner/Lightroom TIFFs with weird ICCs
  decode without crashing" acceptance criterion.)

E2E (`crates/agx-e2e/`):

- New Adobe RGB JPEG fixture with goldens. Synthesize it using the existing e2e
  fixture mechanism — the SP1 Display P3 wide-gamut fixture established the
  pattern for a tagged wide-gamut source with goldens; reuse that path rather
  than introducing a new one. A small authored image tagged with a synthesized
  Adobe RGB profile keeps the fixture deterministic and license-clean, and
  deterministic goldens are preferable for CI to a real capture.

  Supplementary real-world fixture (deferred, not blocking): a real Adobe RGB
  JPEG from a camera makes a good robustness fixture (exercises real camera ICC
  quirks). iPhones do not emit Adobe RGB (they shoot Display P3 HEIC / sRGB);
  Fujifilm and most mirrorless/DSLR bodies do, via the Color Space → Adobe RGB
  menu setting. A backlog note tracks pulling such a capture in as an *additional*
  fixture later; the synthetic fixture carries acceptance and CI on its own.

Architecture tests (`crates/agx/tests/architecture.rs`): `decode/icc.rs` lives
inside the `decode` module; no new cross-module edges, no rule change expected
(verify during implementation).

## Documentation updates

This list is the implementation-phase checklist; adversarial review at the end
verifies every item.

Code-level:

- `crates/agx/src/decode/icc.rs` — module doc: role, lcms2 backing, conversion
  model, fallback contract.
- `crates/agx/src/decode/mod.rs` — `decode_standard` doc updated to describe ICC
  dispatch and the sRGB fallback.
- `crates/agx/src/decode/heic.rs` — doc on the rICC/prof path updated to reflect
  that ICC is now read, not punted.

Module READMEs:

- `crates/agx/src/decode/README.md` — document the input-ICC contract and the
  `icc` feature; extension note: new format decoders should extract ICC and
  route through `decode::icc`.

Architecture:

- `ARCHITECTURE.md` — refine core invariant #3 per "Architecture invariants"
  above.

Book content (`docs/book/src/`):

- `docs/book/src/explanation/color-profiles.md` (created in SP2) — extend with
  the input side: AgX reads embedded ICC profiles and converts to its working
  space. Current-state only; no SP4 teaser.

Backlog:

- `docs/backlog/color-management.md` — check the SP3 sub-task boxes; update
  acceptance text if the shipped scope differs.

## Definition of Done

Per `CLAUDE.md`:

1. `./scripts/verify.sh` passes (with and without the `icc` feature, to confirm
   the gate compiles both ways).
2. `./scripts/e2e-quick.sh` passes.
3. `./scripts/e2e.sh` passes in CI (full matrix, including the new Adobe RGB
   fixture).
4. `ARCHITECTURE.md` invariant #3 refined.
5. `decode/README.md` updated.
6. This design doc lives at `docs/plans/2026-05-30-input-icc-read-design.md`.
7. `explanation/color-profiles.md` extended with the input side.
8. Backlog SP3 boxes checked.
9. PR title: `feat: input ICC read (color management SP3)`.

## Considerations

- **Common case stays free and identical.** Untagged inputs and tagged-sRGB
  inputs — the overwhelming majority — go through the unchanged sRGB path (or an
  sRGB→Rec.2020 conversion that lands in the same place). No goldens move for
  them.
- **lcms2 build cost.** Enabling `icc` compiles LittleCMS's bundled C once. Build
  time and binary size rise modestly; no runtime install dependency. Feature
  gate keeps it optional for library consumers.
- **Rendering intent is fixed for now.** Relative colorimetric (no BPC) is the
  right default for input conversion into a gamut (Rec.2020) that contains
  essentially all input gamuts; out-of-gamut mapping almost never triggers on
  the input side. Exposing intent is deferred unless a concrete need appears.
- **HEIF nclx vs ICC precedence.** Keeping nclx as the fast path and using ICC
  only when nclx is absent or the profile is explicitly rICC/prof avoids
  regressing the common nclx-tagged iPhone path while closing the ICC gap.

  > **Post-implementation note (2026-05-31):** the design above assumed iPhone
  > HEIC captures carry nclx primaries. The actual e2e fixtures
  > (`crates/agx-e2e/fixtures/heic/*.heic`) turned out to embed a **Display P3
  > ICC profile with no nclx tag**, so they decode through the ICC path, not the
  > nclx fast path. This was an unplanned bonus — those fixtures now exercise the
  > HEIF rICC read end-to-end — and is why the HEIC goldens regenerated. The
  > nclx fast path is still real and covered by the synthetic `synthetic_p3_red`
  > fixture (genuine nclx primaries), which did not change.
- **Shared infrastructure with SP4.** SP4 (output gamut choice) needs the inverse
  direction (working space → target gamut + matching ICC embed). The synthetic
  linear-Rec.2020 profile and the lcms2 transform plumbing introduced here are
  directly reusable.
