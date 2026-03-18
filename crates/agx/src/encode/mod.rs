use std::io::Cursor;
use std::path::PathBuf;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::tiff::TiffEncoder;
use image::{DynamicImage, Rgb, Rgb32FImage};
use palette::{LinSrgb, Srgb};

use crate::error::Result;
use crate::metadata::ImageMetadata;

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
                .map_err(|e| crate::error::AgxError::Encode(e.to_string()))?;
            buf
        }
        OutputFormat::Png => {
            let mut buf = Vec::new();
            let encoder = PngEncoder::new(&mut buf);
            rgb8.write_with_encoder(encoder)
                .map_err(|e| crate::error::AgxError::Encode(e.to_string()))?;
            buf
        }
        OutputFormat::Tiff => {
            let mut buf = Vec::new();
            let cursor = Cursor::new(&mut buf);
            let encoder = TiffEncoder::new(cursor);
            rgb8.write_with_encoder(encoder)
                .map_err(|e| crate::error::AgxError::Encode(e.to_string()))?;
            buf
        }
    };

    // Inject metadata if available
    let buf = if let Some(meta) = metadata {
        inject_metadata(buf, format, meta)?
    } else {
        buf
    };

    std::fs::write(&final_path, &buf)?;

    // For TIFF output, inject metadata via little_exif after writing
    if format == OutputFormat::Tiff {
        if let Some(meta) = metadata {
            inject_metadata_tiff(&final_path, meta);
        }
    }

    Ok(final_path)
}

/// Encode a linear sRGB f32 image to a file, converting to sRGB gamma space.
///
/// Uses default options (JPEG quality 92, format inferred from extension).
/// For more control, use `encode_to_file_with_options`.
pub fn encode_to_file(linear: &Rgb32FImage, path: &std::path::Path) -> Result<()> {
    encode_to_file_with_options(linear, path, &EncodeOptions::default(), None)?;
    Ok(())
}

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
                .map_err(|e| crate::error::AgxError::Encode(format!("metadata injection: {e}")))?;
            if let Some(exif) = &metadata.exif {
                jpeg.set_exif(Some(exif.clone().into()));
            }
            if let Some(icc) = &metadata.icc_profile {
                jpeg.set_icc_profile(Some(icc.clone().into()));
            }
            let mut out = Vec::new();
            jpeg.encoder()
                .write_to(&mut out)
                .map_err(|e| crate::error::AgxError::Encode(format!("metadata write: {e}")))?;
            Ok(out)
        }
        OutputFormat::Png => {
            let mut png = img_parts::png::Png::from_bytes(buf.into())
                .map_err(|e| crate::error::AgxError::Encode(format!("metadata injection: {e}")))?;
            if let Some(exif) = &metadata.exif {
                png.set_exif(Some(exif.clone().into()));
            }
            if let Some(icc) = &metadata.icc_profile {
                png.set_icc_profile(Some(icc.clone().into()));
            }
            let mut out = Vec::new();
            png.encoder()
                .write_to(&mut out)
                .map_err(|e| crate::error::AgxError::Encode(format!("metadata write: {e}")))?;
            Ok(out)
        }
        OutputFormat::Tiff => Ok(buf), // Handled separately via inject_metadata_tiff
    }
}

