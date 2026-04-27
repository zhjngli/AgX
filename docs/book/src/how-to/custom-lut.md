# Author a custom `.cube` LUT

AgX supports 3D LUTs in the [Adobe `.cube` format](../reference/concepts/lut-format.md), the de facto exchange format used by Photoshop, DaVinci Resolve, and most colour-grading tools. You can write a `.cube` file by hand, generate one programmatically, or use AgX's bundled `agx-lut-gen` dev tool.

## Prerequisites

- A text editor (for hand-written LUTs) or the AgX source checkout (for `agx-lut-gen`).

## Hand-write a tiny LUT

Save as `lift-shadows.cube`:

```cube
TITLE "Lift shadows by 0.05"
LUT_3D_SIZE 2
0.05 0.05 0.05
1.0  0.05 0.05
0.05 1.0  0.05
1.0  1.0  0.05
0.05 0.05 1.0
1.0  0.05 1.0
0.05 1.0  1.0
1.0  1.0  1.0
```

This is a 2×2×2 LUT — the smallest meaningful one. Each line is the RGB output for one of the 8 corner samples; AgX trilinearly interpolates between them at render time.

Apply it via `--lut`:

```bash ignore
agx-cli edit \
  -i example/images/sunset_river.png \
  -o /tmp/lifted.png \
  --lut lift-shadows.cube
```

Production LUTs are usually 17×17×17, 33×33×33, or 65×65×65 — those have hundreds or thousands of entries and are typically generated rather than hand-written.

## Generate a LUT with `agx-lut-gen`

AgX includes a dev-only `agx-lut-gen` crate that emits canonical `.cube` LUTs for AgX's bundled looks (Portra 400, Neo Noir, B&W High Contrast, and similar). From an AgX source checkout, point it at an output directory:

```bash ignore
cargo run -p agx-lut-gen -- --output-dir /tmp/agx-luts
ls /tmp/agx-luts/
```

The tool generates one `.cube` file per look into the directory you pass. Use any of them with `--lut <path>` or reference one from a preset's `[lut]` section.

If you omit `--output-dir`, the tool writes into `crates/agx-e2e/fixtures/looks/luts/` — the e2e suite's golden LUT directory. Always pass `--output-dir` unless you are intentionally regenerating the e2e fixtures.

## Reference a LUT from a preset

In a `.toml` preset, the `[lut]` section embeds the LUT path:

```toml
[lut]
path = "lift-shadows.cube"
```

AgX resolves `path` relative to the preset file. The LUT is applied at full strength when present; there is no blend-amount field in the current schema. To partially apply a LUT, bake the blend into the `.cube` file itself.

## See also

- [Compose layered looks](compose-looks.md) — combine LUTs with other adjustments.
- [Write your own preset](write-preset.md)
- [LUT format reference](../reference/concepts/lut-format.md) — the full Adobe `.cube` spec.
