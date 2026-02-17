use image::{DynamicImage, Rgb, Rgb32FImage};
use palette::{LinSrgb, Srgb};

use crate::error::Result;

/// Convert a linear sRGB f32 image buffer to a DynamicImage in sRGB gamma space.
pub fn linear_to_srgb_dynamic(linear: &Rgb32FImage) -> DynamicImage {
    let (w, h) = linear.dimensions();
    let srgb = Rgb32FImage::from_fn(w, h, |x, y| {
        let p = linear.get_pixel(x, y);
        let srgb: Srgb<f32> = LinSrgb::new(p.0[0], p.0[1], p.0[2]).into_encoding();
        Rgb([srgb.red, srgb.green, srgb.blue])
    });
    DynamicImage::ImageRgb32F(srgb)
}

/// Encode a linear sRGB f32 image to a file, converting to sRGB gamma space.
///
/// The output format is determined by the file extension.
pub fn encode_to_file(linear: &Rgb32FImage, path: &std::path::Path) -> Result<()> {
    let dynamic = linear_to_srgb_dynamic(linear);
    let rgb8 = dynamic.to_rgb8();
    rgb8.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;

    #[test]
    fn roundtrip_linear_to_srgb_pixel_values() {
        // linear 0.2159 should round-trip to sRGB ~128
        let linear: Rgb32FImage =
            ImageBuffer::from_pixel(1, 1, Rgb([0.2159f32, 0.2159, 0.2159]));
        let dynamic = linear_to_srgb_dynamic(&linear);
        let rgb8 = dynamic.to_rgb8();
        let pixel = rgb8.get_pixel(0, 0);
        assert!(
            (pixel.0[0] as i32 - 128).unsigned_abs() <= 1,
            "Expected ~128, got {}",
            pixel.0[0]
        );
    }

    #[test]
    fn encode_saves_file() {
        let temp_path = std::env::temp_dir().join("oxiraw_test_encode.png");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(2, 2, Rgb([0.5f32, 0.5, 0.5]));
        encode_to_file(&linear, &temp_path).unwrap();
        assert!(temp_path.exists());
        let _ = std::fs::remove_file(&temp_path);
    }
}
