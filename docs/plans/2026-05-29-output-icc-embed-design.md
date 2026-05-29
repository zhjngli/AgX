# Output ICC Embed (sRGB default) Design

## Problem

AgX encodes every output as sRGB-quantized 8-bit RGB but writes no ICC profile. Downstream tools (Preview, Photoshop, browsers, Lightroom) fall back to their own assumption — usually sRGB on legacy paths, sometimes Display P3 on wide-gamut displays, sometimes "whatever the embedded EXIF Color Space tag suggests." The output looks correct in the common case but inconsistent in the wide-gamut-display case, and is not authoritative either way.

Two concrete consequences:

- **Wide-gamut viewer ambiguity.** A macOS user viewing an AgX output JPEG on a P3 display sees a guess: most apps guess sRGB and render correctly; some guess P3 and render oversaturated. There's no tag in the file to resolve the question.
- **Latent post-SP1 bug.** [Wide working space (SP1)](2026-05-16-wide-working-space-design.md) widened the engine to linear Rec.2020 and the engine still encodes output as sRGB. But the metadata pipeline pass-through (`ImageMetadata.icc_profile`) preserves the *input* ICC. iPhone HEIC input carries a Display P3 ICC; the engine converts pixels Rec.2020 → sRGB at encode; the encoder then re-injects the original P3 ICC into the sRGB-encoded output. The file then claims P3 while containing sRGB-encoded pixels — viewers honoring the tag render the wrong colors. This bug is mostly latent today because the most-common output viewers (web browsers, Preview default) ignore ICC for JPEGs without dedicated profile tags, but it surfaces immediately on profile-aware viewers.

Backlog entry: [`docs/backlog/color-management.md`](../backlog/color-management.md), specifically the "Output ICC embed (sRGB default)" sub-project. This design doc covers that sub-project end-to-end.

## Scope

In scope:

- Embed a vetted sRGB v4 ICC profile (~3 KB) into every JPEG, PNG, and TIFF output, unconditionally.
- Remove the `icc_profile` field from `ImageMetadata`. ICC is no longer an input-pass-through concern — it is an output-labeling concern owned by the encoder.
- Fix the post-SP1 latent input-ICC pass-through bug as a consequence of the above (encoder no longer reads input ICC; always writes sRGB blob).
- Promote the `tiff` crate from transitive (via `image`) to a direct dependency so the TIFF encoder can write the `ICCProfile` (0x8773) tag during encode. No change to the resolved dependency graph — the crate is already pulled in transitively at version 0.10.
- New code lives in `crates/agx/src/encode/icc.rs` plus `crates/agx/src/encode/profiles/srgb_v4.icc`.
- Update `ARCHITECTURE.md` with an output-labeling invariant.
- Update `docs/backlog/color-management.md` SP2 acceptance text (drop HEIF and opt-out-flag references).
- New book explanation page covering ICC profiles and the open-source profile licensing landscape.

Out of scope:

- **HEIF output.** No HEIF encoder exists in AgX today. Tracked under the "HEIF encode (future)" parked item in [`docs/backlog/heic-support.md`](../backlog/heic-support.md); the ICC-embed concern is absorbed by that future work. Backlog cross-reference added during implementation.
- **Wider output gamut choice (P3, Adobe RGB).** Tracked as the output gamut choice sub-project in [`docs/backlog/color-management.md`](../backlog/color-management.md). Output remains sRGB-only after this design ships; the new ICC blob is sRGB v4 unconditionally.
- **Input ICC parsing for working-space conversion.** Tracked as the input ICC read sub-project. AgX continues to assume sRGB for inputs without nclx tags after this design.
- **CLI opt-out flag.** Initially scoped per the backlog ("opt-out flag for size-sensitive workflows"), then dropped after design discussion: the ~3 KB per-file overhead is small, hypothetical "size-sensitive workflow" demand has no concrete user, and `exiftool -icc_profile=` strips ICC post-hoc. Adding the flag later is a one-line change if real demand surfaces.
- **`embed_icc: bool` library API.** Same reasoning as CLI flag. If library consumers need to skip ICC for a real reason, the field can be added without API break (default behavior preserved).
- **Generation of the sRGB blob from primaries at build time.** Rejected — adds complexity for one blob with no real upside. Defer the question to SP4, where multiple blobs may make a generator worthwhile.

