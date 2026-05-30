//! Image decoding: raw, JPEG, PNG, TIFF, and other formats supported by the image crate and libraw.

mod orientation;

#[cfg(feature = "raw")]
pub mod raw;

#[cfg(feature = "heic")]
pub mod heic;

#[cfg(feature = "icc")]
pub(crate) mod icc;

use image::Rgb32FImage;
use palette::{LinSrgb, Srgb};

use crate::color_space::LINEAR_SRGB_TO_LINEAR_REC2020;
use crate::error::{AgxError, Result};

/// Known raw file extensions supported via LibRaw.
const RAW_EXTENSIONS: &[&str] = &[
    "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "dng", "rw2", "orf", "pef",
    "srw", "x3f", "3fr", "fff", "iiq", "rwl", "mrw", "mdc", "dcr", "raw", "kdc", "erf", "mef",
    "mos",
];

/// Check if a file path has a known raw format extension.
pub fn is_raw_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| RAW_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Known HEIF container extensions decoded via libheif.
const HEIC_EXTENSIONS: &[&str] = &["heic", "heif"];

/// Check if a file path has a known HEIF container extension.
pub fn is_heic_extension(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| HEIC_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Decode any supported image file into linear Rec.2020 f32.
///
/// Auto-detects format from file extension:
/// - Standard formats (JPEG, PNG, TIFF, BMP, WebP): decoded via the `image` crate
/// - Raw formats (CR2, CR3, NEF, ARW, RAF, DNG, etc.): decoded via LibRaw (requires `raw` feature)
/// - HEIF container formats (HEIC, HEIF): decoded via libheif (requires `heic` feature)
///
/// All back-ends land in the engine working space (linear Rec.2020). Note:
/// during the wide-working-space migration, only the standard-format back-end
/// produces linear Rec.2020 today; RAW and HEIC will follow in subsequent
/// sub-projects.
pub fn decode(path: &std::path::Path) -> Result<Rgb32FImage> {
    if is_raw_extension(path) {
        #[cfg(feature = "raw")]
        {
            return raw::decode_raw(path);
        }
        #[cfg(not(feature = "raw"))]
        {
            return Err(AgxError::Decode(
                "raw format support requires the 'raw' feature flag".into(),
            ));
        }
    }
    if is_heic_extension(path) {
        #[cfg(feature = "heic")]
        {
            return heic::decode_heic(path);
        }
        #[cfg(not(feature = "heic"))]
        {
            return Err(AgxError::Decode(
                "heic format support requires the 'heic' feature flag".into(),
            ));
        }
    }
    decode_standard(path)
}

/// Extract raw embedded ICC profile bytes from a standard-format file.
///
/// JPEG (APP2) and PNG (iCCP) are read via `img-parts`; TIFF via the `tiff`
/// crate's `ICCProfile` tag (0x8773). Returns `None` when no profile is
/// embedded or the container can't be parsed.
#[cfg(feature = "icc")]
fn extract_icc_standard(path: &std::path::Path) -> Option<Vec<u8>> {
    use img_parts::ImageICC;
    use std::io::Read;

    // Probe the leading magic bytes before reading the whole file, so formats
    // that can't carry an ICC profile we parse (BMP, WebP, ...) bail out after
    // 4 bytes instead of allocating a full-file copy only to discard it.
    let mut file = std::fs::File::open(path).ok()?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).ok()?;

    // Read the rest of the file only once the magic matches a supported format.
    // `read_exact` already consumed the first 4 bytes, so prepend them back.
    let read_full = |mut file: std::fs::File| -> Option<Vec<u8>> {
        let mut bytes = magic.to_vec();
        file.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    };

    match magic {
        [0xFF, 0xD8, 0xFF, _] => img_parts::jpeg::Jpeg::from_bytes(read_full(file)?.into())
            .ok()?
            .icc_profile()
            .map(|icc| icc.to_vec()),
        [0x89, 0x50, 0x4E, 0x47] => img_parts::png::Png::from_bytes(read_full(file)?.into())
            .ok()?
            .icc_profile()
            .map(|icc| icc.to_vec()),
        [0x49, 0x49, 0x2A, 0x00] | [0x4D, 0x4D, 0x00, 0x2A] => {
            let bytes = read_full(file)?;
            let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&bytes)).ok()?;
            let icc = decoder.get_tag_u8_vec(tiff::tags::Tag::IccProfile).ok()?;
            (!icc.is_empty()).then_some(icc)
        }
        _ => None,
    }
}

