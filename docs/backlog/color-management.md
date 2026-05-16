# Color Management

Widen AgX beyond the current "sRGB only" invariant so wide-gamut inputs (iPhone Display P3 HEIC, Adobe RGB JPEG, scanner TIFF) survive the pipeline and outputs identify themselves correctly to downstream software.

## Status

AgX today edits everything in linear sRGB. `ARCHITECTURE.md` core invariant #3 codifies this: no working-space conversion, no ICC profile handling. Several backlog items are already waiting on this assumption being lifted:

- The HEIC decoder reads Display P3 / BT.2020 nclx tags but currently matrix-converts to linear sRGB at decode time, discarding the wider gamut (see `heic-support.md` known gaps).
- The pluggable pipeline already exposes a `ColorSpace` enum (`LinearSrgb`, `SrgbGamma`) and auto-inserts conversion stages — the scaffolding is ready for more variants.
- Several adjust stages assume input values lie in `[0, 1]`; once the working space widens, P3 colors in linear sRGB can be negative or > 1 and stages must handle that gracefully.

The work below is sequenced so each sub-project lands an independently useful capability and unblocks the next.

## Sub-projects

Each sub-project gets its own design doc (`docs/plans/`) and implementation plan. Pick them up one at a time, in order.

### 1. Wide working space (matrix-only)

Widen the engine's internal working space beyond linear sRGB so wide-gamut inputs are preserved through editing. Handle the small set of well-known color spaces (sRGB, Display P3, Adobe RGB, Rec.2020) via hardcoded primary matrices — no general ICC profile parsing yet.

- [ ] Pick the wider working space (candidates: linear Rec.2020, linear ACEScg, linear ProPhoto). Document the trade-offs in the design doc.
- [ ] Audit every adjust stage (`basic_tone`, `contrast`, `hsl`, `color_grading`, `tone_curves`, `detail`, `dehaze`, `denoise`, `grain`, `vignette`, LUT lookup) for tolerance to values outside `[0, 1]`.
- [ ] Update decode paths: standard (assume sRGB), RAW (LibRaw output → working space), HEIC (use nclx tags instead of squashing to sRGB).
- [ ] Update encode paths: convert working space → sRGB before encode (output ICC embed is sub-project 2; output gamut choice is sub-project 4).
- [ ] Decide what `.cube` LUTs do: today they assume sRGB-domain input. Either keep LUT semantics sRGB and auto-convert around them, or migrate.
- [ ] Update `ColorSpace` enum in `engine/mod.rs` if more variants are needed.
- [ ] Flip `ARCHITECTURE.md` core invariant #3 to reflect the new working space.
- [ ] Add e2e fixtures: a Display P3 source with out-of-sRGB-gamut color, with goldens.

**Acceptance:** iPhone HEIC (Display P3) round-trips through the pipeline with vivid colors preserved (relative to today's squash-to-sRGB baseline). All existing e2e goldens still pass (default output remains sRGB). Architectural tests updated. New wide-gamut e2e fixture has goldens.

### 2. Output ICC embed (sRGB default)

Embed an sRGB ICC profile in encoded output so downstream software identifies the color space explicitly instead of guessing.

- [ ] Ship a vetted sRGB ICC profile blob (or generate one from primaries/transfer — decision point).
- [ ] Write ICC into JPEG via the APP2 marker chunk.
- [ ] Write ICC into TIFF via the `ICCProfile` tag (0x8773).
- [ ] Write color space info into HEIF (colr box / nclx tag for sRGB).
- [ ] Library / CLI: ICC embed on by default, with an opt-out flag for size-sensitive workflows.

**Acceptance:** Output JPEG/TIFF/HEIF correctly identifies as sRGB in macOS Preview, Photoshop, and `exiftool`. Pixel data bit-identical to pre-change output — only the metadata differs. File-size impact documented.

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

## Deferred / out of scope for this epic

- **BT.2020 / HDR transfer curves.** iPhone HDR HEIC uses BT.2020 primaries with PQ or HLG transfer curves. Handling HDR correctly requires scene-vs-display-referred semantics, tone mapping, and possibly a separate HDR output format. Initial HEIC support already falls back to "treat as sRGB and warn" — that's the right punt for now. See the `BT.2020 transfer curve handling` known gap in `heic-support.md`.
- **Soft proofing.** Preview "how this will look printed on this paper" via a destination ICC + rendering intent. Late-stage feature; depends on sub-project 3 plus a print workflow.
- **Per-camera DCP profiles.** Orthogonal to working-space work — improves raw decode color accuracy, not gamut handling. Tracked under [Processing Parity](processing-parity.md) (Raw processing section).

## Considerations

- **Squashing is lossy and not reversible.** Once a Display P3 photo is clipped or perceptually compressed into sRGB, the out-of-sRGB-gamut information is gone. Heuristic "gamut expansion" can guess but not recover. This is the core reason to widen the working space rather than try to undo the damage later.
- **Wider working space ≠ wider output by default.** sub-project 1 widens the internal math while keeping default output as sRGB. The two concerns are intentionally separated.
- **Display P3, Adobe RGB, Rec.2020** can all be handled via fixed primary matrices — sub-projects 1, 2, and 4 do not need a full ICC engine. ICC engine entry is deferred to sub-project 3, which is where the lcms2 vs pure-Rust decision lives.
- **Stage audit is the riskiest part of sub-project 1.** Many adjustments are written assuming `[0, 1]` inputs; pushing P3 reds into linear sRGB primaries produces negative green/blue values. Each stage needs explicit decisions about clamping, tolerance, or domain conversion.

## Related

- [HEIC Support](heic-support.md) — multiple known gaps explicitly wait for sub-project 1 (P3/BT.2020 fixtures, BT.2020 transfer handling, out-of-gamut clamping audit).
- [Pluggable Pipeline](pluggable-pipeline.md) — `Stage` trait + `ColorSpace` enum already exists; the `color-space-aware stage insertion` sub-task there gates on sub-project 1.
- [Processing Parity](processing-parity.md) — per-camera DCP profiles tracked under that epic's raw processing section.
- [Ecosystem Interop](ecosystem-interop.md) — ICC profiles matter for cross-tool compatibility.
