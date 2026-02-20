# Image Quality & Metadata Design

**Date**: 2026-02-17
**Status**: Approved

## Overview

Add metadata preservation, JPEG quality control, and explicit output format selection to oxiraw. Metadata is copied losslessly as raw bytes from input to output. JPEG quality defaults to 92 (visually lossless for photo work). Output format can be inferred from extension or explicitly specified.

## Background: Image Metadata Types

Image files can contain several types of embedded metadata:

- **EXIF** (Exchangeable Image File Format) — camera/shot metadata written at capture time: camera model, lens, shutter speed, aperture, ISO, date/time, GPS coordinates, image orientation. The most common metadata type.
- **ICC profile** — describes the color space of the image (e.g., sRGB, Adobe RGB, Display P3). Tells software how to interpret pixel colors. Without it, software assumes sRGB.
- **XMP** (Extensible Metadata Platform) — Adobe's XML-based metadata format. Can store everything EXIF can plus ratings, keywords, edit history, copyright. Lightroom and Capture One write their edits here.
- **IPTC** (International Press Telecommunications Council) — news/editorial metadata: caption, headline, creator, copyright, location. Used by stock photo agencies and news organizations.

Currently, oxiraw strips all metadata during processing. This design adds preservation.

## Design

### Metadata Preservation

Copy all metadata (EXIF, ICC, XMP, IPTC) as raw bytes from source to output. No tag-level parsing — transplant the entire byte blob. This preserves camera info, GPS, copyright, color profiles, and everything else without risk of dropping tags.

The pipeline becomes:

1. Decode input image (existing)
2. Extract metadata bytes from input file
3. Process image through engine (existing)
4. Encode output image (existing)
5. Inject metadata bytes into output file

Metadata copy is **best-effort** — if metadata can't be read from the source (corrupt EXIF, unsupported container), produce the output without metadata. Image processing should never fail because of metadata issues.

#### Extraction by Input Format

**Standard formats (JPEG, PNG):**
- `img-parts` reads EXIF and ICC as raw byte blobs. Lossless — all tags preserved verbatim.

**TIFF-based raw formats (CR2, NEF, DNG, ARW, PEF, ORF, etc.):**
- `kamadak-exif` reads EXIF from these files since they use TIFF containers internally.
- Returns raw EXIF bytes via `Exif::buf()` which can be injected into output.
- Near-lossless — preserves all standard EXIF tags including maker notes.

**Non-TIFF raw formats (RAF/Fuji, RW2/Panasonic, CR3/Canon, etc.):**
- `kamadak-exif` cannot read these containers.
- Fallback: read parsed metadata fields from LibRaw's C structs (`imgdata.idata` for camera make/model, `imgdata.other` for ISO/shutter/aperture/focal length/timestamp/GPS, `imgdata.lens` for lens info).
- Construct EXIF using `little_exif` from the parsed fields.
- Lossy — preserves key shooting data (camera, lens, exposure, GPS, date) but loses maker notes and vendor-specific tags.
- Requires additional LibRaw FFI bindings for metadata struct access (behind `raw` feature flag).

The extraction strategy is: try `img-parts` (JPEG/PNG) → try `kamadak-exif` (TIFF-based raw) → try LibRaw fields (all other raw) → give up gracefully.

#### Injection by Output Format

- **JPEG/PNG output**: `img-parts` injects EXIF and ICC byte blobs.
- **TIFF output**: `little_exif` writes EXIF at the tag level.

#### Crate Selection

| Crate | Purpose | When used |
|-------|---------|-----------|
| `img-parts` | Byte-level EXIF/ICC read+write for JPEG/PNG | JPEG/PNG input and output |
| `kamadak-exif` | Read EXIF bytes from TIFF-based raw files | CR2, NEF, DNG, ARW, PEF, ORF input |
| `little_exif` | Construct EXIF from parsed fields; write EXIF to TIFF output | LibRaw fallback + TIFF output |
| LibRaw (existing FFI) | Read parsed metadata fields from any raw format | RAF, RW2, CR3, and other non-TIFF raw input |