/// Decode a standard image file (JPEG, PNG, TIFF, BMP, WebP) into a linear
/// Rec.2020 f32 buffer.
///
/// If the file embeds an ICC profile and the `icc` feature is enabled, lcms2
/// converts the gamma-encoded values straight to linear Rec.2020. Otherwise
/// (no profile, feature off, or malformed profile) the input is assumed to be
/// in sRGB gamma space: each pixel is converted from sRGB gamma to linear sRGB
/// and then matrix-converted into the engine working space (linear Rec.2020) in
/// a single fused per-pixel pass. The output `Rgb32FImage` is the engine's
/// working-space contract: linear Rec.2020.
pub fn decode_standard(path: &std::path::Path) -> Result<Rgb32FImage> {
    let img = image::ImageReader::open(path)
        .map_err(AgxError::Io)?
        .decode()
        .map_err(AgxError::Image)?;
    let orientation = orientation::read_orientation(path);
    let img = orientation.apply(img);
    let mut buf = img.into_rgb32f();

    // ICC path: if the file embeds a profile and the feature is on, let lcms2
    // convert the gamma-encoded values straight to linear Rec.2020. On any
    // failure, fall through to the sRGB assumption below.
    #[cfg(feature = "icc")]
    {
        if let Some(icc_bytes) = extract_icc_standard(path) {
            match icc::convert_to_working_space(&mut buf, &icc_bytes) {
                Ok(()) => return Ok(buf),
                Err(e) => {
                    eprintln!(
                        "agx: embedded ICC profile could not be applied ({e}); assuming sRGB"
                    );
                }
            }
        }
    }

    // sRGB fallback (also the path when the `icc` feature is disabled).
    let m = &LINEAR_SRGB_TO_LINEAR_REC2020;
    for px in buf.pixels_mut() {
        let lin: LinSrgb<f32> = Srgb::new(px.0[0], px.0[1], px.0[2]).into_linear();
        px.0 = [
            m[0][0] * lin.red + m[0][1] * lin.green + m[0][2] * lin.blue,
            m[1][0] * lin.red + m[1][1] * lin.green + m[1][2] * lin.blue,
            m[2][0] * lin.red + m[2][1] * lin.green + m[2][2] * lin.blue,
        ];
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    #[test]
    fn decode_png_to_linear_f32() {
        let temp_path = std::env::temp_dir().join("agx_test_decode.png");
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, Rgb([128, 128, 128]));
        img.save(&temp_path).unwrap();

        let result = decode_standard(&temp_path).unwrap();
        assert_eq!(result.width(), 2);
        assert_eq!(result.height(), 2);

        // sRGB 128/255 ≈ 0.502 → linear ≈ 0.2159. Grey is invariant under
        // sRGB↔Rec.2020 (rows of LINEAR_SRGB_TO_LINEAR_REC2020 sum to ~1).
        let pixel = result.get_pixel(0, 0);
        assert!(
            (pixel.0[0] - 0.2159).abs() < 0.01,
            "Expected ~0.2159, got {}",
            pixel.0[0]
        );

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn decode_preserves_per_pixel_channels() {
        // Asymmetric per-pixel and per-channel values catch in-place loop bugs
        // (channel swap, off-by-one indexing) that decode_png_to_linear_f32's
        // uniform-color image would not.
        let temp_path = std::env::temp_dir().join("agx_test_decode_asymmetric.png");
        let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(2, 2);
        img.put_pixel(0, 0, Rgb([255, 0, 0])); // red
        img.put_pixel(1, 0, Rgb([0, 255, 0])); // green
        img.put_pixel(0, 1, Rgb([0, 0, 255])); // blue
        img.put_pixel(1, 1, Rgb([0, 0, 0])); // black
        img.save(&temp_path).unwrap();

        let result = decode_standard(&temp_path).unwrap();
        let p00 = result.get_pixel(0, 0).0;
        let p10 = result.get_pixel(1, 0).0;
        let p01 = result.get_pixel(0, 1).0;
        let p11 = result.get_pixel(1, 1).0;

        // sRGB pure-channel inputs → linear sRGB unit vectors → linear
        // Rec.2020 columns of LINEAR_SRGB_TO_LINEAR_REC2020.
        let m = &LINEAR_SRGB_TO_LINEAR_REC2020;
        let red_expected = [m[0][0], m[1][0], m[2][0]];
        let green_expected = [m[0][1], m[1][1], m[2][1]];
        let blue_expected = [m[0][2], m[1][2], m[2][2]];
        let black_expected = [0.0_f32, 0.0, 0.0];

        let approx_eq = |a: f32, b: f32| (a - b).abs() < 1e-3;
        for c in 0..3 {
            assert!(
                approx_eq(p00[c], red_expected[c]),
                "red[{c}]: got {} expected {}",
                p00[c],
                red_expected[c]
            );
        }
        for c in 0..3 {
            assert!(
                approx_eq(p10[c], green_expected[c]),
                "green[{c}]: got {} expected {}",
                p10[c],
                green_expected[c]
            );
        }
        for c in 0..3 {
            assert!(
                approx_eq(p01[c], blue_expected[c]),
                "blue[{c}]: got {} expected {}",
                p01[c],
                blue_expected[c]
            );
        }
        for c in 0..3 {
            assert!(
                approx_eq(p11[c], black_expected[c]),
                "black[{c}]: got {} expected {}",
                p11[c],
                black_expected[c]
            );
        }

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn decode_standard_round_trips_to_srgb_via_inverse_matrix() {
        use crate::color_space::LINEAR_REC2020_TO_LINEAR_SRGB;
        // Decode a known sRGB pure-red sample; the matrix multiply means the
        // decoded value is in linear Rec.2020 rather than linear sRGB. Applying
        // the inverse matrix should recover linear sRGB ≈ (1, 0, 0).

        let temp_path = std::env::temp_dir().join("agx_test_decode_round_trip.png");
        let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(1, 1);
        img.put_pixel(0, 0, Rgb([255, 0, 0]));
        img.save(&temp_path).unwrap();

        let result = decode_standard(&temp_path).unwrap();
        let p = result.get_pixel(0, 0).0;

        let m = &LINEAR_REC2020_TO_LINEAR_SRGB;
        let back = [
            m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2],
            m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2],
            m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2],
        ];

        assert!((back[0] - 1.0).abs() < 1e-4, "red: got {}", back[0]);
        assert!(back[1].abs() < 1e-4, "green: got {}", back[1]);
        assert!(back[2].abs() < 1e-4, "blue: got {}", back[2]);

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn decode_nonexistent_file_returns_error() {
        let result = decode_standard(std::path::Path::new("/nonexistent/file.png"));
        assert!(result.is_err());
    }

    #[test]
    fn decode_without_icc_uses_srgb_fallback() {
        let temp_path = std::env::temp_dir().join("agx_test_no_icc.png");
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, Rgb([128, 128, 128]));
        img.save(&temp_path).unwrap();

        let result = decode_standard(&temp_path).unwrap();
        let pixel = result.get_pixel(0, 0);
        assert!(
            (pixel.0[0] - 0.2159).abs() < 0.01,
            "no-ICC decode must use sRGB fallback, got {}",
            pixel.0[0]
        );
        let _ = std::fs::remove_file(&temp_path);
    }

    #[cfg(feature = "icc")]
    #[test]
    fn decode_tagged_adobe_rgb_png_is_honored() {
        use img_parts::ImageICC;

        let icc = icc::adobe_rgb_icc();

        let red: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_pixel(2, 2, Rgb([255, 0, 0]));
        let mut png_bytes = Vec::new();
        red.write_with_encoder(image::codecs::png::PngEncoder::new(&mut png_bytes))
            .unwrap();
        let mut png = img_parts::png::Png::from_bytes(png_bytes.into()).unwrap();
        png.set_icc_profile(Some(icc.into()));
        let mut tagged = Vec::new();
        png.encoder().write_to(&mut tagged).unwrap();

        let temp_path = std::env::temp_dir().join("agx_test_adobe_rgb.png");
        std::fs::write(&temp_path, &tagged).unwrap();

        let decoded = decode_standard(&temp_path).unwrap();
        let p = decoded.get_pixel(0, 0).0;

        // sRGB-assumed decode of pure red lands ~0.627 R; Adobe RGB red is wider.
        assert!(
            p[0] > 0.70,
            "Adobe RGB red should map wider than sRGB red (~0.627); got {}",
            p[0]
        );
        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn is_raw_extension_detects_common_formats() {
        assert!(is_raw_extension(std::path::Path::new("photo.cr2")));
        assert!(is_raw_extension(std::path::Path::new("photo.CR2")));
        assert!(is_raw_extension(std::path::Path::new("photo.nef")));
        assert!(is_raw_extension(std::path::Path::new("photo.arw")));
        assert!(is_raw_extension(std::path::Path::new("photo.raf")));
        assert!(is_raw_extension(std::path::Path::new("photo.dng")));
        assert!(is_raw_extension(std::path::Path::new("photo.cr3")));
        assert!(is_raw_extension(std::path::Path::new("photo.rw2")));
    }

    #[test]
    fn is_raw_extension_rejects_standard_formats() {
        assert!(!is_raw_extension(std::path::Path::new("photo.jpg")));
        assert!(!is_raw_extension(std::path::Path::new("photo.png")));
        assert!(!is_raw_extension(std::path::Path::new("photo.tiff")));
        assert!(!is_raw_extension(std::path::Path::new("photo.bmp")));
    }

    #[test]
    fn decode_routes_png_to_standard() {
        let temp_path = std::env::temp_dir().join("agx_test_unified.png");
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(2, 2, Rgb([128, 128, 128]));
        img.save(&temp_path).unwrap();

        let result = decode(&temp_path);
        assert!(result.is_ok());

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn decode_nonexistent_raw_file_returns_error() {
        let result = decode(std::path::Path::new("/nonexistent/photo.cr2"));
        assert!(result.is_err());
    }

    /// Test decode() with a real raw file. Ignored by default.
    /// To run: place a .dng file at /tmp/agx_test_sample.dng and run:
    ///   cargo test -p agx --features raw -- --ignored decode_real_raw_file
    #[test]
    #[ignore]
    fn decode_real_raw_file() {
        let path = std::path::Path::new("/tmp/agx_test_sample.dng");
        if !path.exists() {
            eprintln!("Skipping: no sample raw file at {}", path.display());
            return;
        }

        let result = decode(path);
        assert!(
            result.is_ok(),
            "Failed to decode raw file: {:?}",
            result.err()
        );

        let img = result.unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);

        // Verify pixels are in a reasonable range (linear Rec.2020, mostly 0-1)
        let pixel = img.get_pixel(img.width() / 2, img.height() / 2);
        for i in 0..3 {
            assert!(
                pixel.0[i] >= 0.0 && pixel.0[i] <= 2.0,
                "Pixel channel {} out of expected range: {}",
                i,
                pixel.0[i]
            );
        }
    }

    #[test]
    fn is_heic_extension_detects_heif_container() {
        assert!(is_heic_extension(std::path::Path::new("photo.heic")));
        assert!(is_heic_extension(std::path::Path::new("photo.HEIC")));
        assert!(is_heic_extension(std::path::Path::new("photo.heif")));
        assert!(is_heic_extension(std::path::Path::new("photo.HEIF")));
    }

    #[test]
    fn is_heic_extension_rejects_other_formats() {
        assert!(!is_heic_extension(std::path::Path::new("photo.jpg")));
        assert!(!is_heic_extension(std::path::Path::new("photo.png")));
        assert!(!is_heic_extension(std::path::Path::new("photo.cr2")));
        assert!(!is_heic_extension(std::path::Path::new("photo.tiff")));
    }
}
