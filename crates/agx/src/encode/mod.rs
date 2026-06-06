//! Image encoding: writing rendered output to JPEG, PNG, TIFF.
//!
//! Input contract: linear Rec.2020 `Rgb32FImage`. This module converts to
//! 8-bit sRGB-encoded output via a fused matrix → curve → quantize pass.

use std::io::Cursor;
use std::path::PathBuf;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::{Rgb, Rgb32FImage, RgbImage};

use crate::color_space::{
    adobe_rgb_curve_signed, srgb_curve_signed, LINEAR_REC2020_TO_LINEAR_ADOBE_RGB,
    LINEAR_REC2020_TO_LINEAR_P3, LINEAR_REC2020_TO_LINEAR_SRGB,
};
use crate::error::Result;
use crate::metadata::ImageMetadata;

mod icc;

/// Supported output image formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// JPEG with quality control.
    Jpeg,
    /// PNG (lossless).
    Png,
    /// TIFF (lossless, 16-bit support).
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

/// Output color space: gamut primaries + transfer curve + embedded ICC profile.
///
/// Chosen at encode time via `--output-gamut`. The default (`Srgb`) reproduces
/// pre-SP4 output byte-for-byte. Not part of the preset schema (no serde) — it is
/// a delivery concern, parsed from the CLI via `FromStr`. The core crate has no
/// `clap` dependency, so this mirrors `VignetteShape` / `GrainType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputGamut {
    /// sRGB — universal, default. Byte-identical to pre-SP4 output.
    #[default]
    Srgb,
    /// Display P3 — DCI-P3 primaries, D65 white, sRGB transfer curve.
    DisplayP3,
    /// Adobe RGB (1998) — Adobe primaries, D65 white, gamma 2.19921875.
    AdobeRgb,
}

impl std::fmt::Display for OutputGamut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Srgb => write!(f, "srgb"),
            Self::DisplayP3 => write!(f, "p3"),
            Self::AdobeRgb => write!(f, "adobe-rgb"),
        }
    }
}