## Approach

ICC embed happens at the encoder layer. The encoder unconditionally writes a known sRGB v4 ICC profile, regardless of input. This is the structurally correct division: input metadata (EXIF) is forwarded from the source file; output color labels are decided by what the encoder produced.

The sRGB v4 ICC blob is shipped as a static asset under `crates/agx/src/encode/profiles/srgb_v4.icc`, embedded into the binary via `include_bytes!`, and referenced through a single `pub(crate) const SRGB_V4_ICC: &[u8]` in a new `encode/icc.rs` module. No runtime profile generation, no I/O. The blob is loaded from a CC0-licensed source (Elle Stone profile set, specifically `sRGB-elle-V4-srgbtrc.icc` or equivalent) — see "Blob source and licensing" below.

`ImageMetadata.icc_profile` is removed. Callers — `extract_metadata*`, `inject_metadata` — stop reading the field. The field's removal is a pre-1.0 API break, called out explicitly in the PR description. Within the workspace, only `agx-cli` and `agx-e2e` reference `ImageMetadata`, neither directly reads `.icc_profile`. There are no external consumers (no published crate releases depend on this field).

The encoder's per-format paths are updated as follows:

- **JPEG.** `img_parts::jpeg::Jpeg::set_icc_profile(SRGB_V4_ICC)` writes the standard APP2 marker chunks with the `ICC_PROFILE\0` signature, splitting across multiple markers as needed. Existing path; the only change is dropping the conditional `if let Some(icc) = metadata.icc_profile` and always passing the const.
- **PNG.** `img_parts::png::Png::set_icc_profile(SRGB_V4_ICC)` writes the `iCCP` chunk. Existing path; same conditional drop as JPEG.
- **TIFF.** New path. The current `image::codecs::tiff::TiffEncoder` does not expose tag injection. The `tiff` crate (already a transitive dep via `image`) is promoted to a direct dependency and used directly. The new `tiff::encoder::TiffEncoder<W>` API allows `write_tag(Tag::ICCProfile, SRGB_V4_ICC)` during encode via the `DirectoryEncoder` returned from `TiffEncoder::new_image()`. The `OutputFormat::Tiff` branch in `encode_to_file_with_options` switches from the `image`-wrapped encoder to the direct `tiff`-crate path.

The `inject_metadata_tiff` step (which currently writes EXIF via `little_exif` post-encode) continues to handle EXIF only. ICC is written during initial TIFF encode, not post-hoc.

## Blob source and licensing

ICC profile files are copyrighted as data (similar in legal treatment to font files). Shipping a profile in a binary without a clear license is a real legal concern for downstream users of an MIT/Apache-2.0 library.

The selected blob is `sRGB-elle-V4-srgbtrc.icc` from Elle Stone's public-domain profile set (CC0). This is the de facto open-source standard sRGB v4 profile — used by GIMP, darktable, RawTherapee. ~3 KB. Profile version 4, profile class "mntr" (display), color space "RGB ".

Alternatives considered:

- The ICC Consortium's `sRGB_v4_ICC_preference.icc` is authoritative but ~60 KB (includes perceptual-rendering-intent data we don't need) and has historically-unclear licensing.
- lcms2 ships sRGB profiles under MIT — equivalent option if Elle Stone's blob proves problematic.
- HP / Microsoft / ICC Consortium reference sRGB v2 is universal but older and has the same licensing ambiguity.

Implementation actions for blob acquisition:

1. Locate exact source file and verify CC0 license claim.
2. Validate the blob with `iccdump` / `exiftool`: profile version `0x04xxxxxx`, profile class "mntr", color space "RGB ", size 3–8 KB.
3. Round-trip test through lcms2 (`tificc` or equivalent): confirm sRGB → sRGB transform is identity within float epsilon.
4. Commit blob at `crates/agx/src/encode/profiles/srgb_v4.icc`.
5. Document source and license in `crates/agx/src/encode/icc.rs` module-level doc comment.

Fallback path if Elle Stone source proves unavailable or unclean: generate compact v4 sRGB via lcms2's profile generator at dev time, commit the output. The lcms2 generator's output is treated as MIT per the lcms2 license FAQ.

