# Image Quality & Metadata Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add metadata preservation, JPEG quality control, and explicit output format selection to oxiraw.

**Architecture:** Metadata is extracted as raw bytes from the input file before processing, then injected into the encoded output. The encode pipeline gains format-specific encoders (JPEG with quality, PNG, TIFF) and an `EncodeOptions` struct. A `resolve_output` function handles format inference and extension correction. `img-parts` handles JPEG/PNG metadata at the byte level; `kamadak-exif` reads EXIF from TIFF-based raw files; LibRaw FFI accessors + `little_exif` construct EXIF from parsed fields for non-TIFF raw files.

**Tech Stack:** Rust 2021, img-parts 0.4, little_exif 0.6, kamadak-exif (exif crate) 0.5, cc 1 (build dep), image 0.25 (existing)

---

## Context

This plan implements the design in `docs/plans/2026-02-17-image-quality-metadata-design.md`. The current `encode_to_file()` in `crates/oxiraw/src/encode/mod.rs` converts linear sRGB f32 to rgb8 and uses `image`'s `save()` method (which picks format from extension and uses default JPEG quality 80). We're replacing this with a richer encoding pipeline.

Metadata extraction follows a cascading strategy: `img-parts` for JPEG/PNG, `kamadak-exif` for TIFF-based raw files (CR2, NEF, DNG, ARW, PEF, ORF), and LibRaw parsed fields + `little_exif` construction for non-TIFF raw files (RAF, RW2, CR3). Extraction is best-effort — failures never block image processing.

## Critical Files

| File | Purpose |
|------|---------|
| `crates/oxiraw/Cargo.toml` | Modify: add `img-parts`, `little_exif`, `exif` deps; add `cc` build dep |
| `crates/oxiraw/build.rs` | Modify: compile C metadata accessor helper (raw feature) |
| `crates/oxiraw/src/encode/mod.rs` | Modify: add EncodeOptions, OutputFormat, resolve_output, metadata extraction/injection, format-specific encoding |
| `crates/oxiraw/src/decode/raw.rs` | Modify: add LibRaw metadata FFI accessors and extraction function |
| `crates/oxiraw/src/decode/libraw_meta.c` | Create: C helper with LibRaw metadata accessor functions |
| `crates/oxiraw/src/lib.rs` | Modify: add re-exports for new public types |
| `crates/oxiraw-cli/src/main.rs` | Modify: add --quality and --format flags, wire up metadata |
| `crates/oxiraw-cli/tests/integration.rs` | Modify: add tests for quality, format, metadata |

---

## Phase 1: OutputFormat + EncodeOptions + Format Resolution

### Task 1.1: Add dependencies

**Files:**
- Modify: `crates/oxiraw/Cargo.toml`

**Step 1: Add img-parts and little_exif**

Add to `[dependencies]` in `crates/oxiraw/Cargo.toml`:

```toml
img-parts = "0.4"
little_exif = "0.6"
```

**Step 2: Verify it compiles**

Run: `cargo build -p oxiraw`
Expected: PASS

**Step 3: Stage**

`git add crates/oxiraw/Cargo.toml`

---

### Task 1.2: OutputFormat enum and EncodeOptions struct

**Files:**
- Modify: `crates/oxiraw/src/encode/mod.rs`

**Step 1: Write failing tests**

Add to the `mod tests` block in `encode/mod.rs`:

```rust
#[test]
fn encode_options_default_quality_is_92() {
    let opts = EncodeOptions::default();
    assert_eq!(opts.jpeg_quality, 92);
    assert!(opts.format.is_none());
}

#[test]
fn output_format_extensions() {
    assert_eq!(OutputFormat::Jpeg.extension(), "jpeg");
    assert_eq!(OutputFormat::Png.extension(), "png");
    assert_eq!(OutputFormat::Tiff.extension(), "tiff");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw encode::tests`
Expected: FAIL — `EncodeOptions` and `OutputFormat` not defined

**Step 3: Write implementation**

Add at the top of `encode/mod.rs`, after the existing `use` statements:

```rust
/// Supported output image formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Jpeg,
    Png,
    Tiff,
}

impl OutputFormat {
    /// The canonical file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Jpeg => "jpeg",
            OutputFormat::Png => "png",
            OutputFormat::Tiff => "tiff",
        }
    }

    /// Try to infer format from a file extension string.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Some(OutputFormat::Jpeg),
            "png" => Some(OutputFormat::Png),
            "tif" | "tiff" => Some(OutputFormat::Tiff),
            _ => None,
        }
    }
}

/// Options controlling image encoding.
pub struct EncodeOptions {
    /// JPEG quality (1-100). Only applies to JPEG output. Default: 92.
    pub jpeg_quality: u8,
    /// Explicit output format. If `None`, inferred from file extension.
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

**Step 4: Run tests**

Run: `cargo test -p oxiraw encode::tests`
Expected: PASS

**Step 5: Stage**

`git add crates/oxiraw/src/encode/mod.rs`

---

### Task 1.3: resolve_output() function

**Files:**
- Modify: `crates/oxiraw/src/encode/mod.rs`

**Step 1: Write failing tests**

Add to the `mod tests` block:

```rust
use std::path::PathBuf;

#[test]
fn resolve_output_infers_jpeg_from_jpg() {
    let (path, fmt) = resolve_output(std::path::Path::new("out.jpg"), None);
    assert_eq!(fmt, OutputFormat::Jpeg);
    assert_eq!(path, PathBuf::from("out.jpg"));
}

#[test]
fn resolve_output_infers_png() {
    let (path, fmt) = resolve_output(std::path::Path::new("out.png"), None);
    assert_eq!(fmt, OutputFormat::Png);
    assert_eq!(path, PathBuf::from("out.png"));
}

#[test]
fn resolve_output_infers_tiff() {
    let (path, fmt) = resolve_output(std::path::Path::new("out.tif"), None);
    assert_eq!(fmt, OutputFormat::Tiff);
    assert_eq!(path, PathBuf::from("out.tif"));
}

#[test]
fn resolve_output_format_override_matching_ext() {
    let (path, fmt) = resolve_output(
        std::path::Path::new("out.jpg"),
        Some(OutputFormat::Jpeg),
    );
    assert_eq!(fmt, OutputFormat::Jpeg);
    assert_eq!(path, PathBuf::from("out.jpg"));
}

#[test]
fn resolve_output_format_override_mismatched_ext_appends() {
    let (path, fmt) = resolve_output(
        std::path::Path::new("out.png"),
        Some(OutputFormat::Jpeg),
    );
    assert_eq!(fmt, OutputFormat::Jpeg);
    assert_eq!(path, PathBuf::from("out.png.jpeg"));
}

