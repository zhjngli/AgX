# AgX

An open-source photo editing library and CLI written in Rust, with a portable, human-readable preset format.

**AgX** takes its name from silver halide (AgX) — the light-sensitive chemical compound at the heart of analog film. Ag is silver, X is a halide. A nod to photography's chemical roots, with a wink at oxidation and Rust.

## Features

- **Tone adjustments**: exposure, contrast, highlights, shadows, whites, blacks
- **White balance**: temperature and tint shifts
- **HSL adjustments**: per-channel hue, saturation, and luminance targeting
- **3D LUT support**: apply `.cube` LUT files for color grading and film emulation
- **EXIF orientation**: automatic orientation correction for standard formats (JPEG, PNG, TIFF)
- **Preset composability**: presets can `extend` a base preset, with Option-style inheritance
- **Raw format support**: decode CR2, CR3, NEF, ARW, RAF, DNG, and 1000+ camera formats via LibRaw
- **Batch processing**: process entire directories with parallel execution via rayon
- **TOML presets**: human-readable, shareable, version-controllable editing presets
- **Metadata preservation**: EXIF and ICC profiles carried through the pipeline
- **Library + CLI**: use as a Rust library or through the command-line interface

## Sample Images

Three source photos are included in `example/images/` along with five presets in `example/presets/`.

### Before & After

| Original | Preset | Result |
|:--:|:--:|:--:|
| ![mountain](example/images/mountain-landscape.jpg) | `high-contrast.toml` | ![mountain-hc](example/outputs/mountain-landscape-high-contrast.jpg) |
| ![forest](example/images/moody-forest.jpg) | `moody-dark.toml` | ![forest-moody](example/outputs/moody-forest-moody-dark.jpg) |
| ![city](example/images/city-skyline.jpg) | `golden-hour.toml` | ![city-golden](example/outputs/city-skyline-golden-hour.jpg) |

### Presets

| Preset | Style |
|--------|-------|
| `golden-hour.toml` | Warm, lifted shadows, pulled highlights — late afternoon look |
| `moody-dark.toml` | Dark, contrasty, cool tones — cinematic mood |
| `high-contrast.toml` | Punchy contrast with extended tonal range |
| `faded-film.toml` | Low contrast, lifted blacks, warm tint — vintage film feel |
| `cool-blue.toml` | Cool temperature shift with gentle contrast |

Try them out:

```bash
cargo run -p agx-cli -- apply \
  -i example/images/moody-forest.jpg \
  -p example/presets/golden-hour.toml \
  -o edited.jpg
```

## Quick Start

```bash
# Apply a preset to an image
cargo run -p agx-cli -- apply \
  -i example/images/mountain-landscape.jpg \
  -p example/presets/golden-hour.toml \
  -o edited.jpg

# Edit with inline parameters
cargo run -p agx-cli -- edit \
  -i example/images/moody-forest.jpg \
  -o edited.jpg \
  --exposure 1.0 --contrast 25 --temperature 30

# Apply a .cube LUT
cargo run -p agx-cli -- edit \
  -i example/images/city-skyline.jpg \
  -o graded.jpg \
  --lut film-emulation.cube

# Combine adjustments with a LUT
cargo run -p agx-cli -- edit \
  -i example/images/mountain-landscape.jpg \
  -o graded.jpg \
  --exposure 0.5 --contrast 10 --lut film-emulation.cube

# Process a raw file (CR2, NEF, ARW, DNG, etc.)
cargo run -p agx-cli -- edit \
  -i photo.dng \
  -o edited.jpg \
  --exposure 0.5 --contrast 15

# Batch process a directory
cargo run -p agx-cli -- batch-edit \
  --input-dir photos/ \
  --output-dir edited/ \
  --exposure 0.5 --contrast 10

# Set JPEG output quality (default: 92)
cargo run -p agx-cli -- edit \
  -i photo.jpg \
  -o output.jpg \
  --quality 95

# Specify output format explicitly
cargo run -p agx-cli -- edit \
  -i photo.jpg \
  -o output.tiff \
  --format tiff
```

### Metadata Preservation

Metadata (EXIF, ICC profiles) is automatically preserved from input to output:
- **JPEG/PNG**: Lossless byte-level copy via `img-parts`
- **TIFF-based raw** (CR2, NEF, DNG, ARW): EXIF extracted via `kamadak-exif`
- **Non-TIFF raw** (RAF, RW2, CR3): Key shooting data reconstructed from LibRaw fields