## API and module layout

`ImageMetadata` simplification:

```rust
// Before
pub struct ImageMetadata {
    pub exif: Option<Vec<u8>>,
    pub icc_profile: Option<Vec<u8>>,
}

// After
pub struct ImageMetadata {
    pub exif: Option<Vec<u8>>,
}
```

New file structure under `crates/agx/src/encode/`:

```
encode/
├── mod.rs              (existing, modified — see per-format notes above)
├── icc.rs              (new — blob const + TIFF tag helper docs)
├── profiles/
│   └── srgb_v4.icc     (new — ~3 KB blob)
└── README.md           (existing, updated)
```

`encode/icc.rs` public surface (all `pub(crate)` — encoder is the only consumer):

```rust
/// sRGB v4 ICC profile, embedded at compile time.
///
/// Source: Elle Stone profile set, sRGB-elle-V4-srgbtrc.icc (CC0).
/// See `profiles/srgb_v4.icc` for the raw bytes.
pub(crate) const SRGB_V4_ICC: &[u8] = include_bytes!("profiles/srgb_v4.icc");
```

(JPEG and PNG injection use the existing `img_parts` helpers; no new wrapper functions needed. TIFF writes the tag inline via the `tiff` crate's `DirectoryEncoder::write_tag` — no wrapper needed there either.)

`Cargo.toml` change (single line addition):

```toml
tiff = "0.10"  # already in the resolved graph via `image`
```

`EncodeOptions` is unchanged. The encoder unconditionally embeds sRGB ICC; no new field, no opt-out.

## Architecture invariants

`ARCHITECTURE.md` gains an output-labeling invariant (or extends an existing encoder-side invariant — exact wording decided during implementation). Suggested text:

> **Encoded output identifies its color space.** Every JPEG, PNG, and TIFF file produced by the encode pipeline embeds an sRGB v4 ICC profile. This is unconditional and does not depend on input metadata. Pixel data is sRGB-encoded; the embedded profile names the same color space, so downstream tools render correctly without guessing.

This invariant sits adjacent to invariant #3 (working space), which it complements: #3 says "engine math is in linear Rec.2020"; the new invariant says "encoded output is sRGB and self-declares as such."

## Testing

Unit tests in `encode::icc::tests`:

- `srgb_v4_icc_blob_is_valid_v4` — parse first 128 bytes of the blob, assert profile version field starts with `0x04`, profile class = `mntr`, color space = `RGB `.
- `srgb_v4_icc_blob_size_in_expected_range` — assert blob size is in 3000–8000 bytes (catches accidental swap to a ~60 KB preference profile).

Unit tests in `encode::tests` (extending existing tests):

- `encode_jpeg_embeds_srgb_icc` — encode 4×4 grey JPEG → reparse via `img_parts::jpeg::Jpeg` → assert `icc_profile()` returns `Some(blob)` matching `SRGB_V4_ICC`.
- `encode_png_embeds_srgb_icc` — same shape for PNG via `img_parts::png::Png`.
- `encode_tiff_embeds_srgb_icc` — encode 4×4 grey TIFF → parse with `tiff::decoder::Decoder` → read tag 0x8773 → assert bytes match `SRGB_V4_ICC`.
- `encode_overrides_input_icc_with_srgb` — call `encode_to_file_with_options` with an `ImageMetadata` carrying EXIF (no `icc_profile` field exists post-refactor); reparse output ICC; assert it equals `SRGB_V4_ICC`. This pins the bug fix.
- `encode_pixel_bytes_unchanged_from_pre_sp2_baseline` — for a deterministic 4×4 input, hash the decoded pixel buffer (post-decode, ignoring ICC metadata). Compare against a hardcoded golden hash. Confirms only metadata changed.

Architecture tests (`crates/agx/tests/architecture.rs`): unchanged. `encode/icc.rs` lives inside the `encode` module; no new cross-module edges.

E2E tests (`crates/agx-e2e/`): the golden comparison harness today compares decoded pixel data, which is metadata-agnostic. Verify this during implementation. If true, no harness change; existing goldens remain valid because pixel output is unchanged. If false (harness compares file bytes), goldens regenerate to include new ICC chunks.

External-tool spot-check (manual, documented in PR description, not in CI):

- `exiftool -ICC_Profile:all sample-output.{jpg,png,tif}` reports presence of sRGB v4 ICC.
- macOS Preview's color-profile inspector reports sRGB v4.

## Documentation updates

Code-level:

- `crates/agx/src/encode/icc.rs` — module doc comment explaining role, blob source, license.
- `crates/agx/src/encode/mod.rs` — doc comment on `encode_to_file_with_options` updated to mention unconditional ICC embed.
- `crates/agx/src/metadata.rs` — doc comments updated to reflect `ImageMetadata` shape change.

Module READMEs:

- `crates/agx/src/encode/README.md` — document the ICC contract, point readers to `icc.rs`. Extension guide: any new output format must also embed sRGB ICC.
- `crates/agx/src/metadata/README.md` — note ICC removal from `ImageMetadata`; redirect readers asking about output color labeling to `encode/`.

Architecture:

- `ARCHITECTURE.md` — add output-labeling invariant per "Architecture invariants" section above.

Book content (`docs/book/src/`):

- New explanation page: `docs/book/src/explanation/color-profiles.md`. Covers: what an ICC profile is, why AgX embeds sRGB on output, why open-source projects use Elle Stone profiles rather than vendor reference profiles. Current-state only; no SP3 / SP4 teasers.
- Update `docs/book/src/SUMMARY.md` to include the new page.
- Audit existing book pages for "AgX writes no ICC" or "viewers must guess sRGB" prose — update or remove. Likely candidates: any `explanation/color*.md` pages.

Backlog:

- `docs/backlog/color-management.md` SP2 sub-project — drop HEIF bullet, drop opt-out-flag bullet, update acceptance text to read "Output JPEG, PNG, and TIFF correctly identifies as sRGB ..." (PNG added, HEIF removed).
- `docs/backlog/heic-support.md` "HEIF encode (future)" item — append cross-reference noting that ICC embed is part of HEIF encode's future scope and will reuse the same blob/pattern.

This list is the implementation-phase checklist. Adversarial review at the end of the implementation verifies every item.

## Migration and Definition of Done

Pre-1.0 API break: `ImageMetadata.icc_profile` field removed. Callers within the workspace verified clean (grep, then test compile). No external consumers exist. PR description calls the break out explicitly.

PR title: `feat: output sRGB ICC embed (color management SP2)`.

Per `CLAUDE.md` DoD:

1. `./scripts/verify.sh` passes.
2. `./scripts/e2e-quick.sh` passes.
3. `./scripts/e2e.sh` passes in CI.
4. `ARCHITECTURE.md` updated.
5. `encode/README.md` and `metadata/README.md` updated.
6. This design doc lives at `docs/plans/2026-05-29-output-icc-embed-design.md`.
7. New book page committed at `docs/book/src/explanation/color-profiles.md`, indexed in `SUMMARY.md`.
8. Backlog updates committed.
9. Manual `exiftool` spot-check on one JPEG, one PNG, one TIFF output — documented in PR body.
10. PR body notes the pre-1.0 API break (removal of `ImageMetadata.icc_profile`).

## Considerations

- **Blob byte-stability across rebuilds.** `include_bytes!` is byte-exact; the blob never changes unless the file on disk changes. Output ICC bytes are deterministic. Reproducible-build pipelines benefit.
- **File-size impact.** ~3 KB per output file. On a 50 MB JPEG, negligible (0.006 %). On a 200 KB thumbnail, ~1.5 %. Documented in the PR description.
- **No runtime overhead.** ICC embed is a constant-time memcpy of a 3 KB blob. Imperceptible against any encode time.
- **HEIF output and SP4 share infrastructure.** When HEIF encode and SP4 (output gamut choice) land, both can reuse the `encode/profiles/` directory pattern. SP4 specifically adds `p3_v4.icc` and `adobe_rgb_v4.icc` alongside `srgb_v4.icc` and selects between them based on `EncodeOptions.output_gamut`.
- **No conflict with the SP1 working-space contract.** The engine continues to operate in linear Rec.2020. Encode continues to convert linear Rec.2020 → sRGB at the final fused matrix → curve → quantize pass. This design only labels the output; it does not change pixel math.