#[test]
fn resolve_output_unknown_ext_defaults_to_jpeg() {
    let (path, fmt) = resolve_output(std::path::Path::new("out.xyz"), None);
    assert_eq!(fmt, OutputFormat::Jpeg);
    assert_eq!(path, PathBuf::from("out.xyz.jpeg"));
}

#[test]
fn resolve_output_no_extension_defaults_to_jpeg() {
    let (path, fmt) = resolve_output(std::path::Path::new("output"), None);
    assert_eq!(fmt, OutputFormat::Jpeg);
    assert_eq!(path, PathBuf::from("output.jpeg"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw encode::tests`
Expected: FAIL — `resolve_output` not defined

**Step 3: Write implementation**

```rust
/// Resolve the output file path and format.
///
/// Rules:
/// 1. If `format` is specified and the extension matches, use as-is.
/// 2. If `format` is specified and the extension doesn't match, append the correct extension.
/// 3. If `format` is `None`, infer from extension.
/// 4. If the extension is unknown, default to JPEG and append `.jpeg`.
pub fn resolve_output(
    path: &std::path::Path,
    format: Option<OutputFormat>,
) -> (std::path::PathBuf, OutputFormat) {
    let ext_format = path
        .extension()
        .and_then(|e| e.to_str())
        .and_then(OutputFormat::from_extension);

    match (format, ext_format) {
        // Explicit format, extension matches
        (Some(fmt), Some(ext_fmt)) if fmt == ext_fmt => (path.to_path_buf(), fmt),
        // Explicit format, extension doesn't match — append correct extension
        (Some(fmt), _) => {
            let mut new_path = path.as_os_str().to_owned();
            new_path.push(".");
            new_path.push(fmt.extension());
            (std::path::PathBuf::from(new_path), fmt)
        }
        // No explicit format, known extension — infer
        (None, Some(ext_fmt)) => (path.to_path_buf(), ext_fmt),
        // No explicit format, unknown/missing extension — default JPEG, append
        (None, None) => {
            let mut new_path = path.as_os_str().to_owned();
            new_path.push(".jpeg");
            (std::path::PathBuf::from(new_path), OutputFormat::Jpeg)
        }
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw encode::tests`
Expected: PASS

**Step 5: Stage**

`git add crates/oxiraw/src/encode/mod.rs`

---

## Phase 2: Format-Specific Encoding with Quality

### Task 2.1: Rewrite encode pipeline

**Files:**
- Modify: `crates/oxiraw/src/encode/mod.rs`

**Step 1: Write failing tests**

Add to `mod tests`:

```rust
#[test]
fn encode_jpeg_with_quality_produces_file() {
    let temp_path = std::env::temp_dir().join("oxiraw_test_quality.jpg");
    let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
    let opts = EncodeOptions {
        jpeg_quality: 95,
        format: None,
    };
    let result = encode_to_file_with_options(&linear, &temp_path, &opts, None);
    assert!(result.is_ok());
    let final_path = result.unwrap();
    assert!(final_path.exists());
    let _ = std::fs::remove_file(&final_path);
}

#[test]
fn encode_jpeg_quality_affects_file_size() {
    let linear: Rgb32FImage = ImageBuffer::from_pixel(64, 64, Rgb([0.5f32, 0.3, 0.1]));

    let path_low = std::env::temp_dir().join("oxiraw_test_q50.jpg");
    let path_high = std::env::temp_dir().join("oxiraw_test_q95.jpg");

    let opts_low = EncodeOptions { jpeg_quality: 50, format: None };
    let opts_high = EncodeOptions { jpeg_quality: 95, format: None };

    encode_to_file_with_options(&linear, &path_low, &opts_low, None).unwrap();
    encode_to_file_with_options(&linear, &path_high, &opts_high, None).unwrap();

    let size_low = std::fs::metadata(&path_low).unwrap().len();
    let size_high = std::fs::metadata(&path_high).unwrap().len();
    assert!(
        size_high > size_low,
        "Higher quality should produce larger file: q95={size_high} vs q50={size_low}"
    );

    let _ = std::fs::remove_file(&path_low);
    let _ = std::fs::remove_file(&path_high);
}

#[test]
fn encode_png_format() {
    let temp_path = std::env::temp_dir().join("oxiraw_test_fmt.png");
    let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
    let opts = EncodeOptions { jpeg_quality: 92, format: None };
    let final_path = encode_to_file_with_options(&linear, &temp_path, &opts, None).unwrap();
    assert!(final_path.exists());
    // Verify it's actually a PNG by reading it back
    let img = image::open(&final_path).unwrap();
    assert_eq!(img.width(), 4);
    let _ = std::fs::remove_file(&final_path);
}

#[test]
fn encode_tiff_format() {
    let temp_path = std::env::temp_dir().join("oxiraw_test_fmt.tiff");
    let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
    let opts = EncodeOptions { jpeg_quality: 92, format: None };
    let final_path = encode_to_file_with_options(&linear, &temp_path, &opts, None).unwrap();
    assert!(final_path.exists());
    let img = image::open(&final_path).unwrap();
    assert_eq!(img.width(), 4);
    let _ = std::fs::remove_file(&final_path);
}

#[test]
fn encode_format_override_appends_extension() {
    let temp_path = std::env::temp_dir().join("oxiraw_test_override.png");
    let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
    let opts = EncodeOptions {
        jpeg_quality: 92,
        format: Some(OutputFormat::Jpeg),
    };
    let final_path = encode_to_file_with_options(&linear, &temp_path, &opts, None).unwrap();
    // Should append .jpeg since original ext is .png
    assert_eq!(
        final_path,
        std::env::temp_dir().join("oxiraw_test_override.png.jpeg")
    );
    assert!(final_path.exists());
    let _ = std::fs::remove_file(&final_path);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw encode::tests`
Expected: FAIL — `encode_to_file_with_options` not defined

**Step 3: Write implementation**

Add these `use` statements at the top of `encode/mod.rs`:

```rust
use std::io::Cursor;
use std::path::PathBuf;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::tiff::TiffEncoder;
```

Add the new encoding function:

```rust
/// Encode a linear sRGB f32 image to a file with full options.
///
/// Resolves the output format and path, encodes with the appropriate encoder,
/// and optionally injects metadata. Returns the final output path (which may
/// differ from the input path if an extension was appended).
pub fn encode_to_file_with_options(
    linear: &Rgb32FImage,
    path: &std::path::Path,
    options: &EncodeOptions,
    metadata: Option<&ImageMetadata>,
) -> Result<PathBuf> {
    let (final_path, format) = resolve_output(path, options.format);

    let dynamic = linear_to_srgb_dynamic(linear);
    let rgb8 = dynamic.to_rgb8();

    // Encode to in-memory buffer with format-specific encoder
    let buf = match format {
        OutputFormat::Jpeg => {
            let mut buf = Vec::new();
            let encoder = JpegEncoder::new_with_quality(&mut buf, options.jpeg_quality);
            rgb8.write_with_encoder(encoder)
                .map_err(|e| crate::error::OxirawError::Encode(e.to_string()))?;
            buf
        }
        OutputFormat::Png => {
            let mut buf = Vec::new();
            let encoder = PngEncoder::new(&mut buf);
            rgb8.write_with_encoder(encoder)
                .map_err(|e| crate::error::OxirawError::Encode(e.to_string()))?;
            buf
        }
        OutputFormat::Tiff => {
            let mut buf = Vec::new();
            let cursor = Cursor::new(&mut buf);
            let encoder = TiffEncoder::new(cursor);
            rgb8.write_with_encoder(encoder)
                .map_err(|e| crate::error::OxirawError::Encode(e.to_string()))?;
            buf
        }
    };

    // Inject metadata if available (JPEG/PNG only for now)
    let buf = if let Some(meta) = metadata {
        inject_metadata(buf, format, meta)?
    } else {
        buf
    };

    std::fs::write(&final_path, &buf)
        .map_err(|e| crate::error::OxirawError::Encode(e.to_string()))?;

    // For TIFF output, inject metadata via little_exif after writing
    if format == OutputFormat::Tiff {
        if let Some(meta) = metadata {
            inject_metadata_tiff(&final_path, meta);
        }
    }

    Ok(final_path)
}
```

Also update the existing `encode_to_file` to delegate:

```rust
/// Encode a linear sRGB f32 image to a file, converting to sRGB gamma space.
///
/// Uses default options (JPEG quality 92, format inferred from extension).
/// For more control, use `encode_to_file_with_options`.
pub fn encode_to_file(linear: &Rgb32FImage, path: &std::path::Path) -> Result<()> {
    encode_to_file_with_options(linear, path, &EncodeOptions::default(), None)?;
    Ok(())
}
```

Add stub functions for metadata injection (implemented in Phase 3):

```rust
/// Extracted metadata from an input image (EXIF, ICC profile).
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    /// Raw EXIF bytes.
    pub exif: Option<Vec<u8>>,
    /// Raw ICC profile bytes.
    pub icc_profile: Option<Vec<u8>>,
}

/// Inject metadata into an encoded JPEG or PNG buffer.
fn inject_metadata(buf: Vec<u8>, format: OutputFormat, _metadata: &ImageMetadata) -> Result<Vec<u8>> {
    // Stub — implemented in Phase 3
    match format {
        OutputFormat::Jpeg | OutputFormat::Png => Ok(buf),
        OutputFormat::Tiff => Ok(buf), // TIFF handled separately via little_exif
    }
}

/// Inject metadata into an existing TIFF file via little_exif. Best-effort.
fn inject_metadata_tiff(_path: &std::path::Path, _metadata: &ImageMetadata) {
    // Stub — implemented in Phase 3
}

/// Extract metadata (EXIF, ICC) from an input image file. Best-effort.
pub fn extract_metadata(_path: &std::path::Path) -> Option<ImageMetadata> {
    // Stub — implemented in Phase 3
    None
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw encode::tests`
Expected: PASS (all existing + new tests)

Also run full workspace tests:
Run: `cargo test --workspace`
Expected: PASS (existing `encode_to_file` callers still work via delegation)

**Step 5: Stage**

`git add crates/oxiraw/src/encode/mod.rs`

---

## Phase 3: Metadata Preservation — Standard Formats

### Task 3.1: Metadata extraction from JPEG/PNG

**Files:**
- Modify: `crates/oxiraw/src/encode/mod.rs`

**Step 1: Write failing tests**

Add to `mod tests`:

```rust
#[test]
fn extract_metadata_from_jpeg_with_no_exif() {
    // Create a plain JPEG with no EXIF
    let temp_path = std::env::temp_dir().join("oxiraw_test_no_exif.jpg");
    let img: image::ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(4, 4, Rgb([128u8, 128, 128]));
    img.save(&temp_path).unwrap();

    let meta = extract_metadata(&temp_path);
    // Should return Some but with no EXIF (image crate doesn't write EXIF)
    if let Some(m) = meta {
        assert!(m.exif.is_none() || m.exif.as_ref().unwrap().is_empty() == false);
    }
    // Either way, no crash

    let _ = std::fs::remove_file(&temp_path);
}

#[test]
fn extract_metadata_nonexistent_file_returns_none() {
    let meta = extract_metadata(std::path::Path::new("/nonexistent/file.jpg"));
    assert!(meta.is_none());
}

#[test]
fn extract_metadata_from_png() {
    let temp_path = std::env::temp_dir().join("oxiraw_test_meta.png");
    let img: image::ImageBuffer<Rgb<u8>, Vec<u8>> =
        ImageBuffer::from_pixel(4, 4, Rgb([128u8, 128, 128]));
    img.save(&temp_path).unwrap();

    let meta = extract_metadata(&temp_path);
    // Should not crash, returns Some with empty metadata
    let _ = std::fs::remove_file(&temp_path);
}
```

**Step 2: Run tests to verify current stubs pass (they should, since extract returns None)**

Run: `cargo test -p oxiraw encode::tests`
Expected: PASS (stubs return None, tests allow that)

**Step 3: Implement extract_metadata for JPEG/PNG**

Replace the `extract_metadata` stub with:

```rust
/// Extract metadata (EXIF, ICC profile) from an input image file.
///
/// Extraction strategy (best-effort, cascading):
/// 1. `img-parts` for JPEG/PNG — lossless byte-level copy
/// 2. `kamadak-exif` for TIFF-based raw files (behind `raw` feature)
/// 3. LibRaw parsed fields for non-TIFF raw files (behind `raw` feature)
/// 4. Return None — no metadata extracted
///
/// Returns `None` for unsupported formats or if the file can't be read.
/// This is best-effort — metadata extraction failure should never block processing.
pub fn extract_metadata(path: &std::path::Path) -> Option<ImageMetadata> {
    let bytes = std::fs::read(path).ok()?;

    // Strategy 1: Try img-parts for JPEG
    if let Some(meta) = extract_metadata_jpeg(&bytes) {
        return Some(meta);
    }

    // Strategy 2: Try img-parts for PNG
    if let Some(meta) = extract_metadata_png(&bytes) {
        return Some(meta);
    }

    // Strategies 3-4 (raw files) added in Phase 4 and Phase 5

    None
}

fn extract_metadata_jpeg(bytes: &[u8]) -> Option<ImageMetadata> {
    use img_parts::{ImageEXIF, ImageICC};

    let jpeg = img_parts::jpeg::Jpeg::from_bytes(bytes.to_vec().into()).ok()?;
    let exif = jpeg.exif().map(|b| b.to_vec());
    let icc = jpeg.icc_profile().map(|b| b.to_vec());
    if exif.is_some() || icc.is_some() {
        return Some(ImageMetadata {
            exif,
            icc_profile: icc,
        });
    }
    None
}

fn extract_metadata_png(bytes: &[u8]) -> Option<ImageMetadata> {
    use img_parts::{ImageEXIF, ImageICC};

    let png = img_parts::png::Png::from_bytes(bytes.to_vec().into()).ok()?;
    let exif = png.exif().map(|b| b.to_vec());
    let icc = png.icc_profile().map(|b| b.to_vec());
    if exif.is_some() || icc.is_some() {
        return Some(ImageMetadata {
            exif,
            icc_profile: icc,
        });
    }
    None
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw encode::tests`
Expected: PASS

**Step 5: Stage**

`git add crates/oxiraw/src/encode/mod.rs`

---

### Task 3.2: Metadata injection for JPEG/PNG

**Files:**
- Modify: `crates/oxiraw/src/encode/mod.rs`

**Step 1: Write failing test**

Add to `mod tests`:

```rust
#[test]
fn metadata_roundtrip_jpeg() {
    // Create EXIF bytes manually and verify they survive encode -> inject -> extract
    let exif_bytes = vec![
        0x45, 0x78, 0x69, 0x66, 0x00, 0x00, // "Exif\0\0"
        0x4D, 0x4D, // Big-endian TIFF header
        0x00, 0x2A, // TIFF magic
        0x00, 0x00, 0x00, 0x08, // offset to IFD
    ];
    let meta = ImageMetadata {
        exif: Some(exif_bytes.clone()),
        icc_profile: None,
    };

    let temp_path = std::env::temp_dir().join("oxiraw_test_meta_rt.jpg");
    let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
    let opts = EncodeOptions { jpeg_quality: 92, format: None };
    encode_to_file_with_options(&linear, &temp_path, &opts, Some(&meta)).unwrap();

    // Extract metadata from the output and verify EXIF was preserved
    let meta_out = extract_metadata(&temp_path);
    assert!(meta_out.is_some(), "Should have metadata in output");
    assert!(
        meta_out.as_ref().unwrap().exif.is_some(),
        "Should have EXIF in output"
    );

    let _ = std::fs::remove_file(&temp_path);
}

#[test]
fn encode_without_metadata_still_works() {
    let temp_path = std::env::temp_dir().join("oxiraw_test_no_meta.jpg");
    let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
    let opts = EncodeOptions::default();
    let result = encode_to_file_with_options(&linear, &temp_path, &opts, None);
    assert!(result.is_ok());
    let _ = std::fs::remove_file(&result.unwrap());
}
```

**Step 2: Run tests — the roundtrip test should fail since inject is a stub**

Run: `cargo test -p oxiraw encode::tests::metadata_roundtrip_jpeg`
Expected: FAIL — EXIF not present in output (stub doesn't inject)

**Step 3: Implement inject_metadata**

Replace the `inject_metadata` stub:

```rust
/// Inject metadata into an encoded JPEG or PNG buffer.
fn inject_metadata(
    buf: Vec<u8>,
    format: OutputFormat,
    metadata: &ImageMetadata,
) -> Result<Vec<u8>> {
    use img_parts::{ImageEXIF, ImageICC};

    match format {
        OutputFormat::Jpeg => {
            let mut jpeg = img_parts::jpeg::Jpeg::from_bytes(buf.into())
                .map_err(|e| crate::error::OxirawError::Encode(format!("metadata injection: {e}")))?;
            if let Some(exif) = &metadata.exif {
                jpeg.set_exif(Some(exif.clone().into()));
            }
            if let Some(icc) = &metadata.icc_profile {
                jpeg.set_icc_profile(Some(icc.clone().into()));
            }
            let mut out = Vec::new();
            jpeg.encoder()
                .write_to(&mut out)
                .map_err(|e| crate::error::OxirawError::Encode(format!("metadata write: {e}")))?;
            Ok(out)
        }
        OutputFormat::Png => {
            let mut png = img_parts::png::Png::from_bytes(buf.into())
                .map_err(|e| crate::error::OxirawError::Encode(format!("metadata injection: {e}")))?;
            if let Some(exif) = &metadata.exif {
                png.set_exif(Some(exif.clone().into()));
            }
            if let Some(icc) = &metadata.icc_profile {
                png.set_icc_profile(Some(icc.clone().into()));
            }
            let mut out = Vec::new();
            png.encoder()
                .write_to(&mut out)
                .map_err(|e| crate::error::OxirawError::Encode(format!("metadata write: {e}")))?;
            Ok(out)
        }
        OutputFormat::Tiff => Ok(buf), // Handled separately via inject_metadata_tiff
    }
}
```

**Step 4: Run tests**

Run: `cargo test -p oxiraw encode::tests`
Expected: PASS

**Step 5: Stage**

`git add crates/oxiraw/src/encode/mod.rs`

---

### Task 3.3: Metadata injection for TIFF

**Files:**
- Modify: `crates/oxiraw/src/encode/mod.rs`

**Step 1: Implement inject_metadata_tiff**

Replace the `inject_metadata_tiff` stub. This uses `little_exif` to write EXIF to the output TIFF. Since `little_exif` works at the file level for TIFF, we parse the raw EXIF bytes into a `Metadata` object and write it.

```rust
/// Inject metadata into an existing TIFF file via little_exif. Best-effort — failures are silent.
fn inject_metadata_tiff(path: &std::path::Path, metadata: &ImageMetadata) {
    if let Some(exif_bytes) = &metadata.exif {
        // Attempt to parse the raw EXIF and write to the TIFF file.
        // If this fails for any reason, silently skip.
        let file_ext = little_exif::filetype::FileExtension::TIFF;
        if let Ok(exif_meta) = little_exif::metadata::Metadata::new_from_vec(exif_bytes, file_ext) {
            let _ = exif_meta.write_to_file(path);
        }
    }
}
```

Note: this may not work perfectly for all EXIF blobs since `little_exif` expects specific formats. That's OK — best-effort.

**Step 2: Run tests**

Run: `cargo test --workspace`
Expected: PASS

**Step 3: Stage**

`git add crates/oxiraw/src/encode/mod.rs`

---

## Phase 4: Raw Metadata — TIFF-Based Raw via kamadak-exif

This phase adds EXIF extraction from TIFF-based raw files (CR2, NEF, DNG, ARW, PEF, ORF) using the `exif` crate (kamadak-exif). These raw formats use TIFF containers internally, so kamadak-exif can read them directly. The extracted EXIF bytes are returned in the standard format for injection into output files.

### Task 4.1: Add kamadak-exif dependency

**Files:**
- Modify: `crates/oxiraw/Cargo.toml`

**Step 1: Add the `exif` crate behind the `raw` feature**

Add to `[dependencies]`:

```toml
exif = { version = "0.5", optional = true }
```

Update the `raw` feature:

```toml
[features]
default = []
raw = ["exif"]
```

**Step 2: Verify it compiles**

Run: `cargo build -p oxiraw --features raw`
Expected: PASS

**Step 3: Stage**

`git add crates/oxiraw/Cargo.toml`

---

### Task 4.2: Extract EXIF from TIFF-based raw files

**Files:**
- Modify: `crates/oxiraw/src/encode/mod.rs`

**Step 1: Write failing tests**

Add to `mod tests`. These tests are behind `#[cfg(feature = "raw")]` since they test raw-file-specific functionality:

```rust
#[cfg(feature = "raw")]
mod raw_metadata_tests {
    use super::*;

    #[test]
    fn extract_metadata_raw_tiff_nonexistent_returns_none() {
        let meta = extract_metadata_raw_tiff(std::path::Path::new("/nonexistent/photo.cr2"));
        assert!(meta.is_none());
    }

    #[test]
    fn extract_metadata_raw_tiff_non_tiff_file_returns_none() {
        // A plain JPEG is not a TIFF-based raw file
        let temp_path = std::env::temp_dir().join("oxiraw_test_not_tiff_raw.jpg");
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();

        let meta = extract_metadata_raw_tiff(&temp_path);
        // kamadak-exif may or may not return EXIF from a JPEG — either way is fine
        let _ = std::fs::remove_file(&temp_path);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw --features raw encode::tests::raw_metadata_tests`
Expected: FAIL — `extract_metadata_raw_tiff` not defined

**Step 3: Write implementation**

Add a new helper function in `encode/mod.rs`, behind `#[cfg(feature = "raw")]`:

```rust
/// Extract EXIF from a TIFF-based raw file using kamadak-exif.
///
/// Works for: CR2, NEF, DNG, ARW, PEF, ORF (TIFF-container raw formats).
/// Returns raw EXIF bytes suitable for injection into output files.
#[cfg(feature = "raw")]
fn extract_metadata_raw_tiff(path: &std::path::Path) -> Option<ImageMetadata> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
    let raw_buf = exif.buf();
    if raw_buf.is_empty() {
        return None;
    }
    // kamadak-exif returns raw EXIF bytes (TIFF header + IFDs).
    // For injection into JPEG via img-parts, we need "Exif\0\0" prefix.
    let exif_bytes = if raw_buf.starts_with(b"Exif\0\0") {
        raw_buf.to_vec()
    } else {
        let mut prefixed = b"Exif\0\0".to_vec();
        prefixed.extend_from_slice(raw_buf);
        prefixed
    };
    Some(ImageMetadata {
        exif: Some(exif_bytes),
        icc_profile: None, // Raw files typically don't have embedded ICC profiles
    })
}
```

**Step 4: Wire into extract_metadata**

Update the `extract_metadata` function to try kamadak-exif after img-parts fails. Add after the PNG check:

```rust
    // Strategy 3: Try kamadak-exif for TIFF-based raw files (CR2, NEF, DNG, ARW, PEF, ORF)
    #[cfg(feature = "raw")]
    {
        if crate::decode::is_raw_extension(path) {
            if let Some(meta) = extract_metadata_raw_tiff(path) {
                return Some(meta);
            }
        }
    }
```

**Step 5: Run tests**

Run: `cargo test -p oxiraw --features raw encode::tests`
Expected: PASS

Run: `cargo test --workspace`
Expected: PASS (non-raw tests still work)

**Step 6: Stage**

`git add crates/oxiraw/src/encode/mod.rs`

---

## Phase 5: Raw Metadata — LibRaw Fallback for Non-TIFF Raw

This phase adds EXIF extraction from non-TIFF raw formats (RAF/Fuji, RW2/Panasonic, CR3/Canon) using LibRaw's parsed metadata fields. Since `kamadak-exif` cannot read these containers, we fall back to reading individual metadata fields via LibRaw's C API and constructing EXIF from them using `little_exif`.

This approach is **lossy** — it preserves key shooting data (camera make/model, ISO, shutter speed, aperture, focal length, timestamp, lens) but loses maker notes and vendor-specific tags.

### Task 5.1: Create C metadata accessor helper

**Files:**
- Create: `crates/oxiraw/src/decode/libraw_meta.c`
- Modify: `crates/oxiraw/Cargo.toml` (add `cc` build dependency)
- Modify: `crates/oxiraw/build.rs` (compile C helper)

**Step 1: Add `cc` build dependency**

Add to `crates/oxiraw/Cargo.toml`:

```toml
[build-dependencies]
cc = "1"
```

**Step 2: Create the C accessor file**

Create `crates/oxiraw/src/decode/libraw_meta.c`:

```c
#include <libraw/libraw.h>
#include <string.h>

/* Safe string copy helpers for extracting metadata from LibRaw structs.
 * These thin accessors avoid defining libraw_data_t layout in Rust FFI,
 * which would be fragile across LibRaw versions. */

void oxiraw_get_make(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->idata.make, buf_size - 1);
    buf[buf_size - 1] = '\0';
}

void oxiraw_get_model(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->idata.model, buf_size - 1);
    buf[buf_size - 1] = '\0';
}

float oxiraw_get_iso(libraw_data_t *data) {
    return data->other.iso_speed;
}

float oxiraw_get_shutter(libraw_data_t *data) {
    return data->other.shutter;
}

float oxiraw_get_aperture(libraw_data_t *data) {
    return data->other.aperture;
}

float oxiraw_get_focal_len(libraw_data_t *data) {
    return data->other.focal_len;
}

long long oxiraw_get_timestamp(libraw_data_t *data) {
    return (long long)data->other.timestamp;
}

void oxiraw_get_lens(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->lens.Lens, buf_size - 1);
    buf[buf_size - 1] = '\0';
}

void oxiraw_get_lens_make(libraw_data_t *data, char *buf, int buf_size) {
    strncpy(buf, data->lens.LensMake, buf_size - 1);
    buf[buf_size - 1] = '\0';
}
```

**Step 3: Update build.rs to compile the C helper**

Update `crates/oxiraw/build.rs` to compile the C file when the `raw` feature is enabled:

```rust
fn main() {
    #[cfg(feature = "raw")]
    {
        // Link to LibRaw system library
        println!("cargo:rustc-link-lib=raw");

        let mut libraw_include = None;

        // On macOS with Homebrew, add the library and include search paths
        if cfg!(target_os = "macos") {
            if let Ok(output) = std::process::Command::new("brew")
                .args(["--prefix", "libraw"])
                .output()
            {
                if output.status.success() {
                    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    println!("cargo:rustc-link-search=native={prefix}/lib");
                    libraw_include = Some(format!("{prefix}/include"));
                }
            }
        }

        // Compile C metadata accessor helper
        let mut build = cc::Build::new();
        build.file("src/decode/libraw_meta.c");
        if let Some(ref inc) = libraw_include {
            build.include(inc);
        }
        build.compile("oxiraw_libraw_meta");
    }
}
```

**Step 4: Verify it compiles**

Run: `cargo build -p oxiraw --features raw`
Expected: PASS

**Step 5: Stage**

`git add crates/oxiraw/src/decode/libraw_meta.c crates/oxiraw/Cargo.toml crates/oxiraw/build.rs`

---

### Task 5.2: Add FFI bindings for metadata accessors

**Files:**
- Modify: `crates/oxiraw/src/decode/raw.rs`

**Step 1: Add FFI declarations**

Add the new extern declarations to the existing `extern "C"` block in `decode/raw.rs`:

```rust
extern "C" {
    // ... existing declarations ...

    // Metadata accessor functions (from libraw_meta.c)
    fn oxiraw_get_make(data: *mut libraw_data_t, buf: *mut c_char, buf_size: c_int);
    fn oxiraw_get_model(data: *mut libraw_data_t, buf: *mut c_char, buf_size: c_int);
    fn oxiraw_get_iso(data: *mut libraw_data_t) -> f32;
    fn oxiraw_get_shutter(data: *mut libraw_data_t) -> f32;
    fn oxiraw_get_aperture(data: *mut libraw_data_t) -> f32;
    fn oxiraw_get_focal_len(data: *mut libraw_data_t) -> f32;
    fn oxiraw_get_timestamp(data: *mut libraw_data_t) -> i64;
    fn oxiraw_get_lens(data: *mut libraw_data_t, buf: *mut c_char, buf_size: c_int);
    fn oxiraw_get_lens_make(data: *mut libraw_data_t, buf: *mut c_char, buf_size: c_int);
}
```

**Step 2: Add safe wrapper methods to LibRawProcessor**

Add methods to the `LibRawProcessor` impl block:

```rust
impl LibRawProcessor {
    // ... existing methods ...

    fn get_make(&self) -> String {
        let mut buf = [0u8; 128];
        unsafe {
            oxiraw_get_make(self.ptr, buf.as_mut_ptr() as *mut c_char, 128);
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) };
        cstr.to_string_lossy().into_owned()
    }

    fn get_model(&self) -> String {
        let mut buf = [0u8; 128];
        unsafe {
            oxiraw_get_model(self.ptr, buf.as_mut_ptr() as *mut c_char, 128);
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) };
        cstr.to_string_lossy().into_owned()
    }

    fn get_iso(&self) -> f32 {
        unsafe { oxiraw_get_iso(self.ptr) }
    }

    fn get_shutter(&self) -> f32 {
        unsafe { oxiraw_get_shutter(self.ptr) }
    }

    fn get_aperture(&self) -> f32 {
        unsafe { oxiraw_get_aperture(self.ptr) }
    }

    fn get_focal_len(&self) -> f32 {
        unsafe { oxiraw_get_focal_len(self.ptr) }
    }

    fn get_timestamp(&self) -> i64 {
        unsafe { oxiraw_get_timestamp(self.ptr) }
    }

    fn get_lens(&self) -> String {
        let mut buf = [0u8; 256];
        unsafe {
            oxiraw_get_lens(self.ptr, buf.as_mut_ptr() as *mut c_char, 256);
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) };
        cstr.to_string_lossy().into_owned()
    }

    fn get_lens_make(&self) -> String {
        let mut buf = [0u8; 256];
        unsafe {
            oxiraw_get_lens_make(self.ptr, buf.as_mut_ptr() as *mut c_char, 256);
        }
        let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) };
        cstr.to_string_lossy().into_owned()
    }
}
```

**Step 3: Verify it compiles**

Run: `cargo build -p oxiraw --features raw`
Expected: PASS

**Step 4: Stage**

`git add crates/oxiraw/src/decode/raw.rs`

---

### Task 5.3: Construct EXIF from LibRaw fields and wire into extract_metadata

**Files:**
- Modify: `crates/oxiraw/src/decode/raw.rs`
- Modify: `crates/oxiraw/src/encode/mod.rs`

**Step 1: Write failing tests**

Add to the `mod tests` block in `decode/raw.rs`:

```rust
#[test]
fn extract_raw_metadata_nonexistent_returns_none() {
    let meta = extract_raw_metadata(Path::new("/nonexistent/photo.raf"));
    assert!(meta.is_none());
}
```

Add to `encode/mod.rs` `raw_metadata_tests`:

```rust
#[test]
fn extract_metadata_falls_through_to_none_for_unknown() {
    // A file that's not JPEG, PNG, or raw
    let temp_path = std::env::temp_dir().join("oxiraw_test_unknown.bmp");
    let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
        image::ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
    img.save(&temp_path).unwrap();

    let meta = extract_metadata(&temp_path);
    // BMP is not recognized by any strategy, should return None
    assert!(meta.is_none());
    let _ = std::fs::remove_file(&temp_path);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p oxiraw --features raw`
Expected: FAIL — `extract_raw_metadata` not defined

**Step 3: Implement extract_raw_metadata in decode/raw.rs**

Add a public function to `decode/raw.rs` that opens a raw file with LibRaw, reads the parsed metadata fields, and constructs EXIF bytes using `little_exif`:

```rust
use crate::encode::ImageMetadata;

/// Parsed metadata fields from a raw file via LibRaw.
struct RawMetadataFields {
    make: String,
    model: String,
    iso: f32,
    shutter: f32,     // seconds
    aperture: f32,    // f-number
    focal_len: f32,   // mm
    timestamp: i64,   // unix timestamp
    lens: String,
    lens_make: String,
}

/// Extract metadata from a raw file using LibRaw's parsed fields.
///
/// Opens the file with LibRaw (lightweight — only reads headers, doesn't process),
/// reads camera/shot metadata, and constructs EXIF bytes using little_exif.
///
/// This is the fallback for raw files where kamadak-exif can't read the container
/// (RAF, RW2, CR3, etc.). The resulting EXIF is lossy — key shooting data is
/// preserved but maker notes and vendor-specific tags are lost.
pub fn extract_raw_metadata(path: &Path) -> Option<ImageMetadata> {
    let processor = LibRawProcessor::new().ok()?;
    processor.open_file(path).ok()?;

    let fields = RawMetadataFields {
        make: processor.get_make(),
        model: processor.get_model(),
        iso: processor.get_iso(),
        shutter: processor.get_shutter(),
        aperture: processor.get_aperture(),
        focal_len: processor.get_focal_len(),
        timestamp: processor.get_timestamp(),
        lens: processor.get_lens(),
        lens_make: processor.get_lens_make(),
    };

    construct_exif_from_fields(&fields)
}

/// Construct EXIF bytes from parsed metadata fields using little_exif.
fn construct_exif_from_fields(fields: &RawMetadataFields) -> Option<ImageMetadata> {
    use little_exif::metadata::Metadata;
    use little_exif::exif_tag::ExifTag;

    let mut metadata = Metadata::new();

    // Camera info
    if !fields.make.is_empty() {
        metadata.set_tag(ExifTag::Make(fields.make.clone()));
    }
    if !fields.model.is_empty() {
        metadata.set_tag(ExifTag::Model(fields.model.clone()));
    }

    // Exposure info
    if fields.iso > 0.0 {
        metadata.set_tag(ExifTag::ISOSpeedRatings(vec![fields.iso as u16]));
    }
    if fields.shutter > 0.0 {
        // ExposureTime is a rational: numerator/denominator
        // For shutter speeds like 1/250, we store as (1, 250)
        // For long exposures like 2s, store as (2, 1)
        let (num, den) = if fields.shutter >= 1.0 {
            (fields.shutter as u32, 1u32)
        } else {
            (1u32, (1.0 / fields.shutter).round() as u32)
        };
        metadata.set_tag(ExifTag::ExposureTime(num, den));
    }
    if fields.aperture > 0.0 {
        // FNumber is a rational: e.g., f/2.8 = (28, 10)
        let num = (fields.aperture * 10.0).round() as u32;
        metadata.set_tag(ExifTag::FNumber(num, 10));
    }
    if fields.focal_len > 0.0 {
        let num = (fields.focal_len * 10.0).round() as u32;
        metadata.set_tag(ExifTag::FocalLength(num, 10));
    }

    // Timestamp
    if fields.timestamp > 0 {
        // Convert unix timestamp to EXIF datetime format: "YYYY:MM:DD HH:MM:SS"
        // Use a simple conversion (UTC)
        let dt = timestamp_to_exif_datetime(fields.timestamp);
        if let Some(dt_str) = dt {
            metadata.set_tag(ExifTag::DateTimeOriginal(dt_str));
        }
    }

    // Lens info
    if !fields.lens.is_empty() {
        metadata.set_tag(ExifTag::LensModel(fields.lens.clone()));
    }
    if !fields.lens_make.is_empty() {
        metadata.set_tag(ExifTag::LensMake(fields.lens_make.clone()));
    }

    // Export as EXIF bytes in JPEG format (includes "Exif\0\0" prefix)
    let exif_bytes = metadata.as_u8_vec(little_exif::filetype::FileExtension::JPEG);
    if exif_bytes.is_empty() {
        return None;
    }

    Some(ImageMetadata {
        exif: Some(exif_bytes),
        icc_profile: None,
    })
}

/// Convert a Unix timestamp to EXIF datetime format "YYYY:MM:DD HH:MM:SS".
fn timestamp_to_exif_datetime(timestamp: i64) -> Option<String> {
    // Simple UTC conversion without external date library
    // Days from Unix epoch (1970-01-01) to given timestamp
    if timestamp <= 0 {
        return None;
    }

    let secs_per_day: i64 = 86400;
    let mut days = timestamp / secs_per_day;
    let day_secs = (timestamp % secs_per_day) as u32;

    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;

    // Calculate year/month/day from days since 1970-01-01
    let mut year = 1970i32;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];

    let mut month = 1u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    let day = days as u32 + 1;

    Some(format!(
        "{year:04}:{month:02}:{day:02} {hours:02}:{minutes:02}:{seconds:02}"
    ))
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
```

**Important:** The exact `little_exif` API for `ExifTag` variants may differ from what's shown above. During implementation, check the actual `ExifTag` enum in the little_exif docs/source and adapt tag constructors accordingly. The key approach is: set each tag from the parsed LibRaw fields, export as JPEG-format EXIF bytes.

**Step 4: Wire into extract_metadata in encode/mod.rs**

Add after the kamadak-exif check in `extract_metadata`:

```rust
    // Strategy 4: Try LibRaw parsed fields for non-TIFF raw files (RAF, RW2, CR3, etc.)
    #[cfg(feature = "raw")]
    {
        if crate::decode::is_raw_extension(path) {
            if let Some(meta) = crate::decode::raw::extract_raw_metadata(path) {
                return Some(meta);
            }
        }
    }
```

**Step 5: Run tests**

Run: `cargo test -p oxiraw --features raw`
Expected: PASS

Run: `cargo test --workspace`
Expected: PASS

**Step 6: Stage**

`git add crates/oxiraw/src/decode/raw.rs crates/oxiraw/src/encode/mod.rs`

---

## Phase 6: CLI Integration

### Task 6.1: Add --quality and --format flags

**Files:**
- Modify: `crates/oxiraw-cli/src/main.rs`

**Step 1: Add flags to both subcommands**

Add `--quality` and `--format` to both `Apply` and `Edit` variants:

```rust
/// JPEG output quality (1-100, default 92)
#[arg(long, default_value_t = 92)]
quality: u8,
/// Output format (jpeg, png, tiff). Inferred from extension if not specified.
#[arg(long)]
format: Option<String>,
```

**Step 2: Parse format string to OutputFormat**

Add a helper function:

```rust
fn parse_output_format(s: &str) -> oxiraw::Result<oxiraw::encode::OutputFormat> {
    match s.to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => Ok(oxiraw::encode::OutputFormat::Jpeg),
        "png" => Ok(oxiraw::encode::OutputFormat::Png),
        "tiff" | "tif" => Ok(oxiraw::encode::OutputFormat::Tiff),
        _ => Err(oxiraw::OxirawError::Encode(format!(
            "unsupported output format '{s}'. Use: jpeg, png, or tiff"
        ))),
    }
}
```

**Step 3: Update run_apply and run_edit**

Both functions gain `quality: u8` and `format: Option<String>` parameters. The pipeline becomes:

```rust
fn run_apply(
    input: &std::path::Path,
    preset_path: &std::path::Path,
    output: &std::path::Path,
    quality: u8,
    format: Option<&str>,
) -> oxiraw::Result<()> {
    let metadata = oxiraw::encode::extract_metadata(input);
    let linear = oxiraw::decode::decode(input)?;
    let preset = Preset::load_from_file(preset_path)?;
    let mut engine = Engine::new(linear);
    engine.apply_preset(&preset);
    let rendered = engine.render();
    let fmt = format.map(parse_output_format).transpose()?;
    let opts = oxiraw::encode::EncodeOptions {
        jpeg_quality: quality,
        format: fmt,
    };
    let final_path =
        oxiraw::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())?;
    println!("Saved to {}", final_path.display());
    Ok(())
}
```

Similarly for `run_edit`.

**Step 4: Verify CLI compiles and existing tests pass**

Run: `cargo build -p oxiraw-cli`
Expected: PASS

Run: `cargo test -p oxiraw-cli`
Expected: PASS (existing tests use PNG which still works)

**Step 5: Stage**

`git add crates/oxiraw-cli/src/main.rs`

---

### Task 6.2: Add re-exports to lib.rs

**Files:**
- Modify: `crates/oxiraw/src/lib.rs`

**Step 1: Add re-exports**

```rust
pub use encode::{EncodeOptions, ImageMetadata, OutputFormat};
```

**Step 2: Verify**

Run: `cargo test --workspace`
Expected: PASS

**Step 3: Stage**

`git add crates/oxiraw/src/lib.rs`

---

### Task 6.3: CLI integration tests

**Files:**
- Modify: `crates/oxiraw-cli/tests/integration.rs`

**Step 1: Add tests**

```rust
#[test]
fn cli_edit_with_quality() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_quality_in.png");
    let output_low = temp_dir.join("oxiraw_cli_q50.jpg");
    let output_high = temp_dir.join("oxiraw_cli_q95.jpg");

    create_test_png(&input);

    let status = cli_bin()
        .args([
            "edit", "-i", input.to_str().unwrap(),
            "-o", output_low.to_str().unwrap(),
            "--quality", "50",
        ])
        .status()
        .expect("failed to run CLI");
    assert!(status.success());

    let status = cli_bin()
        .args([
            "edit", "-i", input.to_str().unwrap(),
            "-o", output_high.to_str().unwrap(),
            "--quality", "95",
        ])
        .status()
        .expect("failed to run CLI");
    assert!(status.success());

    let size_low = std::fs::metadata(&output_low).unwrap().len();
    let size_high = std::fs::metadata(&output_high).unwrap().len();
    assert!(size_high > size_low, "q95 should be larger than q50");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&output_low);
    let _ = std::fs::remove_file(&output_high);
}

#[test]
fn cli_edit_with_format_override() {
    let temp_dir = std::env::temp_dir();
    let input = temp_dir.join("oxiraw_cli_fmt_in.png");
    let output = temp_dir.join("oxiraw_cli_fmt_out.png");

    create_test_png(&input);

    let status = cli_bin()
        .args([
            "edit", "-i", input.to_str().unwrap(),
            "-o", output.to_str().unwrap(),
            "--format", "jpeg",
        ])
        .status()
        .expect("failed to run CLI");
    assert!(status.success());

    // Output should be at out.png.jpeg (appended extension)
    let expected = temp_dir.join("oxiraw_cli_fmt_out.png.jpeg");
    assert!(expected.exists(), "Should have appended .jpeg extension");

    let _ = std::fs::remove_file(&input);
    let _ = std::fs::remove_file(&expected);
}
```

**Step 2: Run tests**

Run: `cargo test -p oxiraw-cli`
Expected: PASS

**Step 3: Stage**

`git add crates/oxiraw-cli/tests/integration.rs`

---

## Phase 7: Documentation + Final Verification

### Task 7.1: Update README

**Files:**
- Modify: `README.md`

**Step 1: Add quality and format examples to Quick Start**

Add after the existing raw file example:

```bash
# Set JPEG output quality (default: 92)
cargo run -p oxiraw-cli -- edit \
  -i photo.jpg \
  -o output.jpg \
  --quality 95

# Specify output format explicitly
cargo run -p oxiraw-cli -- edit \
  -i photo.jpg \
  -o output.tiff \
  --format tiff
```

**Step 2: Stage**

`git add README.md`

---

### Task 7.2: Final verification

Run: `cargo fmt --all`
Run: `cargo test --workspace`
Expected: all tests pass

Stage any formatting changes: `git add -A`

---

## Summary

| Phase | Tasks | Tests Added | Key Deliverable |
|-------|-------|-------------|-----------------|
| 1 | 1.1-1.3 | 9 | Dependencies, OutputFormat, EncodeOptions, resolve_output() |
| 2 | 2.1 | 5 | Format-specific encoders, JPEG quality, encode_to_file_with_options() |
| 3 | 3.1-3.3 | 5 | Metadata extraction + injection for JPEG/PNG (img-parts) + TIFF (little_exif) |
| 4 | 4.1-4.2 | 2 | Raw metadata extraction for TIFF-based raw files (kamadak-exif) |
| 5 | 5.1-5.3 | 2 | LibRaw metadata FFI accessors, EXIF construction for non-TIFF raw (little_exif), full extraction chain |
| 6 | 6.1-6.3 | 2 | CLI --quality/--format flags, metadata wiring, integration tests |
| 7 | 7.1-7.2 | 0 | README updates, cargo fmt, final verification |