All pure Rust crate additions, no new system dependencies (LibRaw is already linked for raw decoding).

### JPEG Quality Control

- Default quality: **92** (visually lossless for photo work, similar to Lightroom's default)
- CLI flag: `--quality <1-100>` on both `edit` and `apply` subcommands
- Quality only applies to JPEG output — PNG and TIFF are lossless and ignore it
- Invalid values (0 or >100) return `OxirawError::Encode`

Library API gains an options struct:

```rust
pub struct EncodeOptions {
    pub jpeg_quality: u8,    // 1-100, default 92
    pub format: Option<OutputFormat>,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            jpeg_quality: 92,
            format: None,
        }
    }
}
```

The `image` crate's `JpegEncoder::new_with_quality()` accepts a `u8` quality parameter, so this maps directly.

### Output Format Selection

Supported output formats: JPEG, PNG, TIFF.

```rust
pub enum OutputFormat {
    Jpeg,
    Png,
    Tiff,
}
```

**Resolution logic:**

1. If `--format` is specified, use that format. If the output path's extension doesn't match, append the correct extension (e.g., `out.png` + `--format jpeg` → `out.png.jpeg`).
2. If `--format` is not specified, infer from the output path's extension.
3. If the extension is unknown/unrecognized, default to JPEG and append `.jpeg` (e.g., `out.xyz` → `out.xyz.jpeg`).

**Extension mapping:**

- JPEG: `.jpg`, `.jpeg`
- PNG: `.png`
- TIFF: `.tif`, `.tiff`

A `resolve_output` function handles this logic and returns both the resolved `OutputFormat` and the final `PathBuf`. This lives in the encode module so both the library and CLI can use it.

The CLI gets `--format <jpeg|png|tiff>` and `--quality <1-100>` on both `edit` and `apply` subcommands.

## Scope

**In scope:**
- Metadata byte-level copy (EXIF, ICC, XMP, IPTC) from input to output
- Raw file metadata: lossless for TIFF-based raw (via kamadak-exif), constructed fallback for non-TIFF raw (via LibRaw fields + little_exif)
- Additional LibRaw FFI bindings for metadata structs (idata, other, lens)
- `EncodeOptions` struct with JPEG quality and output format
- `OutputFormat` enum and format resolution logic
- `--quality` and `--format` CLI flags
- New dependencies: `img-parts`, `kamadak-exif`, `little_exif`

**Out of scope (future):**
- Metadata stripping (--strip-gps, --strip-metadata)
- Modifying individual EXIF tags (e.g., setting software tag to "oxiraw")
- EXIF orientation auto-rotation
- WebP output format
- Reading EXIF for processing decisions (orientation, color space)

## Testing

- Roundtrip test: encode JPEG with EXIF, decode+process+encode, verify EXIF preserved in output
- Quality test: encode same image at quality 50 vs 95, verify file sizes differ significantly
- Format resolution tests: cover all 4 rules (known ext, format override, mismatch append, unknown ext)
- Best-effort test: image with no EXIF still encodes successfully

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| Byte-level copy, not tag-level | Preserves all metadata without risk of dropping unknown tags. Simplest approach. |
| `img-parts` for JPEG/PNG | Pure Rust, high adoption (~7.8M downloads), lossless byte-level EXIF/ICC copy. |
| `kamadak-exif` for TIFF-based raw | Most robust EXIF reader in Rust (~393k/month). Reads TIFF containers (CR2, NEF, DNG, ARW) which `img-parts` can't. |
| LibRaw fallback for non-TIFF raw | Already linked for decoding. Exposes parsed EXIF fields for all 1000+ camera formats. Covers RAF, RW2, CR3 where kamadak-exif fails. |
| `little_exif` for TIFF output | Only pure-Rust crate that can write EXIF to TIFF files. |
| Best-effort metadata | Metadata issues should never block image processing. Warn and continue. |
| Default JPEG quality 92 | Visually lossless for photo work. Matches Lightroom's default (~93). Current default of 80 loses too much quality. |
| Append extension on format mismatch | Output file extension always reflects the real format. Prevents silent mismatches. |
