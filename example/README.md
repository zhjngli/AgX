# Example

Sample images, presets, and LUTs for trying out agx.

## Images

Three photos covering different lighting conditions:

| Image | Description | Good for testing |
|-------|-------------|------------------|
| `mountain-landscape.jpg` | Mountain landscape, good tonal range | Exposure, contrast, white balance |
| `moody-forest.jpg` | Backlit forest, deep shadows and bright highlights | Shadows, blacks, highlights |
| `city-skyline.jpg` | City skyline at dusk, mixed lighting | Temperature shifts, overall tone |

Photos from [Unsplash](https://unsplash.com) (free to use under the [Unsplash License](https://unsplash.com/license)).

## Presets

| Preset | Style |
|--------|-------|
| `golden-hour.toml` | Warm, lifted shadows, pulled highlights — late afternoon look |
| `moody-dark.toml` | Dark, contrasty, cool tones — cinematic mood |
| `high-contrast.toml` | Punchy contrast with extended tonal range |
| `faded-film.toml` | Low contrast, lifted blacks, warm tint — vintage film feel |
| `cool-blue.toml` | Cool temperature shift with gentle contrast |

## Sample Outputs

Pre-generated output images in `outputs/`, each pairing a source image with a preset that suits it:

| Original | Preset | Result |
|----------|--------|--------|
| `mountain-landscape.jpg` | `high-contrast.toml` | `mountain-landscape-high-contrast.jpg` |
| `moody-forest.jpg` | `moody-dark.toml` | `moody-forest-moody-dark.jpg` |
| `city-skyline.jpg` | `golden-hour.toml` | `city-skyline-golden-hour.jpg` |

## LUTs

| LUT | Description |
|-----|-------------|
| `identity.cube` | 17x17x17 identity LUT (output = input) — useful for testing |

## Usage

Apply a preset:

```bash
cargo run -p agx-cli -- apply \
  -i example/images/moody-forest.jpg \
  -p example/presets/golden-hour.toml \
  -o /tmp/forest-golden.jpg
```

Edit with inline parameters:

```bash
cargo run -p agx-cli -- edit \
  -i example/images/mountain-landscape.jpg \
  -o /tmp/mountain-bright.jpg \
  --exposure 1.5 --shadows 40 --blacks 20
```

Apply a LUT:

```bash
cargo run -p agx-cli -- edit \
  -i example/images/city-skyline.jpg \
  -o /tmp/city-lut.jpg \
  --lut example/luts/identity.cube
```

Apply every preset to every image:

```bash
for img in example/images/*.jpg; do
  img_name=$(basename "$img" .jpg)
  for preset in example/presets/*.toml; do
    preset_name=$(basename "$preset" .toml)
    cargo run -p agx-cli -- apply \
      -i "$img" -p "$preset" \
      -o "/tmp/${img_name}-${preset_name}.jpg"
  done
done
```
