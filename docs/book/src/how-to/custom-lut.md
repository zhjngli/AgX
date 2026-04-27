# Author a custom `.cube` LUT

AgX supports 3D LUTs in the [Adobe `.cube` format](../reference/concepts/lut-format.md), the de facto exchange format used by Photoshop, DaVinci Resolve, and most colour-grading tools. You can write a `.cube` file by hand, generate one programmatically, or use AgX's bundled `agx-lut-gen` dev tool.

## Prerequisites

- AgX installed (see [Install](../install.md)).
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

AgX includes a dev-only `agx-lut-gen` crate that emits canonical `.cube` LUTs corresponding to specific looks. From an AgX source checkout:

```bash
cargo run -p agx-lut-gen -- --help
```

Outputs the list of bakeable looks. Each one writes a `.cube` file you can load through `--lut` or reference from a preset's `[lut]` section.

## Reference a LUT from a preset

In a `.toml` preset, the `[lut]` section embeds the LUT path:

```toml
[lut]
path = "lift-shadows.cube"
amount = 1.0
```

AgX resolves `path` relative to the preset file. The `amount` field blends the LUT result with the pre-LUT image (1.0 = full LUT, 0.0 = no LUT).

## See also

- [Compose layered looks](compose-looks.md) — combine LUTs with other adjustments.
- [Write your own preset](write-preset.md)
- [LUT format reference](../reference/concepts/lut-format.md) — the full Adobe `.cube` spec.