impl std::str::FromStr for OutputGamut {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "srgb" => Ok(Self::Srgb),
            "p3" => Ok(Self::DisplayP3),
            "adobe-rgb" => Ok(Self::AdobeRgb),
            _ => Err(format!(
                "invalid output gamut '{s}'. Use: srgb, p3, or adobe-rgb"
            )),
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

/// Convert a single post-curve f32 channel value to 8-bit u8 with clamping;
/// NaN/inf inputs map to 255, negative inputs to 0. Called from
/// `encode_linear_rec2020_to_rgb8` after the matrix and transfer-curve
/// passes (i.e. the input is already post-matrix, post-curve gamma).
///
/// Reproduces the rounding/clamping that `image::DynamicImage::to_rgb8()`
/// performs on `ImageRgb32F` input: clamp to [0, 1], scale to [0, 255], round
/// to nearest. NaN inputs produce 255 — matching the image crate's
/// `normalize_float`, which collapses NaN to the upper bound. This is
/// deliberate: it keeps output byte-identical to the prior
/// `linear_to_srgb_dynamic + to_rgb8` path even on out-of-range floats.
#[inline]
fn quantize_u8(x: f32) -> u8 {
    // NaN and values >= 1.0 all map to 255, matching `image`'s `normalize_float`.
    // `is_nan()` is required because NaN does not compare >= 1.0.
    let normalized = if x.is_nan() || x >= 1.0 {
        1.0
    } else {
        x.max(0.0)
    };
    (normalized * 255.0).round() as u8
}

/// Encode a linear Rec.2020 f32 image to an 8-bit `RgbImage` in a single fused
/// pass: `matrix` (Rec.2020 → target linear) → `curve` (target transfer, applied
/// sign-preserving) → `quantize_u8`. Allocating intermediate f32 buffers would
/// cost hundreds of MB at high resolution; this avoids that.
fn encode_linear_rec2020_to_rgb8(
    linear_rec2020: &Rgb32FImage,
    matrix: &[[f32; 3]; 3],
    curve: fn(f32) -> f32,
) -> RgbImage {
    let (w, h) = linear_rec2020.dimensions();
    RgbImage::from_fn(w, h, |x, y| {
        let p = linear_rec2020.get_pixel(x, y);
        let r = p.0[0];
        let g = p.0[1];
        let b = p.0[2];
        let r_t = matrix[0][0] * r + matrix[0][1] * g + matrix[0][2] * b;
        let g_t = matrix[1][0] * r + matrix[1][1] * g + matrix[1][2] * b;
        let b_t = matrix[2][0] * r + matrix[2][1] * g + matrix[2][2] * b;
        Rgb([
            quantize_u8(curve(r_t)),
            quantize_u8(curve(g_t)),
            quantize_u8(curve(b_t)),
        ])
    })
}

/// Encode a linear Rec.2020 f32 image to 8-bit sRGB. Thin wrapper over the
/// generalized pass; preserves the long-standing public entry point.
pub fn encode_linear_rec2020_to_srgb_rgb8(linear_rec2020: &Rgb32FImage) -> RgbImage {
    encode_linear_rec2020_to_rgb8(
        linear_rec2020,
        &LINEAR_REC2020_TO_LINEAR_SRGB,
        srgb_curve_signed,
    )
}

/// A (Rec.2020 → target linear matrix, transfer-curve) pair for one output gamut.
type GamutRecipe = (&'static [[f32; 3]; 3], fn(f32) -> f32);

/// The (matrix, transfer-curve) pair that realizes an output gamut. Display P3
/// reuses the sRGB transfer curve by design (it differs from sRGB only in
/// primaries); Adobe RGB uses its own gamma curve.
//
// Wired into `encode_to_file_with_options` once `EncodeOptions.output_gamut`
// lands; until then only the unit tests exercise it.
#[allow(dead_code)]
fn gamut_recipe(gamut: OutputGamut) -> GamutRecipe {
    match gamut {
        OutputGamut::Srgb => (&LINEAR_REC2020_TO_LINEAR_SRGB, srgb_curve_signed),
        OutputGamut::DisplayP3 => (&LINEAR_REC2020_TO_LINEAR_P3, srgb_curve_signed),
        OutputGamut::AdobeRgb => (&LINEAR_REC2020_TO_LINEAR_ADOBE_RGB, adobe_rgb_curve_signed),
    }
}

/// Encode a linear Rec.2020 f32 image to a file with full options.
///
/// Converts to 8-bit sRGB via a fused matrix → curve → quantize pass; resolves
/// the output format and path, encodes with the appropriate encoder, and
/// optionally injects metadata. Returns the final output path (which may
/// differ from the input path if an extension was appended).
pub fn encode_to_file_with_options(
    linear: &Rgb32FImage,
    path: &std::path::Path,
    options: &EncodeOptions,
    metadata: Option<&ImageMetadata>,
) -> Result<PathBuf> {
    let (final_path, format) = resolve_output(path, options.format);

    let rgb8 = encode_linear_rec2020_to_srgb_rgb8(linear);

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
            use tiff::encoder::{colortype, TiffEncoder};
            use tiff::tags::Tag;

            let raw = rgb8.into_raw();
            let (w, h) = (linear.width(), linear.height());

            let mut buf = Vec::new();
            {
                let cursor = Cursor::new(&mut buf);
                let mut tiff = TiffEncoder::new(cursor)
                    .map_err(|e| crate::error::AgxError::Encode(e.to_string()))?;
                let mut img = tiff
                    .new_image::<colortype::RGB8>(w, h)
                    .map_err(|e| crate::error::AgxError::Encode(e.to_string()))?;
                img.encoder()
                    .write_tag(Tag::IccProfile, crate::encode::icc::SRGB_V4_ICC)
                    .map_err(|e| crate::error::AgxError::Encode(e.to_string()))?;
                img.write_data(&raw)
                    .map_err(|e| crate::error::AgxError::Encode(e.to_string()))?;
            }
            buf
        }
    };

    let buf = match format {
        OutputFormat::Jpeg => inject_jpeg_icc_and_exif(buf, metadata)?,
        OutputFormat::Png => inject_png_icc_and_exif(buf, metadata)?,
        OutputFormat::Tiff => buf,
    };

    std::fs::write(&final_path, &buf)?;

