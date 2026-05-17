# encode

## Purpose

Encode rendered linear Rec.2020 images to 8-bit sRGB output files (JPEG, PNG, TIFF) with optional metadata injection.

## Public API

- `OutputFormat` -- enum: `Jpeg`, `Png`, `Tiff`
- `OutputFormat::extension()` / `OutputFormat::from_extension(ext)` -- format-extension mapping
- `EncodeOptions` -- `jpeg_quality` (default 92), `format` (optional override)
- `encode_to_file(linear, path)` -- simple encode with defaults (format inferred from extension)
- `encode_to_file_with_options(linear, path, options, metadata)` -- full-control encode with quality, format, and metadata injection; returns the final output path
- `resolve_output(path, format)` -- determine final path and format from extension and optional override
- `encode_linear_rec2020_to_srgb_rgb8(linear_rec2020)` -- convert linear Rec.2020 f32 buffer to 8-bit sRGB `RgbImage` in a single fused pass: matrix → curve → quantize

## Extension Guide

To add a new output format:

1. Add a variant to `OutputFormat` and update `extension()` / `from_extension()`.
2. Add an encoding branch in `encode_to_file_with_options` using the appropriate encoder.
3. Add metadata injection support in `inject_metadata` if the format supports EXIF/ICC.
4. Update `parse_output_format` in the CLI.

## Does NOT

- Decode or read input images.
- Apply adjustments or know about presets.
- Define metadata types (receives `ImageMetadata` from the metadata module).

## Key Decisions

- **Rec.2020-to-sRGB conversion on encode.** The engine outputs linear Rec.2020; this module fuses the gamut conversion (matrix multiply), the sRGB transfer curve, and the f32 → u8 quantization into a single per-pixel pass.
- **Path resolution rules.** If the requested format and extension match, use as-is. If they conflict, append the correct extension. Unknown extensions default to JPEG.
- **Metadata injection is best-effort.** Uses `img_parts` for JPEG/PNG and `little_exif` for TIFF. Injection failures on TIFF are silent (best-effort).
