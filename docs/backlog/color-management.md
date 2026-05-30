# Color Management

Widen AgX beyond the current "sRGB only" invariant so wide-gamut inputs (iPhone Display P3 HEIC, Adobe RGB JPEG, scanner TIFF) survive the pipeline and outputs identify themselves correctly to downstream software.

## Status

AgX edits in linear Rec.2020 for physical operations and gamma-encoded Rec.2020 for perceptual operations (SP1 shipped). `ARCHITECTURE.md` core invariant #3 codifies the new working space. The remaining sub-projects below cover ICC profile handling, output gamut choice, and HDR — independently useful capabilities that build on SP1.

The work below is sequenced so each sub-project lands an independently useful capability and unblocks the next.

## Sub-projects

Each sub-project gets its own design doc (`docs/plans/`) and implementation plan. Pick them up one at a time, in order.

### 1. Wide working space (matrix-only)

Widened the engine's internal working space beyond linear sRGB to linear Rec.2020 so wide-gamut inputs are preserved through editing. Handles the small set of well-known color spaces (sRGB, Display P3, Adobe RGB, Rec.2020) via hardcoded primary matrices — no general ICC profile parsing yet.

- [x] Pick the wider working space (candidates: linear Rec.2020, linear ACEScg, linear ProPhoto). Document the trade-offs in the design doc.
- [x] Audit every adjust stage (`basic_tone`, `contrast`, `hsl`, `color_grading`, `tone_curves`, `detail`, `dehaze`, `denoise`, `grain`, `vignette`, LUT lookup) for tolerance to values outside `[0, 1]`.
- [x] Update decode paths: standard (assume sRGB), RAW (LibRaw output → working space), HEIC (use nclx tags instead of squashing to sRGB).
- [x] Update encode paths: convert working space → sRGB before encode (output ICC embed is sub-project 2; output gamut choice is sub-project 4).
- [x] Decide what `.cube` LUTs do: today they assume sRGB-domain input. Either keep LUT semantics sRGB and auto-convert around them, or migrate.
- [x] Update `ColorSpace` enum in `engine/mod.rs` if more variants are needed.
- [x] Flip `ARCHITECTURE.md` core invariant #3 to reflect the new working space.
- [x] Add e2e fixtures: a Display P3 source with out-of-sRGB-gamut color, with goldens.

**Acceptance:** met (commit range pending merge). iPhone HEIC (Display P3) round-trips through the pipeline with vivid colors preserved (relative to the pre-SP1 squash-to-sRGB baseline). Existing e2e goldens regenerated for the wider working space; default output remains sRGB. Architectural tests updated. New wide-gamut e2e fixture has goldens.

### 2. Output ICC embed (sRGB default)

Embed an sRGB ICC profile in encoded output so downstream software identifies the color space explicitly instead of guessing.

- [x] Ship a vetted sRGB ICC profile blob (or generate one from primaries/transfer — decision point). Resolved as generate, via `crates/agx-profile-gen` using lcms2 (MIT). See [`docs/contributing/asset-licensing.md`](../contributing/asset-licensing.md) for the rejected-vendor-blob rationale.
- [x] Write ICC into JPEG via the APP2 marker chunk.
- [x] Write ICC into PNG via the `iCCP` chunk.
- [x] Write ICC into TIFF via the `ICCProfile` tag (0x8773).

**Acceptance:** met. Output JPEG, PNG, and TIFF carry an sRGB v4 ICC profile (~3 KB per file). Pixel data unchanged from pre-change output — only the metadata differs. Verified via `exiftool` and via unit tests pinning byte-equality with `SRGB_V4_ICC`. HEIF output ICC deferred to the `HEIF encode (future)` item in [heic-support.md](heic-support.md) — no HEIF encoder exists today.

### 3. Input ICC read

Parse embedded ICC profiles from inputs and convert into the working space, replacing the current "assume everything is sRGB" fallback.

- [ ] Pick the ICC parsing tactic: `lcms2` (C FFI, comprehensive, heavy) vs pure-Rust crates (e.g. `qcms`) — document maturity, gamut coverage, and binary-size trade-offs in the design doc.
- [ ] Parse ICC from JPEG (APP2), TIFF (`ICCProfile` tag), PNG (`iCCP` chunk), HEIF (icc/rICC profile box).
- [ ] Convert input → working space using the parsed profile.
- [ ] Fallback when ICC is missing: assume sRGB (preserves current behavior).
- [ ] Add e2e fixture: an Adobe RGB JPEG with goldens.

**Acceptance:** Adobe RGB JPEGs decode to correct values in the working space (verified against ImageMagick or `tificc`). Scanner / Lightroom-exported TIFFs with non-standard ICCs decode without crashing. New e2e fixture has goldens.

### 4. Output gamut choice

Let the user pick the output color space at apply time (default sRGB, with Display P3 and Adobe RGB as options). Embeds the matching ICC.