    if format == OutputFormat::Tiff {
        if let Some(meta) = metadata {
            inject_metadata_tiff(&final_path, meta);
        }
    }

    Ok(final_path)
}

/// Encode a linear Rec.2020 f32 image to a file, converting to 8-bit sRGB.
///
/// Uses default options (JPEG quality 92, format inferred from extension).
/// For more control, use `encode_to_file_with_options`.
pub fn encode_to_file(linear: &Rgb32FImage, path: &std::path::Path) -> Result<()> {
    encode_to_file_with_options(linear, path, &EncodeOptions::default(), None)?;
    Ok(())
}

/// Inject sRGB ICC + optional EXIF into a JPEG buffer. ICC is unconditional
/// per the encode module's output-labeling contract; EXIF passes through
/// only if input metadata carried it.
fn inject_jpeg_icc_and_exif(buf: Vec<u8>, metadata: Option<&ImageMetadata>) -> Result<Vec<u8>> {
    use img_parts::{ImageEXIF, ImageICC};

    let mut jpeg = img_parts::jpeg::Jpeg::from_bytes(buf.into())
        .map_err(|e| crate::error::AgxError::Encode(format!("metadata injection: {e}")))?;
    jpeg.set_icc_profile(Some(crate::encode::icc::SRGB_V4_ICC.to_vec().into()));
    if let Some(exif) = metadata.and_then(|m| m.exif.as_ref()) {
        jpeg.set_exif(Some(exif.clone().into()));
    }
    let mut out = Vec::new();
    jpeg.encoder()
        .write_to(&mut out)
        .map_err(|e| crate::error::AgxError::Encode(format!("metadata write: {e}")))?;
    Ok(out)
}

