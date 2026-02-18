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

## Image Quality & Metadata

- **EXIF/metadata preservation**: Preserve all metadata (camera, lens, shutter speed, aperture, ISO, GPS, date/time, copyright, etc.) from input to output. Currently all metadata is stripped during processing. Likely needs the `kamadak-exif` or `rexiv2` crate to read metadata from the source and re-embed it in the output.
- **JPEG quality control**: Allow specifying JPEG output quality (e.g., `--quality 95`). The `image` crate defaults to quality 80. Original JPEG quality cannot be read back from the file, so it must be user-specified. Consider a sensible default (e.g., 92-95) for photo editing use cases.
- **Output format selection**: Allow choosing output format (JPEG, PNG, TIFF, WebP) independently of input format. Currently output format is inferred from the output file extension.

## API Service

- **REST API**: Expose oxiraw as an HTTP API service. Accept an image + a preset file or inline adjustment parameters, return the processed image. Could serve as the backend for a web UI, mobile app, or the preset marketplace. Key considerations: multipart upload for image + preset, JSON body for inline adjustments, streaming response for large images, authentication, rate limiting, job queuing for heavy processing.

## Ecosystem & Interop

- **Lightroom XMP import**: Parse Adobe Camera Raw XMP presets and convert to oxiraw format
- **Capture One .costyle import**: Parse Capture One styles
- **darktable XMP import**: Parse darktable sidecar files
- **RawTherapee .pp3 import**: Parse RawTherapee processing profiles
- **Export to other formats**: Generate XMP/costyle/pp3 from oxiraw presets (lossy — not all params map 1:1)
- **Preset marketplace / registry**: A platform for sharing and discovering community presets
- **Sidecar files**: Store per-image edits alongside the source file (like Lightroom's .xmp sidecars)

## Pipeline Architecture

- **Pluggable pipeline stages**: Refactor the render pipeline into a sequence of discrete stages, each declaring its expected input/output color space. The engine auto-inserts color space conversions between stages. LUTs, adjustments, and future transforms (curves, HSL, etc.) each become a stage. This enables user-configurable pipeline ordering and makes it easy to add new transform types without modifying the core engine.
- **Stage-level caching**: Cache intermediate results at stage boundaries. When a parameter changes, only recompute from the affected stage forward.
- **Color-space-aware stages**: Each stage declares whether it operates in linear, sRGB gamma, log, or another space. The pipeline inserts conversions automatically, which also enables proper support for LUTs designed for different input spaces (sRGB, log, linear).

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

## Processing Parity

Rendering differences between oxiraw and other photo editors (Capture One, Lightroom, darktable) are expected for any input format — not just raw files. Multiple factors contribute:

- **Demosaicing algorithm**: LibRaw defaults (AHD/PPG) differ from Capture One's proprietary algorithms, affecting detail and color at the pixel level
- **Tone curves**: Each processor applies its own base tone curve to raw data before user adjustments. LibRaw's default rendering is fairly flat compared to commercial processors
- **White balance calculation**: "Auto" white balance varies between implementations; camera-stored WB may be interpreted differently
- **Exposure mapping**: How "+1 stop" translates to pixel values may differ (linear multiply vs curve-aware lift)
- **Color matrices**: Each processor may use different per-camera color calibration data
- **Gamma/highlight handling**: Highlight recovery, highlight reconstruction, and rolloff behavior vary significantly

This is the nature of raw processing — there is no single "correct" rendering, only different interpretations. Normalizing output to match a specific processor is possible but complex (would require reverse-engineering their tone curves and color science). For now, oxiraw produces its own look. Future work could include:

- Configurable base tone curves (flat, medium contrast, match-Lightroom, etc.)
- Per-camera color profiles (DCP/ICC) for more accurate starting points
- User-adjustable demosaicing algorithm selection
- A/B comparison tooling to visualize differences against reference renders

## Advanced / Research

- **Camera color profiles**: Per-camera color matrix tuning for accurate color rendering
- **Film emulation database**: Community-contributed film stock emulations (Portra, Ektar, Tri-X, etc.)
- **AI-assisted editing**: Suggest preset adjustments based on image content
- **HDR merge**: Combine multiple exposures
- **Panorama stitching**: Combine overlapping images
- **Focus stacking**: Combine images with different focus planes
- **Tethered shooting**: Direct camera control and live preview
