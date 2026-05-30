# metadata

## Purpose

Extract and represent EXIF metadata from input files for preservation through the editing pipeline. Output ICC profile labeling is owned by the `encode` module, not by input metadata pass-through.

## Public API

- `ImageMetadata` -- struct with `exif: Option<Vec<u8>>` (raw bytes)
- `extract_metadata(path)` -- best-effort extraction using a cascading strategy; returns `Option<ImageMetadata>`

## Extension Guide

To add a new metadata extraction strategy:

1. Write an `extract_metadata_foo(...)` function following the existing pattern.
2. Add it as a new step in the cascade inside `extract_metadata()`, after existing strategies.
3. Each strategy returns `Option<ImageMetadata>` -- return `None` to fall through to the next.

Current cascade order:

1. `img_parts` for JPEG (lossless byte-level EXIF copy)
2. `img_parts` for PNG (EXIF)
3. `kamadak-exif` for TIFF-based raw files (behind `raw` feature)
4. LibRaw parsed fields for non-TIFF raw files (behind `raw` feature)
5. `libheif` for HEIC/HEIF containers (behind `heic` feature)
6. Return `None`

After any successful extraction, the EXIF `Orientation` tag (0x0112) is rewritten to `1` (Normal) — see Key Decisions.

## Does NOT

- Manipulate pixel data.
- Encode or inject metadata into output files (the encode module does that).
- Block the processing pipeline on failure -- extraction is always best-effort.

## Key Decisions

- **Raw bytes, not parsed structures.** EXIF is stored as opaque `Vec<u8>` for lossless round-tripping. No field-level parsing means no data loss.
- **Output ICC owned by encoder, not input pass-through.** AgX always produces sRGB output, so the encoder writes a canonical sRGB ICC blob regardless of input. Preserving the input ICC (e.g. AdobeRGB, Display P3) and stamping it onto an sRGB-encoded output would mislabel the file. See `encode::icc` for the output side.
- **Best-effort extraction.** `extract_metadata` returns `Option` and never errors. Metadata is valuable but not essential -- missing metadata should never prevent image processing.
- **Cascading strategies.** Different file types need different extraction approaches. The cascade tries format-specific parsers in order and stops at the first success.
- **Orientation tag normalized to 1.** Decoders apply EXIF orientation to pixel data during decode, leaving the engine's pixels in canonical form. The returned EXIF blob has its `Orientation` tag (0x0112) rewritten to `1` (Normal) in every IFD of the TIFF chain (IFD0 main, IFD1 thumbnail, and so on) so that neither main-image viewers nor thumbnail-preferring tools (Bridge, Lightroom) rotate the already-canonical output a second time. This is the only field-level mutation made to the otherwise opaque EXIF bytes.