/// Inject sRGB ICC + optional EXIF into a PNG buffer. Mirrors the JPEG path.
fn inject_png_icc_and_exif(buf: Vec<u8>, metadata: Option<&ImageMetadata>) -> Result<Vec<u8>> {
    use img_parts::{ImageEXIF, ImageICC};

    let mut png = img_parts::png::Png::from_bytes(buf.into())
        .map_err(|e| crate::error::AgxError::Encode(format!("metadata injection: {e}")))?;
    png.set_icc_profile(Some(crate::encode::icc::SRGB_V4_ICC.to_vec().into()));
    if let Some(exif) = metadata.and_then(|m| m.exif.as_ref()) {
        png.set_exif(Some(exif.clone().into()));
    }
    let mut out = Vec::new();
    png.encoder()
        .write_to(&mut out)
        .map_err(|e| crate::error::AgxError::Encode(format!("metadata write: {e}")))?;
    Ok(out)
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
        // linear 0.2159 should round-trip to sRGB ~128. Greyscale input is
        // matrix-invariant (rows of LINEAR_REC2020_TO_LINEAR_SRGB sum to ~1.0).
        let linear: Rgb32FImage = ImageBuffer::from_pixel(1, 1, Rgb([0.2159f32, 0.2159, 0.2159]));
        let rgb8 = encode_linear_rec2020_to_srgb_rgb8(&linear);
        let pixel = rgb8.get_pixel(0, 0);
        assert!(
            (pixel.0[0] as i32 - 128).unsigned_abs() <= 1,
            "Expected ~128, got {}",
            pixel.0[0]
        );
    }

    #[test]
    fn quantize_u8_handles_edge_values() {
        // NaN, +∞, and values >= 1.0 all collapse to 255 — matching `image`
        // crate's `normalize_float` (and therefore the prior `to_rgb8` path).
        assert_eq!(quantize_u8(f32::NAN), 255);
        assert_eq!(quantize_u8(f32::INFINITY), 255);
        assert_eq!(quantize_u8(f32::NEG_INFINITY), 0);
        assert_eq!(quantize_u8(0.0), 0);
        assert_eq!(quantize_u8(1.0), 255);
        assert_eq!(quantize_u8(-1.0), 0);
        assert_eq!(quantize_u8(2.0), 255);
    }

    #[test]
    fn encode_rec2020_to_srgb_rgb8_quantization_table() {
        // Hardcoded expected values pin the linear→sRGB→u8 conversion behavior
        // against future drift in the curve or quantize_u8. Computed from the
        // pre-refactor `linear_to_srgb_dynamic + to_rgb8` path; updates here must
        // be deliberate decisions, not accidental regressions.
        // why: greyscale is matrix-invariant (rows of LINEAR_REC2020_TO_LINEAR_SRGB
        // sum to ~1.0), so this pinning table survives the working-space widening.
        let table: &[(f32, u8)] = &[
            (0.0, 0),
            (0.0001, 0),
            (0.0031308, 10),
            (0.04, 56),
            (0.18, 118),
            (0.2159, 128),
            (0.5, 188),
            (0.9, 243),
            (0.999, 255),
            (1.0, 255),
            (1.0001, 255),
            (2.0, 255),
            (-0.001, 0),
            (-1.0, 0),
            (f32::NAN, 255),
        ];

        for &(linear, expected) in table {
            let img: Rgb32FImage = ImageBuffer::from_pixel(1, 1, Rgb([linear, linear, linear]));
            let rgb8 = encode_linear_rec2020_to_srgb_rgb8(&img);
            let actual = rgb8.get_pixel(0, 0).0[0];
            assert_eq!(
                actual, expected,
                "linear {linear} should encode to u8 {expected}, got {actual}"
            );
        }
    }

    #[test]
    fn encode_rec2020_to_srgb_rgb8_preserves_pure_srgb_via_inverse_matrix() {
        use crate::color_space::LINEAR_SRGB_TO_LINEAR_REC2020;

        // Start with linear sRGB pure red, push it into linear Rec.2020
        // (decode side), then encode. The encoder's matrix is the inverse;
        // we should round-trip back to sRGB pure red (255, 0, 0).
        let m_in = &LINEAR_SRGB_TO_LINEAR_REC2020;
        let rec2020_red = [
            m_in[0][0] * 1.0 + m_in[0][1] * 0.0 + m_in[0][2] * 0.0,
            m_in[1][0] * 1.0 + m_in[1][1] * 0.0 + m_in[1][2] * 0.0,
            m_in[2][0] * 1.0 + m_in[2][1] * 0.0 + m_in[2][2] * 0.0,
        ];

        let img: Rgb32FImage = ImageBuffer::from_pixel(1, 1, Rgb(rec2020_red));
        let rgb8 = encode_linear_rec2020_to_srgb_rgb8(&img);
        let px = rgb8.get_pixel(0, 0).0;
        assert_eq!(px[0], 255, "sRGB red R: got {}", px[0]);
        assert_eq!(px[1], 0, "sRGB red G: got {}", px[1]);
        assert_eq!(px[2], 0, "sRGB red B: got {}", px[2]);
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

    #[test]
    fn encode_jpeg_embeds_srgb_v4_icc() {
        use crate::encode::icc::SRGB_V4_ICC;
        use img_parts::ImageICC;

        let temp_path = std::env::temp_dir().join("agx_test_icc_jpeg.jpg");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
        encode_to_file(&linear, &temp_path).unwrap();

        let bytes = std::fs::read(&temp_path).unwrap();
        let jpeg = img_parts::jpeg::Jpeg::from_bytes(bytes.into()).unwrap();
        let icc = jpeg
            .icc_profile()
            .expect("output JPEG must carry an ICC profile");
        assert_eq!(
            &icc[..],
            SRGB_V4_ICC,
            "embedded ICC must equal the SRGB_V4_ICC blob"
        );

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn encode_tiff_embeds_srgb_v4_icc() {
        use crate::encode::icc::SRGB_V4_ICC;

        let temp_path = std::env::temp_dir().join("agx_test_icc_tiff.tiff");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
        let opts = EncodeOptions {
            jpeg_quality: 92,
            format: Some(OutputFormat::Tiff),
        };
        encode_to_file_with_options(&linear, &temp_path, &opts, None).unwrap();

        let bytes = std::fs::read(&temp_path).unwrap();
        let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(bytes))
            .expect("output must be parseable as TIFF");
        let icc = decoder
            .get_tag_u8_vec(tiff::tags::Tag::IccProfile)
            .expect("output TIFF must carry an ICCProfile tag");
        assert_eq!(icc, SRGB_V4_ICC);

        let _ = std::fs::remove_file(&temp_path);
    }

    /// TIFF writes ICC inline during encode; `inject_metadata_tiff` then
    /// rewrites the file post-write via `little_exif` to add EXIF. This
    /// test pins that the ICC tag survives the EXIF rewrite — if
    /// `little_exif` ever reconstructs the IFD in a way that drops
    /// unknown tags, the regression surfaces here.
    #[test]
    fn encode_tiff_with_exif_still_embeds_srgb_v4_icc() {
        use crate::encode::icc::SRGB_V4_ICC;

        let exif_bytes = vec![
            0x45, 0x78, 0x69, 0x66, 0x00, 0x00, b'M', b'M', 0x00, 0x2A, 0x00, 0x00, 0x00, 0x08,
        ];
        let meta = ImageMetadata {
            exif: Some(exif_bytes),
        };

        let temp_path = std::env::temp_dir().join("agx_test_icc_tiff_with_exif.tiff");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
        let opts = EncodeOptions {
            jpeg_quality: 92,
            format: Some(OutputFormat::Tiff),
        };
        encode_to_file_with_options(&linear, &temp_path, &opts, Some(&meta)).unwrap();

        let bytes = std::fs::read(&temp_path).unwrap();
        let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(bytes))
            .expect("output must be parseable as TIFF after EXIF injection");
        let icc = decoder.get_tag_u8_vec(tiff::tags::Tag::IccProfile).expect(
            "ICC tag must survive little_exif post-write — regression in EXIF-then-ICC ordering",
        );
        assert_eq!(icc, SRGB_V4_ICC);

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn encode_png_embeds_srgb_v4_icc() {
        use crate::encode::icc::SRGB_V4_ICC;
        use img_parts::ImageICC;

        let temp_path = std::env::temp_dir().join("agx_test_icc_png.png");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));
        let opts = EncodeOptions {
            jpeg_quality: 92,
            format: Some(OutputFormat::Png),
        };
        encode_to_file_with_options(&linear, &temp_path, &opts, None).unwrap();

        let bytes = std::fs::read(&temp_path).unwrap();
        let png = img_parts::png::Png::from_bytes(bytes.into()).unwrap();
        let icc = png
            .icc_profile()
            .expect("output PNG must carry an ICC profile");
        assert_eq!(&icc[..], SRGB_V4_ICC);

        let _ = std::fs::remove_file(&temp_path);
    }

    /// Pin the contract that input metadata never influences output ICC.
    /// `ImageMetadata` no longer carries an `icc_profile` field, but a
    /// caller could still pass EXIF; this test confirms the EXIF path
    /// does not somehow leak into the ICC slot.
    #[test]
    fn encode_jpeg_overrides_any_input_metadata_with_srgb_icc() {
        use crate::encode::icc::SRGB_V4_ICC;
        use img_parts::ImageICC;

        let temp_path = std::env::temp_dir().join("agx_test_icc_override.jpg");
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.5, 0.5]));

        let exif_bytes = vec![
            0x45, 0x78, 0x69, 0x66, 0x00, 0x00, b'M', b'M', 0x00, 0x2A, 0x00, 0x00, 0x00, 0x08,
        ];
        let meta = ImageMetadata {
            exif: Some(exif_bytes),
        };

        let opts = EncodeOptions::default();
        encode_to_file_with_options(&linear, &temp_path, &opts, Some(&meta)).unwrap();

        let bytes = std::fs::read(&temp_path).unwrap();
        let jpeg = img_parts::jpeg::Jpeg::from_bytes(bytes.into()).unwrap();
        let icc = jpeg.icc_profile().expect("output must have ICC");
        assert_eq!(
            &icc[..],
            SRGB_V4_ICC,
            "input metadata must not influence output ICC"
        );

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn output_gamut_default_is_srgb() {
        assert_eq!(OutputGamut::default(), OutputGamut::Srgb);
    }

    #[test]
    fn output_gamut_round_trips_through_string() {
        use std::str::FromStr;
        for (s, g) in [
            ("srgb", OutputGamut::Srgb),
            ("p3", OutputGamut::DisplayP3),
            ("adobe-rgb", OutputGamut::AdobeRgb),
        ] {
            assert_eq!(OutputGamut::from_str(s).unwrap(), g);
            assert_eq!(g.to_string(), s);
        }
    }

    #[test]
    fn output_gamut_rejects_unknown() {
        use std::str::FromStr;
        let err = OutputGamut::from_str("rec2020").unwrap_err();
        assert!(err.contains("rec2020"));
    }

    #[test]
    fn srgb_recipe_matches_legacy_srgb_encode() {
        // The generalized path, called with the sRGB recipe, must reproduce the
        // dedicated sRGB function byte-for-byte (the zero-churn guarantee).
        use crate::color_space::{srgb_curve_signed, LINEAR_REC2020_TO_LINEAR_SRGB};
        let mut img = Rgb32FImage::new(4, 4);
        for (i, p) in img.pixels_mut().enumerate() {
            let v = i as f32 / 16.0;
            *p = Rgb([v, 1.0 - v, 0.5 * v]);
        }
        let legacy = encode_linear_rec2020_to_srgb_rgb8(&img);
        let generic =
            encode_linear_rec2020_to_rgb8(&img, &LINEAR_REC2020_TO_LINEAR_SRGB, srgb_curve_signed);
        assert_eq!(legacy.into_raw(), generic.into_raw());
    }

    #[test]
    fn gamut_recipe_selects_distinct_matrices() {
        // p3 and adobe-rgb must produce different bytes than srgb for a saturated
        // input (proves the recipe actually switches the conversion).
        let mut img = Rgb32FImage::new(2, 2);
        for p in img.pixels_mut() {
            *p = Rgb([0.9, 0.1, 0.2]);
        }
        let srgb = {
            let (m, c) = gamut_recipe(OutputGamut::Srgb);
            encode_linear_rec2020_to_rgb8(&img, m, c).into_raw()
        };
        for g in [OutputGamut::DisplayP3, OutputGamut::AdobeRgb] {
            let (m, c) = gamut_recipe(g);
            let out = encode_linear_rec2020_to_rgb8(&img, m, c).into_raw();
            assert_ne!(srgb, out, "{g} should differ from srgb");
        }
    }

    /// Pin pixel data unchanged after the ICC embed. Encode a colored
    /// (non-greyscale) Rec.2020 input across JPEG / PNG / TIFF; decode
    /// back and assert each pixel matches what the public per-pixel
    /// kernel `encode_linear_rec2020_to_srgb_rgb8` produces independently.
    /// Colored input exercises the off-diagonal matrix terms — greyscale
    /// would short-circuit them. JPEG gets a small tolerance because
    /// chroma subsampling shifts colored pixels by ±2 even at q=100;
    /// PNG and TIFF are lossless.
    #[test]
    fn encode_pixel_bytes_unchanged_after_icc_embed() {
        let linear: Rgb32FImage = ImageBuffer::from_pixel(4, 4, Rgb([0.5f32, 0.1, 0.2]));
        let expected_rgb8 = encode_linear_rec2020_to_srgb_rgb8(&linear);
        let expected_pixel = expected_rgb8.get_pixel(0, 0).0;

        for (fmt, ext) in [
            (OutputFormat::Jpeg, "jpg"),
            (OutputFormat::Png, "png"),
            (OutputFormat::Tiff, "tiff"),
        ] {
            let path = std::env::temp_dir().join(format!("agx_test_pix_{ext}.{ext}"));
            let opts = EncodeOptions {
                jpeg_quality: 100,
                format: Some(fmt),
            };
            encode_to_file_with_options(&linear, &path, &opts, None).unwrap();

            let img = image::open(&path).unwrap().to_rgb8();
            assert_eq!(img.dimensions(), (4, 4), "fmt {fmt:?} dims wrong");
            let px = img.get_pixel(0, 0).0;

            let tol: i32 = if fmt == OutputFormat::Jpeg { 3 } else { 0 };
            for c in 0..3 {
                let d = (px[c] as i32 - expected_pixel[c] as i32).abs();
                assert!(
                    d <= tol,
                    "fmt {fmt:?} channel {c} px={} expected={} tol={tol}",
                    px[c],
                    expected_pixel[c],
                );
            }
            let _ = std::fs::remove_file(&path);
        }
    }
}
