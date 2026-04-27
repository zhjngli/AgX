# Example

Sample images, presets, and LUTs for trying out AgX. The CLI tutorial under [`docs/book/src/tutorials/`](../docs/book/src/tutorials/getting-started.md) uses files from this directory.

## Images

| Image | Description |
|-------|-------------|
| `sunset_river.png` | Warm-light landscape over a river — dynamic range across sky and water |
| `temple_blossoms.png` | Soft-light scene with cherry blossoms framing a temple roofline |
| `night_architecture.png` | High-contrast night architecture with strong artificial light |

## Presets

| Preset | Style |
|--------|-------|
| `cool-blue.toml` | Cool temperature shift with gentle contrast |
| `faded-film.toml` | Low contrast, lifted blacks, warm tint — vintage film feel |
| `golden-hour.toml` | Warm, lifted shadows, pulled highlights — late-afternoon look |
| `high-contrast.toml` | Punchy contrast with extended tonal range |
| `moody-dark.toml` | Dark, contrasty, cool tones — cinematic mood |

## LUTs

| LUT | Description |
|-----|-------------|
| `identity.cube` | 17×17×17 identity LUT (output = input) — useful for testing |

## Sample outputs

Pre-rendered sample outputs live under `outputs/`. They were generated from the e2e fixture look set (`crates/agx-e2e/fixtures/looks/`) rather than from `example/presets/`, so they are not paired one-for-one with anything in this directory — they exist to showcase what AgX produces.

## Quick start

Apply a preset to one image:

```bash
agx-cli apply \
  -i example/images/sunset_river.png \
  -p example/presets/golden-hour.toml \
  -o /tmp/out.png
```

See the [Getting Started tutorial](../docs/book/src/tutorials/getting-started.md) for a guided walkthrough.