- [ ] Add a CLI flag (`--output-gamut srgb|p3|adobe-rgb`) and / or a preset apply-time field.
- [ ] Convert working space → target at encode.
- [ ] Embed matching ICC profile (depends on sub-project 2).
- [ ] Extend e2e matrix to cover each output gamut option (or a representative subset).

**Acceptance:** `agx apply --output-gamut p3 ...` produces a Display P3 JPEG that Preview / Photoshop identifies correctly. Default behavior unchanged (sRGB output). Wide-gamut e2e goldens added.

## Parked

- **Out-of-gamut-tolerant HSL.** The wide working space sub-project (sub-project 1) clamps RGB → HSL conversion at entry; the HSL adjustment loses wide-gamut headroom for its stage specifically (every other stage benefits from the wider working space). The fix is a perceptually-uniform color space defined over the full positive RGB range — **OKHsl** (polar OKLab) is the modern industry answer; IPT and JzAzBz are alternatives. Cost: OKLab ↔ linear RGB conversions, OKHsl ↔ OKLab polar form, redo of the per-channel hue / saturation / luminance math in OK-space, golden regeneration for HSL-using presets. A simpler decompose-and-recombine approach (apply HSL to the clamped in-gamut portion of each pixel, add the out-of-gamut residual back unchanged) was considered during the SP1 brainstorm and rejected: it produces semantically wrong output for hue shifts, because the OOG residual still carries the original hue direction, which shouldn't survive a hue rotation.
- **Gamut compression on encode** ("smart clip" instead of hard clip). Once the working space widens (sub-project 1), values that exceed the output gamut get hard-clipped to `[0, 1]` at encode. On saturated regions (vivid sunsets, brake-light reds, peak-sky cyans) this produces flat-mesa artifacts where a smooth gradient collapses to a constant boundary value. A smoother projection — Reinhard `x / (1 + x)`, Hable filmic, ACES gamut compression, or OKLab-based chromaticity compression — trades a small global desaturation for smooth gradients into the peak. Most relevant when sub-project 1 has shipped but output is still sRGB; sub-project 4 partially mitigates by letting users pick a wider output gamut for wide-gamut sources. Defer to its own design doc when first user reports a banding artifact, or when SP1 reveals the cases are common enough to justify the work.
- **BT.2020 / HDR transfer curves.** iPhone HDR HEIC uses BT.2020 primaries with PQ or HLG transfer curves. Handling HDR correctly requires scene-vs-display-referred semantics, tone mapping, and possibly a separate HDR output format. Initial HEIC support already falls back to "treat as sRGB and warn" — that's the right punt for now. See the `BT.2020 transfer curve handling` parked item in `heic-support.md`.
- **Soft proofing.** Preview "how this will look printed on this paper" via a destination ICC + rendering intent. Late-stage feature; depends on sub-project 3 plus a print workflow.
- **Per-camera DCP profiles.** Orthogonal to working-space work — improves raw decode color accuracy, not gamut handling. Tracked under [Processing Parity](processing-parity.md) (Raw processing section).
- **Real-world Adobe RGB e2e fixture.** Sub-project 3 ships with a synthetically-tagged Adobe RGB JPEG fixture (deterministic, license-clean, carries CI). A real camera capture would additionally exercise real-world ICC quirks. iPhones don't emit Adobe RGB (Display P3 HEIC / sRGB only); Fujifilm and most mirrorless/DSLR bodies do via the Color Space → Adobe RGB menu setting. Pull one in as a supplementary fixture when convenient.

## Considerations

- **Squashing is lossy and not reversible.** Once a Display P3 photo is clipped or perceptually compressed into sRGB, the out-of-sRGB-gamut information is gone. Heuristic "gamut expansion" can guess but not recover. This is the core reason to widen the working space rather than try to undo the damage later.
- **Wider working space ≠ wider output by default.** sub-project 1 widens the internal math while keeping default output as sRGB. The two concerns are intentionally separated.
- **Display P3, Adobe RGB, Rec.2020** can all be handled via fixed primary matrices — sub-projects 1, 2, and 4 do not need a full ICC engine. ICC engine entry is deferred to sub-project 3, which is where the lcms2 vs pure-Rust decision lives.
- **Stage audit is the riskiest part of sub-project 1.** Many adjustments are written assuming `[0, 1]` inputs; pushing P3 reds into linear sRGB primaries produces negative green/blue values. Each stage needs explicit decisions about clamping, tolerance, or domain conversion.

## Related

- [HEIC Support](heic-support.md) — multiple parked items explicitly wait for sub-project 1 (P3/BT.2020 fixtures, BT.2020 transfer handling, out-of-gamut clamping audit).
- [Pluggable Pipeline](pluggable-pipeline.md) — `Stage` trait + `ColorSpace` enum already exists; the `color-space-aware stage insertion` sub-task there gates on sub-project 1.
- [Processing Parity](processing-parity.md) — per-camera DCP profiles tracked under that epic's raw processing section.
- [Ecosystem Interop](ecosystem-interop.md) — ICC profiles matter for cross-tool compatibility.
