use image::DynamicImage;
use std::path::Path;

/// EXIF orientation values (EXIF tag 0x0112).
///
/// These correspond to the 8 possible orientations defined by the EXIF spec,
/// describing how pixel rows/columns map to visual top/left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Row 0 = top, Col 0 = left (no transform needed)
    Normal,
    /// Row 0 = top, Col 0 = right (flip horizontal)
    FlipHorizontal,
    /// Row 0 = bottom, Col 0 = right (rotate 180)
    Rotate180,
    /// Row 0 = bottom, Col 0 = left (flip vertical)
    FlipVertical,
    /// Row 0 = left, Col 0 = top (transpose: flip horizontal + rotate 270)
    Transpose,
    /// Row 0 = right, Col 0 = top (rotate 90 CW)
    Rotate90,
    /// Row 0 = right, Col 0 = bottom (transverse: flip horizontal + rotate 90)
    Transverse,
    /// Row 0 = left, Col 0 = bottom (rotate 270 CW)
    Rotate270,
}

impl Orientation {
    /// Create an `Orientation` from the EXIF orientation tag value (1-8).
    /// Returns `Normal` for unknown or out-of-range values.
    pub fn from_exif_value(value: u16) -> Self {
        match value {
            1 => Orientation::Normal,
            2 => Orientation::FlipHorizontal,
            3 => Orientation::Rotate180,
            4 => Orientation::FlipVertical,
            5 => Orientation::Transpose,
            6 => Orientation::Rotate90,
            7 => Orientation::Transverse,
            8 => Orientation::Rotate270,
            _ => Orientation::Normal,
        }
    }

    /// Apply this orientation transform to a `DynamicImage`, returning the
    /// correctly oriented image.
    pub fn apply(self, img: DynamicImage) -> DynamicImage {
        match self {
            Orientation::Normal => img,
            Orientation::FlipHorizontal => img.fliph(),
            Orientation::Rotate180 => img.rotate180(),
            Orientation::FlipVertical => img.flipv(),
            Orientation::Transpose => img.fliph().rotate270(),
            Orientation::Rotate90 => img.rotate90(),
            Orientation::Transverse => img.fliph().rotate90(),
            Orientation::Rotate270 => img.rotate270(),
        }
    }
}

/// Read the EXIF orientation tag from an image file.
///
/// Returns `Orientation::Normal` if the file has no EXIF data, the orientation
/// tag is missing, or the format doesn't support EXIF (e.g., PNG, BMP).
pub fn read_orientation(path: &Path) -> Orientation {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Orientation::Normal,
    };
    let mut reader = std::io::BufReader::new(file);
    let exif = match exif::Reader::new().read_from_container(&mut reader) {
        Ok(e) => e,
        Err(_) => return Orientation::Normal,
    };
    match exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
        Some(field) => {
            if let Some(value) = field.value.get_uint(0) {
                Orientation::from_exif_value(value as u16)
            } else {
                Orientation::Normal
            }
        }
        None => Orientation::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn from_exif_value_maps_known_values() {
        assert_eq!(Orientation::from_exif_value(1), Orientation::Normal);
        assert_eq!(Orientation::from_exif_value(2), Orientation::FlipHorizontal);
        assert_eq!(Orientation::from_exif_value(3), Orientation::Rotate180);
        assert_eq!(Orientation::from_exif_value(4), Orientation::FlipVertical);
        assert_eq!(Orientation::from_exif_value(5), Orientation::Transpose);
        assert_eq!(Orientation::from_exif_value(6), Orientation::Rotate90);
        assert_eq!(Orientation::from_exif_value(7), Orientation::Transverse);
        assert_eq!(Orientation::from_exif_value(8), Orientation::Rotate270);
    }

    #[test]
    fn from_exif_value_defaults_unknown_to_normal() {
        assert_eq!(Orientation::from_exif_value(0), Orientation::Normal);
        assert_eq!(Orientation::from_exif_value(9), Orientation::Normal);
        assert_eq!(Orientation::from_exif_value(255), Orientation::Normal);
    }

    #[test]
    fn apply_normal_is_identity() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 2, Rgb([128, 64, 32])));
        let result = Orientation::Normal.apply(img.clone());
        assert_eq!(result.width(), 4);
        assert_eq!(result.height(), 2);
    }

    #[test]
    fn apply_rotate90_swaps_dimensions() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 2, Rgb([128, 64, 32])));
        let result = Orientation::Rotate90.apply(img);
        assert_eq!(result.width(), 2);
        assert_eq!(result.height(), 4);
    }

    #[test]
    fn apply_rotate270_swaps_dimensions() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 2, Rgb([128, 64, 32])));
        let result = Orientation::Rotate270.apply(img);
        assert_eq!(result.width(), 2);
        assert_eq!(result.height(), 4);
    }

    #[test]
    fn apply_rotate180_preserves_dimensions() {
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 2, Rgb([128, 64, 32])));
        let result = Orientation::Rotate180.apply(img);
        assert_eq!(result.width(), 4);
        assert_eq!(result.height(), 2);
    }

    #[test]
    fn read_orientation_returns_normal_for_nonexistent_file() {
        let orientation = read_orientation(Path::new("/nonexistent/file.jpg"));
        assert_eq!(orientation, Orientation::Normal);
    }

    #[test]
    fn read_orientation_returns_normal_for_png() {
        let temp_path = std::env::temp_dir().join("agx_test_orientation.png");
        let img = RgbImage::from_pixel(2, 2, Rgb([128, 128, 128]));
        img.save(&temp_path).unwrap();

        let orientation = read_orientation(&temp_path);
        assert_eq!(orientation, Orientation::Normal);

        let _ = std::fs::remove_file(&temp_path);
    }
}
