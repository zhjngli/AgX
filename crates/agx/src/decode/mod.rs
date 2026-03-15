#[cfg(feature = "raw")]
pub mod raw;

use image::{Rgb, Rgb32FImage};
use palette::{LinSrgb, Srgb};

use crate::error::{OxirawError, Result};

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

/// Decode any supported image file into linear sRGB f32.
///
/// Auto-detects format from file extension:
/// - Standard formats (JPEG, PNG, TIFF, BMP, WebP): decoded via the `image` crate
/// - Raw formats (CR2, CR3, NEF, ARW, RAF, DNG, etc.): decoded via LibRaw (requires `raw` feature)
pub fn decode(path: &std::path::Path) -> Result<Rgb32FImage> {
    if is_raw_extension(path) {
        #[cfg(feature = "raw")]
        {
            return raw::decode_raw(path);
        }
        #[cfg(not(feature = "raw"))]
        {
            return Err(OxirawError::Decode(
                "raw format support requires the 'raw' feature flag".into(),
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
        .map_err(OxirawError::Io)?
        .decode()
        .map_err(OxirawError::Image)?;
    let srgb_f32 = img.into_rgb32f();
    let (w, h) = srgb_f32.dimensions();
    let linear = Rgb32FImage::from_fn(w, h, |x, y| {
        let p = srgb_f32.get_pixel(x, y);
        let lin: LinSrgb<f32> = Srgb::new(p.0[0], p.0[1], p.0[2]).into_linear();
        Rgb([lin.red, lin.green, lin.blue])
    });
    Ok(linear)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;

    #[test]
    fn decode_png_to_linear_f32() {
        let temp_path = std::env::temp_dir().join("oxiraw_test_decode.png");
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
        let temp_path = std::env::temp_dir().join("oxiraw_test_unified.png");
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
    /// To run: place a .dng file at /tmp/oxiraw_test_sample.dng and run:
    ///   cargo test -p oxiraw --features raw -- --ignored decode_real_raw_file
    #[test]
    #[ignore]
    fn decode_real_raw_file() {
        let path = std::path::Path::new("/tmp/oxiraw_test_sample.dng");
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
}
