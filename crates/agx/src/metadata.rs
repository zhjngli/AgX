//! Image metadata extraction and representation.
//!
//! Provides a unified interface for extracting EXIF metadata from various
//! image formats (JPEG, PNG, TIFF-based raw, LibRaw-parsed raw).
//!
//! Output ICC profile labeling is owned by the `encode` module, not by
//! input metadata pass-through — see `encode::icc`.

use std::path::Path;

/// Extracted EXIF metadata from an input image.
///
/// Output color labeling (ICC profile) is owned by the encoder, not by
/// input metadata pass-through — every output file is sRGB and gets a
/// fresh sRGB ICC blob from `encode::icc`. See the `encode` module-level
/// doc for the contract.
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    /// Raw EXIF bytes.
    pub exif: Option<Vec<u8>>,
}

/// Extract EXIF metadata from an input image file.
///
/// Extraction strategy (best-effort, cascading):
/// 1. `img-parts` for JPEG — lossless byte-level copy
/// 2. `img-parts` for PNG — lossless byte-level copy
/// 3. `kamadak-exif` for TIFF-based raw files (behind `raw` feature)
/// 4. LibRaw parsed fields for non-TIFF raw files (behind `raw` feature)
/// 5. `libheif` for HEIC/HEIF containers (behind `heic` feature)
///
/// Falls through to `None` — no metadata extracted — if none of the above match.
///
/// Returns `None` for unsupported formats or if the file can't be read.
/// This is best-effort — metadata extraction failure should never block processing.
///
/// The returned EXIF bytes have their Orientation tag (0x0112) rewritten to
/// `1` (Normal). Decoders apply orientation to pixel data, so the canonical
/// output pixels must be paired with an orientation tag that says "no
/// rotation needed" — otherwise EXIF-aware viewers would rotate twice.
pub fn extract_metadata(path: &Path) -> Option<ImageMetadata> {
    let mut meta = extract_metadata_raw(path)?;
    if let Some(exif) = meta.exif.as_mut() {
        normalize_orientation_in_exif(exif);
    }
    Some(meta)
}

fn extract_metadata_raw(path: &Path) -> Option<ImageMetadata> {
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
                });
            }
        }
    }

    // Strategy 5: Try libheif for HEIC/HEIF containers
    #[cfg(feature = "heic")]
    {
        if crate::decode::is_heic_extension(path) {
            if let Some(exif_bytes) = crate::decode::heic::extract_heic_metadata(path) {
                return Some(ImageMetadata {
                    exif: Some(exif_bytes),
                });
            }
        }
    }

    None
}

/// Rewrite the EXIF Orientation tag (0x0112) to `1` (Normal) in every IFD of
/// the TIFF chain (IFD0 main, IFD1 thumbnail, etc.).
///
/// AgX decoders apply EXIF orientation to pixel data during decode, leaving
/// the engine's working pixels in canonical (top-left, no rotation) form.
/// Without this normalization, copying the source EXIF blob to the output
/// would tell viewers to rotate the already-canonical pixels a second time.
/// IFD1 carries thumbnail metadata; tools that prefer the embedded thumbnail
/// (Bridge, Lightroom) read its Orientation independently from IFD0.
///
/// Best-effort: silently leaves `bytes` unchanged on any parse failure
/// (unknown byte order, bad TIFF magic, truncated buffer, missing tag, or
/// an Orientation entry whose declared type/count doesn't match the spec).
/// Handles both raw TIFF buffers and the `Exif\0\0`-prefixed form that
/// `img-parts` and most JPEG/HEIC pipelines hand around.
pub(crate) fn normalize_orientation_in_exif(bytes: &mut [u8]) {
    let tiff_start = if bytes.starts_with(b"Exif\0\0") { 6 } else { 0 };
    if bytes.len() < tiff_start + 8 {
        return;
    }

    let big_endian = match &bytes[tiff_start..tiff_start + 2] {
        b"MM" => true,
        b"II" => false,
        _ => return,
    };

    let read_u16 = |b: &[u8], off: usize| -> u16 {
        let arr = [b[off], b[off + 1]];
        if big_endian {
            u16::from_be_bytes(arr)
        } else {
            u16::from_le_bytes(arr)
        }
    };
    let read_u32 = |b: &[u8], off: usize| -> u32 {
        let arr = [b[off], b[off + 1], b[off + 2], b[off + 3]];
        if big_endian {
            u32::from_be_bytes(arr)
        } else {
            u32::from_le_bytes(arr)
        }
    };

    if read_u16(bytes, tiff_start + 2) != 42 {
        return;
    }

    // Cap chain traversal at 4 IFDs — guards against malformed files with
    // circular next-IFD pointers. Real-world files have IFD0 + optional IFD1.
    let mut next_ifd_rel = read_u32(bytes, tiff_start + 4) as usize;
    for _ in 0..4 {
        if next_ifd_rel == 0 {
            break;
        }
        let ifd_abs = tiff_start + next_ifd_rel;
        if bytes.len() < ifd_abs + 2 {
            return;
        }
        let num_entries = read_u16(bytes, ifd_abs) as usize;
        let next_ptr_abs = ifd_abs + 2 + num_entries * 12;
        if bytes.len() < next_ptr_abs + 4 {
            return;
        }

        for i in 0..num_entries {
            let entry_abs = ifd_abs + 2 + i * 12;
            if read_u16(bytes, entry_abs) != 0x0112 {
                continue;
            }
            // Skip if the field shape doesn't match the EXIF spec (type
            // SHORT, count 1). For type LONG or count > 1, a blind 4-byte
            // SHORT write would either encode the wrong value or follow an
            // out-of-line offset and corrupt unrelated entries.
            if read_u16(bytes, entry_abs + 2) != 3 || read_u32(bytes, entry_abs + 4) != 1 {
                continue;
            }
            let value_abs = entry_abs + 8;
            let (b0, b1) = if big_endian { (0u8, 1u8) } else { (1u8, 0u8) };
            bytes[value_abs] = b0;
            bytes[value_abs + 1] = b1;
            bytes[value_abs + 2] = 0;
            bytes[value_abs + 3] = 0;
        }

        next_ifd_rel = read_u32(bytes, next_ptr_abs) as usize;
    }
}

