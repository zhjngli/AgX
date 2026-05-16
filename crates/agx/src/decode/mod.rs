//! Image decoding: raw, JPEG, PNG, TIFF, and other formats supported by the image crate and libraw.

mod orientation;

#[cfg(feature = "raw")]
pub mod raw;

#[cfg(feature = "heic")]
pub mod heic;

use image::Rgb32FImage;
use palette::{LinSrgb, Srgb};

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

/// Decode any supported image file into linear sRGB f32.
///
/// Auto-detects format from file extension:
/// - Standard formats (JPEG, PNG, TIFF, BMP, WebP): decoded via the `image` crate
/// - Raw formats (CR2, CR3, NEF, ARW, RAF, DNG, etc.): decoded via LibRaw (requires `raw` feature)
/// - HEIF container formats (HEIC, HEIF): decoded via libheif (requires `heic` feature)
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

/// Decode a standard image file (JPEG, PNG, TIFF) into a linear sRGB f32 buffer.
///
/// The input image is assumed to be in sRGB gamma space. Each pixel is converted
/// to linear sRGB for internal processing.
pub fn decode_standard(path: &std::path::Path) -> Result<Rgb32FImage> {
    let img = image::ImageReader::open(path)
        .map_err(AgxError::Io)?
        .decode()
        .map_err(AgxError::Image)?;
    let orientation = orientation::read_orientation(path);
    let img = orientation.apply(img);
    let mut buf = img.into_rgb32f();
    for px in buf.pixels_mut() {
        let lin: LinSrgb<f32> = Srgb::new(px.0[0], px.0[1], px.0[2]).into_linear();
        px.0 = [lin.red, lin.green, lin.blue];
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

        // sRGB 128/255 ≈ 0.502 → linear ≈ 0.2159
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

        // sRGB 255 → linear 1.0; sRGB 0 → linear 0.0
        assert!(
            (p00[0] - 1.0).abs() < 0.001 && p00[1] < 0.001 && p00[2] < 0.001,
            "red pixel: {p00:?}"
        );
        assert!(
            p10[0] < 0.001 && (p10[1] - 1.0).abs() < 0.001 && p10[2] < 0.001,
            "green pixel: {p10:?}"
        );
        assert!(
            p01[0] < 0.001 && p01[1] < 0.001 && (p01[2] - 1.0).abs() < 0.001,
            "blue pixel: {p01:?}"
        );
        assert!(
            p11[0] < 0.001 && p11[1] < 0.001 && p11[2] < 0.001,
            "black pixel: {p11:?}"
        );

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn decode_nonexistent_file_returns_error() {
        let result = decode_standard(std::path::Path::new("/nonexistent/file.png"));
        assert!(result.is_err());
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

        // Verify pixels are in a reasonable range (linear sRGB, mostly 0-1)
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
