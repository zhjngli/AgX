# decode

## Purpose

Decode image files into linear Rec.2020 `Rgb32FImage` buffers for the engine.

## Public API

- `decode(path)` -- unified entry point; auto-detects format from extension
- `decode_standard(path)` -- decode JPEG, PNG, TIFF, BMP, WebP via the `image` crate, converting from sRGB gamma to linear, then matrix-converted to linear Rec.2020
- `is_raw_extension(path)` -- check if a file extension is a known raw format
- `raw::decode_raw(path)` -- decode raw files via LibRaw FFI (behind `raw` feature)
- `raw::extract_raw_metadata(path)` -- extract synthetic EXIF from LibRaw parsed fields (behind `raw` feature)
- `is_heic_extension(path)` -- check if a file extension is a HEIF container (.heic, .heif)
- `heic::decode_heic(path)` -- decode HEIC/HEIF files via libheif FFI (behind `heic` feature)
- `heic::extract_heic_metadata(path)` -- extract raw EXIF bytes from HEIF metadata blocks (behind `heic` feature)

## Extension Guide

- **Standard formats:** Supported automatically via the `image` crate. If `image` adds a new format, it works with no changes.
- **Raw formats:** Add the extension to `RAW_EXTENSIONS`. LibRaw already supports 1000+ camera models, so new raw formats typically just need the extension added.
- **HEIF container formats:** AgX recognizes `.heic` and `.heif` extensions today. To add AV1-based `.avif` decoding, append `"avif"` to `HEIC_EXTENSIONS`; libheif will handle the codec if the system build of libheif was compiled with AV1 support. New variants typically just need an extension entry.

## Does NOT

- Process or adjust images after decoding.
- Encode or write output files.
- Define or use metadata types -- returns raw EXIF bytes (`Vec<u8>`) and leaves wrapping to the metadata module.

## Key Decisions

- **Output is always linear Rec.2020 f32.** Standard sRGB inputs and BT.709 are matrix-converted; Display P3 HEIC inputs are matrix-converted directly to Rec.2020 (preserving wide gamut); BT.2020 SDR primaries map identity. LibRaw stays in sRGB output mode then matrix-converts (native Rec.2020 path deferred as perf follow-up).
- **Raw support is feature-gated.** The `raw` feature flag controls LibRaw FFI compilation. Without it, raw extensions produce an error message rather than a compile failure.
- **HEIC support is feature-gated.** The `heic` feature flag controls libheif FFI compilation. Without it, HEIC/HEIF extensions produce an error message rather than a compile failure.
- **Extension-based routing.** `decode()` checks the file extension to choose the decode path. This is simple and aligns with how camera files are named.

## Building from source

The optional FFI-backed decoders require system libraries:

- **LibRaw** (for `raw` feature): `brew install libraw` (macOS) or `sudo apt install libraw-dev` (Debian/Ubuntu).
- **libheif** (for `heic` feature): `brew install libheif` (macOS) or `sudo apt install libheif-dev libheif-plugin-libde265` (Debian/Ubuntu — the plugin package provides the HEVC decoder backend, which `libheif-dev` no longer pulls in transitively on Ubuntu 24.04).
