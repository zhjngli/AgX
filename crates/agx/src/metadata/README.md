# metadata

## Purpose
Extract and represent image metadata (EXIF, ICC profiles) from input files for preservation through the editing pipeline.

## Public API
- `ImageMetadata` -- struct with `exif: Option<Vec<u8>>` and `icc_profile: Option<Vec<u8>>` (raw bytes)
- `extract_metadata(path)` -- best-effort extraction using a cascading strategy; returns `Option<ImageMetadata>`

## Extension Guide
To add a new metadata extraction strategy:
1. Write an `extract_metadata_foo(...)` function following the existing pattern.
2. Add it as a new step in the cascade inside `extract_metadata()`, after existing strategies.
3. Each strategy returns `Option<ImageMetadata>` -- return `None` to fall through to the next.

Current cascade order:
1. `img_parts` for JPEG (lossless byte-level EXIF + ICC copy)
2. `img_parts` for PNG (EXIF + ICC)
3. `kamadak-exif` for TIFF-based raw files (behind `raw` feature)
4. LibRaw parsed fields for non-TIFF raw files (behind `raw` feature)
5. Return `None`

## Does NOT
- Manipulate pixel data.
- Encode or inject metadata into output files (the encode module does that).
- Block the processing pipeline on failure -- extraction is always best-effort.

## Key Decisions
- **Raw bytes, not parsed structures.** EXIF and ICC profiles are stored as opaque `Vec<u8>` for lossless round-tripping. No field-level parsing means no data loss.
- **Best-effort extraction.** `extract_metadata` returns `Option` and never errors. Metadata is valuable but not essential -- missing metadata should never prevent image processing.
- **Cascading strategies.** Different file types need different extraction approaches. The cascade tries format-specific parsers in order and stops at the first success.
