# Example

Sample images, presets, and LUTs for trying out AgX. The CLI tutorial under [`docs/book/src/tutorials/`](../docs/book/src/tutorials/getting-started.md) uses files from this directory.

## Images

| Image | Description |
|-------|-------------|
| `marina_sunset.heic` | Marina sunset with birds in V-formation — vivid orange-to-blue gradient, Display P3 source |
| `grand_canyon_overlook.raf` | Grand Canyon overlook with atmospheric haze and snow patches — wide dynamic range, dehaze demo |
| `cinque_terre_manarola.raf` | Iconic colorful cliffside houses in daylight — vivid texture, complementary color |
| `concert_hall.heic` | Concert hall steps lit by warm wood + teal spotlight — soft complementary color |
| `mountain_valley.heic` | Mountain valley stream silhouette at twilight — deep shadow, near-monochrome |
| `sky_moon_wires.heic` | Crossing power lines against a P3 blue sky with the moon — minimal pattern |
| `geisel_library_bw.jpg` | Geisel Library brutalist architecture, B&W — mono source |
| `cinque_terre_window.jpg` | Window-framed view of the Ligurian coast at sunset — soft golden hour |
| `film_beach.jpg` | Beach scene scanned from Gold 200 film — gentle film aesthetic |
| `ranunculus_field.heic` | Pink and orange ranunculus field under blue sky — saturated floral |
| `stairwell_silhouette.heic` | Silhouette on a dark escalator with pinpoint doorway light — extreme DR |
| `foggy_sintra.heic` | Moorish castle ramparts emerging from fog at Sintra — atmospheric haze |
| `grand_canyon_rainbow.raf` | Grand Canyon with sky-spanning rainbow — soft pastel sky, alt scenic |

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
agx apply \
  -i example/images/cinque_terre_window.jpg \
  -p example/presets/golden-hour.toml \
  -o /tmp/out.png
```

See the [Getting Started tutorial](../docs/book/src/tutorials/getting-started.md) for a guided walkthrough.
