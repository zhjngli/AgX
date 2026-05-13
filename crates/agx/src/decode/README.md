# decode

## Purpose

Decode image files into linear sRGB `Rgb32FImage` buffers for the engine.

## Public API

- `decode(path)` -- unified entry point; auto-detects format from extension
- `decode_standard(path)` -- decode JPEG, PNG, TIFF, BMP, WebP via the `image` crate, converting from sRGB gamma to linear
- `is_raw_extension(path)` -- check if a file extension is a known raw format
- `raw::decode_raw(path)` -- decode raw files via LibRaw FFI (behind `raw` feature)
- `raw::extract_raw_metadata(path)` -- extract synthetic EXIF from LibRaw parsed fields (behind `raw` feature)
- `is_heic_extension(path)` -- check if a file extension is a HEIF container (.heic, .heif)
- `heic::decode_heic(path)` -- decode HEIC/HEIF files via libheif FFI (behind `heic` feature)
- `heic::extract_heic_metadata(path)` -- extract raw EXIF bytes from HEIF metadata blocks (behind `heic` feature)

## Extension Guide

- **Standard formats:** Supported automatically via the `image` crate. If `image` adds a new format, it works with no changes.
- **Raw formats:** Add the extension to `RAW_EXTENSIONS`. LibRaw already supports 1000+ camera models, so new raw formats typically just need the extension added.
- **HEIF container formats:** Add the extension to `HEIC_EXTENSIONS`. libheif handles every codec inside (HEVC for `.heic`, AV1 for `.avif` if libheif is built with AV1 support), so new HEIF variants typically just need an extension entry.

## Does NOT

- Process or adjust images after decoding.
- Encode or write output files.
- Define or use metadata types -- returns raw EXIF bytes (`Vec<u8>`) and leaves wrapping to the metadata module.

## Key Decisions

- **Output is always linear sRGB f32.** Standard images are assumed sRGB gamma and converted to linear on decode. Raw images are demosaicked by LibRaw and converted to linear sRGB via its color pipeline.
- **Raw support is feature-gated.** The `raw` feature flag controls LibRaw FFI compilation. Without it, raw extensions produce an error message rather than a compile failure.
- **HEIC support is feature-gated.** The `heic` feature flag controls libheif FFI compilation. Without it, HEIC/HEIF extensions produce an error message rather than a compile failure.
- **Extension-based routing.** `decode()` checks the file extension to choose the decode path. This is simple and aligns with how camera files are named.

## Building from source

The optional FFI-backed decoders require system libraries:

- **LibRaw** (for `raw` feature): `brew install libraw` (macOS) or `sudo apt install libraw-dev` (Debian/Ubuntu).
- **libheif** (for `heic` feature): `brew install libheif` (macOS) or `sudo apt install libheif-dev` (Debian/Ubuntu).
