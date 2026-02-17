use image::{Rgb, Rgb32FImage};
use palette::{LinSrgb, Srgb};

use crate::error::{OxirawError, Result};

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
}
