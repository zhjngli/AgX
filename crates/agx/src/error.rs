//! Error types for the AgX library. See [`AgxError`].

use thiserror::Error;

/// All errors that can occur in the AgX rendering pipeline.
///
/// `AgxError` is the unified error type returned by every fallible AgX function:
/// decoding raw or encoded images, applying presets, building LUTs, encoding output.
/// Variants wrap underlying errors with a short context string so callers can match
/// on the kind without losing the original message.
#[derive(Debug, Error)]
pub enum AgxError {
    /// Failure during image decoding (raw, JPEG, PNG, TIFF, etc.).
    #[error("Decode error: {0}")]
    Decode(String),
    /// Failure during image encoding to an output file or buffer.
    #[error("Encode error: {0}")]
    Encode(String),
    /// Failure parsing or applying a preset.
    #[error("Preset error: {0}")]
    Preset(String),
    /// Failure parsing or applying a LUT (e.g. invalid `.cube` file).
    #[error("LUT error: {0}")]
    Lut(String),
    /// Underlying error from the `image` crate.
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    /// Underlying I/O error from the standard library.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Failure initializing the GPU runtime (no adapter, device creation failed, etc.).
    #[error("GPU error: {0}")]
    Gpu(String),
}

/// Convenience alias for `Result<T, AgxError>` used throughout the AgX API.
pub type Result<T> = std::result::Result<T, AgxError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_decode() {
        let err = AgxError::Decode("bad file".into());
        assert_eq!(err.to_string(), "Decode error: bad file");
    }

    #[test]
    fn error_display_encode() {
        let err = AgxError::Encode("write failed".into());
        assert_eq!(err.to_string(), "Encode error: write failed");
    }

    #[test]
    fn error_display_preset() {
        let err = AgxError::Preset("parse failed".into());
        assert_eq!(err.to_string(), "Preset error: parse failed");
    }

    #[test]
    fn error_display_lut() {
        let err = AgxError::Lut("invalid size".into());
        assert_eq!(err.to_string(), "LUT error: invalid size");
    }

    #[test]
    fn error_display_gpu() {
        let err = AgxError::Gpu("no adapter found".into());
        assert_eq!(err.to_string(), "GPU error: no adapter found");
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: AgxError = io_err.into();
        assert!(matches!(err, AgxError::Io(_)));
    }
}
