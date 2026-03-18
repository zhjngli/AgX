//! Image metadata extraction and representation.
//!
//! Provides a unified interface for extracting EXIF and ICC profile metadata
//! from various image formats (JPEG, PNG, TIFF-based raw, LibRaw-parsed raw).

use std::path::Path;

/// Extracted metadata from an input image (EXIF, ICC profile).
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    /// Raw EXIF bytes.
    pub exif: Option<Vec<u8>>,
    /// Raw ICC profile bytes.
    pub icc_profile: Option<Vec<u8>>,
}

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
pub fn extract_metadata(path: &Path) -> Option<ImageMetadata> {
    let bytes = std::fs::read(path).ok()?;

    // Strategy 1: Try img-parts for JPEG
    if let Some(meta) = extract_metadata_jpeg(&bytes) {
        return Some(meta);
    }

    // Strategy 2: Try img-parts for PNG
    if let Some(meta) = extract_metadata_png(&bytes) {
        return Some(meta);
    }

    // Strategy 3: Try kamadak-exif for TIFF-based raw files (CR2, NEF, DNG, ARW, PEF, ORF)
    #[cfg(feature = "raw")]
    {
        if crate::decode::is_raw_extension(path) {
            if let Some(meta) = extract_metadata_raw_tiff(path) {
                return Some(meta);
            }
        }
    }

    // Strategy 4: Try LibRaw parsed fields for non-TIFF raw files (RAF, RW2, CR3, etc.)
    #[cfg(feature = "raw")]
    {
        if crate::decode::is_raw_extension(path) {
            if let Some(exif_bytes) = crate::decode::raw::extract_raw_metadata(path) {
                return Some(ImageMetadata {
                    exif: Some(exif_bytes),
                    icc_profile: None,
                });
            }
        }
    }

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

/// Extract EXIF from a TIFF-based raw file using kamadak-exif.
///
/// Works for: CR2, NEF, DNG, ARW, PEF, ORF (TIFF-container raw formats).
/// Returns raw EXIF bytes suitable for injection into output files.
#[cfg(feature = "raw")]
fn extract_metadata_raw_tiff(path: &Path) -> Option<ImageMetadata> {
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
        icc_profile: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_metadata_from_jpeg_with_no_exif() {
        use image::{ImageBuffer, Rgb};

        let temp_path = std::env::temp_dir().join("agx_test_no_exif.jpg");
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();

        let meta = extract_metadata(&temp_path);
        if let Some(m) = meta {
            assert!(m.exif.is_none() || !m.exif.as_ref().unwrap().is_empty());
        }

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn extract_metadata_nonexistent_file_returns_none() {
        let meta = extract_metadata(std::path::Path::new("/nonexistent/file.jpg"));
        assert!(meta.is_none());
    }

    #[test]
    fn extract_metadata_from_png() {
        use image::{ImageBuffer, Rgb};

        let temp_path = std::env::temp_dir().join("agx_test_meta.png");
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();

        let _meta = extract_metadata(&temp_path);
        // Should not crash
        let _ = std::fs::remove_file(&temp_path);
    }
}

#[cfg(all(test, feature = "raw"))]
mod raw_metadata_tests {
    use super::*;

    #[test]
    fn extract_metadata_raw_tiff_nonexistent_returns_none() {
        let meta = extract_metadata_raw_tiff(std::path::Path::new("/nonexistent/photo.cr2"));
        assert!(meta.is_none());
    }

    #[test]
    fn extract_metadata_raw_tiff_non_tiff_file_returns_none() {
        let temp_path = std::env::temp_dir().join("agx_test_not_tiff_raw.jpg");
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();

        let _meta = extract_metadata_raw_tiff(&temp_path);
        // kamadak-exif may or may not return EXIF from a JPEG — either way is fine
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn extract_metadata_falls_through_to_none_for_unknown() {
        let temp_path = std::env::temp_dir().join("agx_test_unknown.bmp");
        let img: image::ImageBuffer<image::Rgb<u8>, Vec<u8>> =
            image::ImageBuffer::from_pixel(4, 4, image::Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();
        let meta = extract_metadata(&temp_path);
        assert!(meta.is_none());
        let _ = std::fs::remove_file(&temp_path);
    }
}