/// Inject metadata into an existing TIFF file via little_exif. Best-effort — failures are silent.
fn inject_metadata_tiff(path: &std::path::Path, metadata: &ImageMetadata) {
    if let Some(exif_bytes) = &metadata.exif {
        let file_ext = little_exif::filetype::FileExtension::TIFF;
        if let Ok(exif_meta) = little_exif::metadata::Metadata::new_from_vec(exif_bytes, file_ext) {
            let _ = exif_meta.write_to_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageBuffer;
    use std::path::PathBuf;

    #[test]
    fn roundtrip_linear_to_srgb_pixel_values() {
        // linear 0.2159 should round-trip to sRGB ~128
        let linear: Rgb32FImage = ImageBuffer::from_pixel(1, 1, Rgb([0.2159f32, 0.2159, 0.2159]));
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
        let temp_path = std::env::temp_dir().join("agx_test_encode.png");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(2, 2, Rgb([0.5f32, 0.5, 0.5]));
        encode_to_file(&linear, &temp_path).unwrap();
        assert!(temp_path.exists());
        let _ = std::fs::remove_file(&temp_path);
    }

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
        let (path, fmt) = resolve_output(std::path::Path::new("out.jpg"), Some(OutputFormat::Jpeg));
        assert_eq!(fmt, OutputFormat::Jpeg);
        assert_eq!(path, PathBuf::from("out.jpg"));
    }

    #[test]
    fn resolve_output_format_override_mismatched_ext_appends() {
        let (path, fmt) = resolve_output(std::path::Path::new("out.png"), Some(OutputFormat::Jpeg));
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

    #[test]
    fn encode_jpeg_with_quality_produces_file() {
        let temp_path = std::env::temp_dir().join("agx_test_quality.jpg");
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

        let path_low = std::env::temp_dir().join("agx_test_q50.jpg");
        let path_high = std::env::temp_dir().join("agx_test_q95.jpg");

        let opts_low = EncodeOptions {
            jpeg_quality: 50,
            format: None,
        };
        let opts_high = EncodeOptions {
            jpeg_quality: 95,
            format: None,
        };

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
        let temp_path = std::env::temp_dir().join("agx_test_fmt.png");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
        let opts = EncodeOptions {
            jpeg_quality: 92,
            format: None,
        };
        let final_path = encode_to_file_with_options(&linear, &temp_path, &opts, None).unwrap();
        assert!(final_path.exists());
        let img = image::open(&final_path).unwrap();
        assert_eq!(img.width(), 4);
        let _ = std::fs::remove_file(&final_path);
    }

    #[test]
    fn encode_tiff_format() {
        let temp_path = std::env::temp_dir().join("agx_test_fmt.tiff");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
        let opts = EncodeOptions {
            jpeg_quality: 92,
            format: None,
        };
        let final_path = encode_to_file_with_options(&linear, &temp_path, &opts, None).unwrap();
        assert!(final_path.exists());
        let img = image::open(&final_path).unwrap();
        assert_eq!(img.width(), 4);
        let _ = std::fs::remove_file(&final_path);
    }

    #[test]
    fn encode_format_override_appends_extension() {
        let temp_path = std::env::temp_dir().join("agx_test_override.png");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
        let opts = EncodeOptions {
            jpeg_quality: 92,
            format: Some(OutputFormat::Jpeg),
        };
        let final_path = encode_to_file_with_options(&linear, &temp_path, &opts, None).unwrap();
        assert_eq!(
            final_path,
            std::env::temp_dir().join("agx_test_override.png.jpeg")
        );
        assert!(final_path.exists());
        let _ = std::fs::remove_file(&final_path);
    }

    #[test]
    fn metadata_roundtrip_jpeg() {
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

        let temp_path = std::env::temp_dir().join("agx_test_meta_rt.jpg");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
        let opts = EncodeOptions {
            jpeg_quality: 92,
            format: None,
        };
        encode_to_file_with_options(&linear, &temp_path, &opts, Some(&meta)).unwrap();

        let meta_out = crate::metadata::extract_metadata(&temp_path);
        assert!(meta_out.is_some(), "Should have metadata in output");
        assert!(
            meta_out.as_ref().unwrap().exif.is_some(),
            "Should have EXIF in output"
        );

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn encode_without_metadata_still_works() {
        let temp_path = std::env::temp_dir().join("agx_test_no_meta.jpg");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
        let opts = EncodeOptions::default();
        let result = encode_to_file_with_options(&linear, &temp_path, &opts, None);
        assert!(result.is_ok());
        let _ = std::fs::remove_file(result.unwrap());
    }
}