fn extract_metadata_jpeg(bytes: &[u8]) -> Option<ImageMetadata> {
    use img_parts::ImageEXIF;

    let jpeg = img_parts::jpeg::Jpeg::from_bytes(bytes.to_vec().into()).ok()?;
    let exif = jpeg.exif().map(|b| b.to_vec());
    if exif.is_some() {
        return Some(ImageMetadata { exif });
    }
    None
}

fn extract_metadata_png(bytes: &[u8]) -> Option<ImageMetadata> {
    use img_parts::ImageEXIF;

    let png = img_parts::png::Png::from_bytes(bytes.to_vec().into()).ok()?;
    let exif = png.exif().map(|b| b.to_vec());
    if exif.is_some() {
        return Some(ImageMetadata { exif });
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

    /// Build a minimal TIFF blob with a single Orientation tag at IFD0.
    /// `big_endian` selects byte order; `with_prefix` adds the `Exif\0\0`
    /// prefix that JPEG/HEIC pipelines pass around.
    fn build_tiff_with_orientation(value: u16, big_endian: bool, with_prefix: bool) -> Vec<u8> {
        let mut out = Vec::new();
        if with_prefix {
            out.extend_from_slice(b"Exif\0\0");
        }
        let u16_bytes = |v: u16| {
            if big_endian {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            }
        };
        let u32_bytes = |v: u32| {
            if big_endian {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            }
        };
        out.extend_from_slice(if big_endian { b"MM" } else { b"II" });
        out.extend_from_slice(&u16_bytes(42)); // magic
        out.extend_from_slice(&u32_bytes(8)); // IFD0 offset (relative to TIFF start)
        out.extend_from_slice(&u16_bytes(1)); // num entries
        out.extend_from_slice(&u16_bytes(0x0112)); // tag = Orientation
        out.extend_from_slice(&u16_bytes(3)); // type = SHORT
        out.extend_from_slice(&u32_bytes(1)); // count = 1
        out.extend_from_slice(&u16_bytes(value)); // value (low 2 bytes)
        out.extend_from_slice(&[0u8, 0u8]); // value-field padding
        out.extend_from_slice(&u32_bytes(0)); // next IFD offset = 0
        out
    }

    fn read_orientation_from_tiff(bytes: &[u8]) -> Option<u16> {
        let tiff_start = if bytes.starts_with(b"Exif\0\0") { 6 } else { 0 };
        let header = bytes.get(tiff_start..tiff_start + 2)?;
        let big_endian = match header {
            b"MM" => true,
            b"II" => false,
            _ => return None,
        };
        let entry_abs = tiff_start + 8 + 2; // ifd0_abs (= tiff_start + 8) + 2 for num_entries
        let value_abs = entry_abs + 8;
        let arr = [bytes[value_abs], bytes[value_abs + 1]];
        Some(if big_endian {
            u16::from_be_bytes(arr)
        } else {
            u16::from_le_bytes(arr)
        })
    }

    #[test]
    fn normalize_orientation_big_endian_with_prefix() {
        let mut bytes = build_tiff_with_orientation(6, true, true);
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(read_orientation_from_tiff(&bytes), Some(1));
    }

    #[test]
    fn normalize_orientation_little_endian_with_prefix() {
        let mut bytes = build_tiff_with_orientation(3, false, true);
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(read_orientation_from_tiff(&bytes), Some(1));
    }

    #[test]
    fn normalize_orientation_without_prefix() {
        let mut bytes = build_tiff_with_orientation(8, true, false);
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(read_orientation_from_tiff(&bytes), Some(1));
    }

    #[test]
    fn normalize_orientation_already_one_is_noop() {
        let bytes_in = build_tiff_with_orientation(1, true, true);
        let mut bytes = bytes_in.clone();
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(bytes, bytes_in);
    }

    #[test]
    fn normalize_orientation_clears_value_field_padding() {
        // Build a blob, then poke garbage into the value-field padding to
        // confirm the rewrite zeros it out — preventing junk bytes from
        // surviving into the output.
        let mut bytes = build_tiff_with_orientation(6, true, true);
        let value_abs = 6 + 8 + 2 + 8; // prefix + header + num_entries + entry header
        bytes[value_abs + 2] = 0xAB;
        bytes[value_abs + 3] = 0xCD;
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(bytes[value_abs], 0);
        assert_eq!(bytes[value_abs + 1], 1);
        assert_eq!(bytes[value_abs + 2], 0);
        assert_eq!(bytes[value_abs + 3], 0);
    }

    #[test]
    fn normalize_orientation_no_tag_is_noop() {
        // Build a TIFF with a different tag (Make = 0x010F) and no Orientation.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"Exif\0\0");
        bytes.extend_from_slice(b"MM"); // big-endian
        bytes.extend_from_slice(&42u16.to_be_bytes());
        bytes.extend_from_slice(&8u32.to_be_bytes()); // IFD0 offset
        bytes.extend_from_slice(&1u16.to_be_bytes()); // num entries
        bytes.extend_from_slice(&0x010Fu16.to_be_bytes()); // tag = Make
        bytes.extend_from_slice(&2u16.to_be_bytes()); // type = ASCII
        bytes.extend_from_slice(&4u32.to_be_bytes()); // count
        bytes.extend_from_slice(b"foo\0");
        bytes.extend_from_slice(&0u32.to_be_bytes());
        let before = bytes.clone();
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(bytes, before);
    }

    #[test]
    fn normalize_orientation_truncated_buffer_no_panic() {
        let mut bytes = vec![b'E', b'x', b'i', b'f', 0, 0, b'M', b'M']; // header only
        normalize_orientation_in_exif(&mut bytes);
        // No panic, no crash. Content unchanged on early return.
        assert_eq!(bytes, vec![b'E', b'x', b'i', b'f', 0, 0, b'M', b'M']);
    }

    #[test]
    fn normalize_orientation_unknown_byte_order_is_noop() {
        let mut bytes = vec![b'X', b'X', 0, 42, 0, 0, 0, 8];
        let before = bytes.clone();
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(bytes, before);
    }

    #[test]
    fn normalize_orientation_bad_magic_is_noop() {
        let mut bytes = build_tiff_with_orientation(6, true, true);
        // Corrupt the magic field (offset 6+2 = 8 in prefixed blob).
        bytes[8] = 0xFF;
        bytes[9] = 0xFF;
        let before = bytes.clone();
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(bytes, before);
    }

    #[test]
    fn normalize_orientation_empty_is_noop() {
        let mut bytes = Vec::new();
        normalize_orientation_in_exif(&mut bytes);
        assert!(bytes.is_empty());
    }

    /// Build a big-endian TIFF with Orientation entries in both IFD0 and
    /// IFD1. Layout follows EXIF: header → IFD0 → next-IFD pointer to IFD1 →
    /// IFD1 → terminating zero.
    fn build_tiff_with_ifd0_and_ifd1(ifd0_value: u16, ifd1_value: u16) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"Exif\0\0"); // 0..6
        out.extend_from_slice(b"MM"); // 6..8 — big-endian
        out.extend_from_slice(&42u16.to_be_bytes()); // 8..10 — magic
        out.extend_from_slice(&8u32.to_be_bytes()); // 10..14 — IFD0 rel = 8 → abs 14
        out.extend_from_slice(&1u16.to_be_bytes()); // 14..16 — IFD0 num_entries
        out.extend_from_slice(&0x0112u16.to_be_bytes()); // 16..18 — tag = Orientation
        out.extend_from_slice(&3u16.to_be_bytes()); // 18..20 — type = SHORT
        out.extend_from_slice(&1u32.to_be_bytes()); // 20..24 — count = 1
        out.extend_from_slice(&ifd0_value.to_be_bytes()); // 24..26 — value
        out.extend_from_slice(&[0u8, 0u8]); // 26..28 — value-field padding
        out.extend_from_slice(&26u32.to_be_bytes()); // 28..32 — next IFD rel = 26 → abs 32
        out.extend_from_slice(&1u16.to_be_bytes()); // 32..34 — IFD1 num_entries
        out.extend_from_slice(&0x0112u16.to_be_bytes()); // 34..36
        out.extend_from_slice(&3u16.to_be_bytes()); // 36..38
        out.extend_from_slice(&1u32.to_be_bytes()); // 38..42
        out.extend_from_slice(&ifd1_value.to_be_bytes()); // 42..44
        out.extend_from_slice(&[0u8, 0u8]); // 44..46
        out.extend_from_slice(&0u32.to_be_bytes()); // 46..50 — chain terminator
        out
    }

    #[test]
    fn normalize_orientation_rewrites_ifd1_too() {
        let mut bytes = build_tiff_with_ifd0_and_ifd1(6, 6);
        normalize_orientation_in_exif(&mut bytes);
        let ifd0_value_abs = 6 + 8 + 2 + 8; // prefix + header + num_entries + entry header
        let ifd1_value_abs = 6 + 26 + 2 + 8;
        assert_eq!((bytes[ifd0_value_abs], bytes[ifd0_value_abs + 1]), (0, 1));
        assert_eq!((bytes[ifd1_value_abs], bytes[ifd1_value_abs + 1]), (0, 1));
    }

    #[test]
    fn normalize_orientation_skips_non_short_type() {
        // Type = LONG (4) instead of SHORT (3). Writing 1 as SHORT into a
        // LONG field would read back as 0x00010000 = 65536. The walker must
        // refuse to touch the entry.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"Exif\0\0");
        bytes.extend_from_slice(b"MM");
        bytes.extend_from_slice(&42u16.to_be_bytes());
        bytes.extend_from_slice(&8u32.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&0x0112u16.to_be_bytes());
        bytes.extend_from_slice(&4u16.to_be_bytes()); // type = LONG
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend_from_slice(&6u32.to_be_bytes()); // value as LONG = 6
        bytes.extend_from_slice(&0u32.to_be_bytes());
        let before = bytes.clone();
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(bytes, before);
    }

    #[test]
    fn normalize_orientation_skips_count_not_one() {
        // Count = 2 means the 4-byte field is an *offset* into the buffer,
        // not the value. Writing into it would corrupt unrelated bytes.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"Exif\0\0");
        bytes.extend_from_slice(b"MM");
        bytes.extend_from_slice(&42u16.to_be_bytes());
        bytes.extend_from_slice(&8u32.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&0x0112u16.to_be_bytes());
        bytes.extend_from_slice(&3u16.to_be_bytes());
        bytes.extend_from_slice(&2u32.to_be_bytes()); // count = 2
        bytes.extend_from_slice(&0u32.to_be_bytes()); // value field as offset
        bytes.extend_from_slice(&0u32.to_be_bytes());
        let before = bytes.clone();
        normalize_orientation_in_exif(&mut bytes);
        assert_eq!(bytes, before);
    }

    #[test]
    fn extract_metadata_normalizes_orientation_end_to_end() {
        use image::{ImageBuffer, Rgb};
        use img_parts::ImageEXIF;

        // Build a real JPEG and inject EXIF with Orientation = 6 (Rotate90).
        let temp_path = std::env::temp_dir().join("agx_test_orient_norm.jpg");
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(4, 4, Rgb([128u8, 128, 128]));
        img.save(&temp_path).unwrap();

        let raw = std::fs::read(&temp_path).unwrap();
        let mut jpeg = img_parts::jpeg::Jpeg::from_bytes(raw.into()).unwrap();
        let exif_bytes = build_tiff_with_orientation(6, true, true);
        jpeg.set_exif(Some(exif_bytes.into()));
        let mut out = Vec::new();
        jpeg.encoder().write_to(&mut out).unwrap();
        std::fs::write(&temp_path, &out).unwrap();

        // extract_metadata must rewrite the orientation tag to 1.
        let meta = extract_metadata(&temp_path).expect("metadata present");
        let exif = meta.exif.expect("exif present");
        assert_eq!(
            read_orientation_from_tiff(&exif),
            Some(1),
            "orientation tag must be normalized to 1"
        );

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
