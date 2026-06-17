# Ecosystem Interop

Make AgX's preset language portable across the photo-editing ecosystem. This is strategically central: the portable preset language is AgX's product, and its reach depends on whether an AgX-authored look can travel to the engines users already run.

## The hard problem

Porting a preset is not copying parameter values. Lightroom, Capture One, darktable, and RawTherapee each interpret a named adjustment ("contrast", "highlights", a tone curve) through different algorithms and color science. Writing AgX's numbers straight into an XMP or `.costyle` renders wildly differently — the same nominal contrast lands in a different place in each engine. Faithful portability means *calibrating* the mapping so the rendered look approximates, not transcribing fields. This is closer to a research/measurement track than a parsing task, and may not be fully solvable for every parameter. It is the same calibration problem as the cross-engine consistency framing under [Processing Parity](processing-parity.md).

## Sub-tasks

### Export — the preset language reaches other engines (strategic direction)

- [ ] **Export to Lightroom XMP** — generate ACR/Lightroom presets from AgX presets, calibrated so the rendered look approximates AgX's. Likely highest leverage (largest user base).
- [ ] **Export to Capture One `.costyle`** — the same calibration, for Capture One styles.
- [ ] **Export to darktable / RawTherapee** — darktable XMP and RawTherapee `.pp3`.
- [ ] **Sidecar files** — store per-image AgX edits alongside the source (e.g. `photo.cr2.agx`) in the native TOML format, enabling non-destructive workflows without a database. Pure AgX, no cross-engine mapping — the lowest-risk item here.

### Import — other engines' presets into AgX (demand-gated)

- [ ] **Lightroom XMP import** — parse ACR XMP presets into AgX. The same calibration problem in reverse.
- [ ] **Capture One / darktable / RawTherapee import** — `.costyle`, darktable XMP, and `.pp3` profiles.

## Considerations

- Mapping is inherently approximate and engine-specific; "faithful" means the rendered look approximates, never byte-identical parameters.
- Export leads import under the language-as-product framing: getting AgX looks *into* powerful proprietary engines serves the vision more directly than ingesting other tools' presets.
- There is no programmatic access to Adobe/Capture One rendering — the delivery mechanism is a preset file the user applies in their own tool, not a hosted call into their engine.

## Related

- [Processing Parity](processing-parity.md) — cross-engine consistency is the same calibration problem; a faithful export depends on understanding how each engine renders a given parameter.
- [Preset Tooling](preset-tooling.md) — validation and schema stability keep the portable format trustworthy.
- [Platform and Distribution](platform-and-distribution.md) — a marketplace distributes the portable language; portability widens where those presets can be used.
