# decode

## Purpose
Decode image files into linear sRGB `Rgb32FImage` buffers for the engine.

## Public API
- `decode(path)` -- unified entry point; auto-detects format from extension
- `decode_standard(path)` -- decode JPEG, PNG, TIFF, BMP, WebP via the `image` crate, converting from sRGB gamma to linear
- `is_raw_extension(path)` -- check if a file extension is a known raw format
- `raw::decode_raw(path)` -- decode raw files via LibRaw FFI (behind `raw` feature)
- `raw::extract_raw_metadata(path)` -- extract synthetic EXIF from LibRaw parsed fields (behind `raw` feature)

## Extension Guide
- **Standard formats:** Supported automatically via the `image` crate. If `image` adds a new format, it works with no changes.
- **Raw formats:** Add the extension to `RAW_EXTENSIONS`. LibRaw already supports 1000+ camera models, so new raw formats typically just need the extension added.

## Does NOT
- Process or adjust images after decoding.
- Encode or write output files.
- Define or use metadata types -- returns raw EXIF bytes (`Vec<u8>`) and leaves wrapping to the metadata module.

## Key Decisions
- **Output is always linear sRGB f32.** Standard images are assumed sRGB gamma and converted to linear on decode. Raw images are demosaicked by LibRaw and converted to linear sRGB via its color pipeline.
- **Raw support is feature-gated.** The `raw` feature flag controls LibRaw FFI compilation. Without it, raw extensions produce an error message rather than a compile failure.
- **Extension-based routing.** `decode()` checks the file extension to choose the decode path. This is simple and aligns with how camera files are named.