## Preset Format

Presets are TOML files with a simple, declarative structure:

```toml
[metadata]
name = "Golden Hour"
version = "1.0"
author = "agx"

[tone]
exposure = 0.5       # stops, -5.0 to +5.0
contrast = 15.0      # -100 to +100
highlights = -30.0   # -100 to +100
shadows = 25.0       # -100 to +100
whites = 10.0        # -100 to +100
blacks = -5.0        # -100 to +100

[white_balance]
temperature = 40.0   # warm (+) / cool (-)
tint = 5.0           # magenta (+) / green (-)

[hsl.red]
hue = 10.0           # -180 to +180
saturation = 15.0    # -100 to +100
luminance = -5.0     # -100 to +100

[lut]
path = "film-emulation.cube"   # resolved relative to the preset file
```

Presets can extend a base preset with `extends`:

```toml
extends = "base_cinematic.toml"   # inherit parameters, override selectively

[tone]
contrast = 30.0   # override base contrast
```

Missing values default to neutral (no change). See `example/presets/` for more examples.

## Library Usage

```rust
use agx::{Engine, Lut3D, Preset};
use agx::decode::decode;
use agx::encode::encode_to_file;

// Decode an image (auto-detects format: JPEG, PNG, TIFF, CR2, NEF, DNG, etc.)
let image = decode("photo.jpg".as_ref()).unwrap();

// Create engine and apply a preset
let mut engine = Engine::new(image);
let preset = Preset::load_from_file("preset.toml".as_ref()).unwrap();
engine.apply_preset(&preset);

// Or set parameters directly
engine.params_mut().exposure = 1.0;
engine.params_mut().contrast = 20.0;

// Apply a .cube LUT
let lut = Lut3D::from_cube_file("film.cube".as_ref()).unwrap();
engine.set_lut(Some(lut));

// Render and save
let result = engine.render();
encode_to_file(&result, "output.jpg".as_ref()).unwrap();
```

## Project Structure

```
agx/
├── crates/
│   ├── agx/             # core library
│   │   └── src/
│   │       ├── adjust/  # adjustment algorithms (per-pixel)
│   │       ├── decode/  # image decoding (sRGB → linear) + EXIF orientation
│   │       ├── encode/  # image encoding (linear → sRGB)
│   │       ├── engine/  # rendering engine
│   │       ├── lut/     # 3D LUT parsing and interpolation
│   │       ├── preset/  # TOML preset serialization + composability
│   │       └── error.rs # error types
│   ├── agx-cli/         # CLI wrapper (edit, apply, batch-edit)
│   ├── agx-e2e/         # e2e test suite (golden file comparison)
│   └── agx-lut-gen/     # dev tool for generating .cube LUT files
├── example/             # sample images, presets, and LUTs
├── scripts/             # verify.sh, e2e.sh
└── docs/                # design docs, ideas, and contributing guides
```

## Architecture

The engine uses an **always-re-render-from-original** model: the original image is stored immutably, and every render applies all adjustments from scratch. This makes the system order-independent from the user's perspective — presets are purely declarative parameter values, not operation sequences.

All processing happens in **sRGB** color space. Exposure and white balance operate in linear sRGB; contrast, highlights, shadows, whites, blacks, HSL, and LUTs operate in sRGB gamma space. See `docs/reference/color-spaces.md` for a detailed explanation.

## Testing

```bash
# Fast checks (format, clippy, unit tests, architecture tests, doc links)
./scripts/verify.sh

# Full e2e suite (builds CLI in release, runs 54 golden comparisons)
./scripts/e2e.sh

# Regenerate golden files
GOLDEN_UPDATE=1 cargo test -p agx-e2e
```

The e2e suite tests every fixture image against 9 film-inspired look presets (6 color + 3 B&W), each combining parameter adjustments with generated 33x33x33 `.cube` LUTs. See `crates/agx-e2e/README.md` for details.

## Building with Raw Support

Raw format decoding requires [LibRaw](https://www.libraw.org/) installed on your system:

```bash
# macOS
brew install libraw

# Ubuntu/Debian
sudo apt install libraw-dev
```

The CLI enables raw support by default. To use the library without raw support (no LibRaw dependency):

```toml
# Cargo.toml — no "raw" feature, only standard formats
[dependencies]
agx = "0.1"
```

## Image Credits

Sample photos from [Unsplash](https://unsplash.com) (free to use under the [Unsplash License](https://unsplash.com/license)).

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)

at your option.
