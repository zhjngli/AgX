# Future Features & Ideas

A running list of ideas for oxiraw's development. Nothing here is committed — this is a scratchpad for exploration.

## Editing Capabilities

- **HSL adjustments**: Per-channel (R, O, Y, G, A, B, P, M) hue/saturation/luminance
- **Tone curves**: Parametric (region-based) and point curves, RGB + per-channel
- **Color grading**: 3-way color wheels (shadows/midtones/highlights), global tint
- **Sharpening**: Amount, radius, detail, masking (luminance-based)
- **Noise reduction**: Luminance NR, color NR, detail preservation
- **Grain/film simulation**: Amount, size, roughness — simulate film stock looks
- **Dehaze**: Atmospheric haze removal
- **Clarity / Texture**: Local contrast enhancement at different frequency scales
- **Lens corrections**: Distortion, chromatic aberration, vignette (via lensfun FFI)
- **Perspective correction**: Vertical/horizontal keystone, rotation
- **Crop and rotation**: Non-destructive crop with aspect ratio presets
- **Local adjustments**: Brushes, gradients, radial filters with per-region parameters

## Preset System

- **Composable/layered presets**: Apply a base preset, then overlay another (e.g., "base look" + "warm tint" + "high contrast")
- **Preset inheritance**: A preset can extend another, overriding only specific values
- **Partial presets**: Presets that only touch certain parameter groups (e.g., only color grading)
- **Preset variables / shortcuts**: Named shortcuts for common parameter combinations
- **Preset versioning**: Schema version in presets for forward/backward compatibility
- **Preset validation**: CLI command to validate a preset against the current schema

## Ecosystem & Interop

- **Lightroom XMP import**: Parse Adobe Camera Raw XMP presets and convert to oxiraw format
- **Capture One .costyle import**: Parse Capture One styles
- **darktable XMP import**: Parse darktable sidecar files
- **RawTherapee .pp3 import**: Parse RawTherapee processing profiles
- **Export to other formats**: Generate XMP/costyle/pp3 from oxiraw presets (lossy — not all params map 1:1)
- **Preset marketplace / registry**: A platform for sharing and discovering community presets
- **Sidecar files**: Store per-image edits alongside the source file (like Lightroom's .xmp sidecars)

## Performance & Platforms

- **GPU acceleration**: Use wgpu or compute shaders for real-time rendering
- **WASM compilation**: Run the engine in the browser for web-based editing
- **Incremental rendering**: Cache intermediate results, only recompute from the changed stage
- **Thumbnail/preview pipeline**: Fast low-res preview during editing, full-res on export
- **Parallel batch processing**: Process entire folders with rayon/tokio

## UI

- **Native desktop UI**: Possibly via egui, iced, or Tauri
- **Web UI**: Via WASM compilation of the core library
- **Real-time histogram**: Live histogram display during editing
- **Before/after comparison**: Side-by-side or split view
- **Undo/redo**: Parameter state history stack

## Color Space & Color Management

- **Adobe RGB support**: Wider gamut for professional print workflows
- **ProPhoto RGB working space**: Internal processing in a wide-gamut space (like Lightroom does) to avoid clipping colors during editing
- **Display P3 output**: For Apple displays and modern wide-gamut monitors
- **ICC profile reading**: Read embedded ICC profiles from input images to determine their color space
- **ICC profile embedding**: Embed correct ICC profiles in output images
- **Color space conversion**: Convert between working spaces (sRGB → Adobe RGB → ProPhoto RGB etc.)
- **Soft proofing**: Preview how an image will look in a different color space (e.g., CMYK for print)
- **lcms2 integration**: Use the `lcms2` Rust crate for production-quality ICC profile handling
- **Per-camera color matrices**: Custom color matrices for each camera model to improve color accuracy from raw files

## Advanced / Research

- **Camera color profiles**: Per-camera color matrix tuning for accurate color rendering
- **Film emulation database**: Community-contributed film stock emulations (Portra, Ektar, Tri-X, etc.)
- **AI-assisted editing**: Suggest preset adjustments based on image content
- **HDR merge**: Combine multiple exposures
- **Panorama stitching**: Combine overlapping images
- **Focus stacking**: Combine images with different focus planes
- **Tethered shooting**: Direct camera control and live preview
