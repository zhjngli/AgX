# encode

## Purpose

Encode rendered linear Rec.2020 images to 8-bit output files (JPEG, PNG, TIFF) in the chosen output gamut (default sRGB), with optional metadata injection.

## Public API

- `OutputFormat` -- enum: `Jpeg`, `Png`, `Tiff`
- `OutputFormat::extension()` / `OutputFormat::from_extension(ext)` -- format-extension mapping
- `EncodeOptions` -- `jpeg_quality` (default 92), `format` (optional override), `output_gamut` (default sRGB)
- `encode_to_file(linear, path)` -- simple encode with defaults (format inferred from extension)
- `encode_to_file_with_options(linear, path, options, metadata)` -- full-control encode with quality, format, and metadata injection; returns the final output path
- `resolve_output(path, format)` -- determine final path and format from extension and optional override
- `encode_linear_rec2020_to_srgb_rgb8(linear_rec2020)` -- convert linear Rec.2020 f32 buffer to 8-bit sRGB `RgbImage` in a single fused pass: matrix → curve → quantize (sRGB path; internal helpers handle other gamuts)

## Output color labeling

Every output file embeds an ICC profile matching the chosen output gamut (`EncodeOptions::output_gamut`, default sRGB): `SRGB_V4_ICC`, `DISPLAY_P3_V4_ICC`, or `ADOBE_RGB_V4_ICC` in `icc.rs` (blobs under `profiles/`; see [`profiles/README.md`](profiles/README.md)). The embed is unconditional given a gamut — caller-provided metadata cannot disable or replace it — and the pixels are encoded into the same gamut via a fixed matrix + transfer curve. Default sRGB output is byte-identical to pre-SP4. See `ARCHITECTURE.md` core invariant #4.

Per-format mechanism:

- **JPEG / PNG**: written via `img_parts::set_icc_profile` in the per-format helpers `inject_jpeg_icc_and_exif` / `inject_png_icc_and_exif` (post-encode buffer rewrite).
- **TIFF**: written via the `tiff` crate's `write_tag(Tag::IccProfile, ...)` during encode (inline tag in the IFD).

## Extension Guide

To add a new output format:

1. Add a variant to `OutputFormat` and update `extension()` / `from_extension()`.
2. Add an encoding branch in `encode_to_file_with_options` using the appropriate encoder.
3. Embed the gamut's ICC blob via `icc::icc_for(gamut)` per the new format's standard ICC mechanism (e.g. JPEG APP2, PNG `iCCP`, TIFF `IccProfile` tag, HEIF `colr` box).
4. Add EXIF injection support in a per-format `inject_<fmt>_icc_and_exif` helper (or a format-specific post-write step) if the format supports it.
5. Update `parse_output_format` in the CLI.

## Does NOT

- Decode or read input images.
- Apply adjustments or know about presets.
- Define metadata types (receives `ImageMetadata` from the metadata module).
- Consult input ICC profiles. Input color space is fixed by decode (linear Rec.2020); output gamut is chosen by the caller via `EncodeOptions::output_gamut` (default sRGB).

## Key Decisions

- **Rec.2020-to-gamut conversion on encode.** The engine outputs linear Rec.2020; this module fuses the gamut conversion (matrix multiply), the transfer curve for the chosen gamut, and the f32 → u8 quantization into a single per-pixel pass.
- **Path resolution rules.** If the requested format and extension match, use as-is. If they conflict, append the correct extension. Unknown extensions default to JPEG.
- **Output ICC is encoder-owned.** Output color labeling is decided here, not forwarded from input metadata. The caller selects the output gamut via `EncodeOptions::output_gamut`; the encoder embeds the matching ICC profile unconditionally.
- **EXIF injection is best-effort.** Uses `img_parts` for JPEG/PNG and `little_exif` for TIFF (post-write). EXIF write failures on TIFF are silent (best-effort).
